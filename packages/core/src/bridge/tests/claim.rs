//! Claims against the chain itself. The rollup's reads as HyperEVM testnet answered them
//! (`hyperevm_testnet.json`); a live withdrawal's journey (`elysium_testnet.json`): the latest
//! confirmed assertion, the block it names, the proof, and an `executeTransaction` HyperEVM ran in
//! simulation without a revert; a claim HyperEVM testnet executed, rebuilt byte for byte from its
//! Elysium log; and viem's calldata and decodings (`arbitrum_sdk.json`). [`claim`] runs end to end
//! over the journey's recorded answers, in the order it asks.

use df_hyperliquid_core::client::evm::abi;
use serde_json::{Value, json};

use crate::client::rpc::eth;
use crate::client::{Client, Network};
use crate::test_support::{self, Wire, address, block_on, bytes, logs, ok, result, step, text};
use crate::utils::hex;

use super::*;

/// One recorded call to HyperEVM testnet's rollup contracts.
fn parent_call(contract: &str, name: &str) -> Value {
    let file = test_support::hyperevm();
    let found =
        file["calls"].as_array().unwrap().iter().find(|call| call["contract"] == contract && call["name"] == name);
    found.unwrap_or_else(|| panic!("no {contract}.{name}")).clone()
}

fn word_answer(call: &Value) -> B256 {
    text(&result(text(&call["answer"]))).parse().unwrap()
}

fn sent_as(read: &Read<impl Sized>, call: &Value) {
    assert_eq!(read.call.to.map(|to| to.to_hex()), Some(text(&call["params"][0]["to"]).to_lowercase()));
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&call["params"][0]["data"]));
}

#[test]
fn the_rollup_names_the_contracts_elysium_s_network_names() {
    let contracts = Network::Testnet.contracts();
    for (name, ours) in [
        ("inbox", contracts.inbox),
        ("outbox", contracts.outbox),
        ("bridge", contracts.bridge),
        ("sequencerInbox", contracts.sequencer_inbox),
        ("rollupEventInbox", contracts.rollup_event_inbox),
        ("challengeManager", contracts.challenge_manager),
    ] {
        let call = parent_call("Rollup", name);
        assert_eq!(text(&call["params"][0]["to"]), contracts.rollup.to_hex(), "{name}");
        assert_eq!(word_answer(&call).to_address(), Some(ours), "{name}");
    }
    for contract in ["Bridge", "Outbox", "SequencerInbox"] {
        assert_eq!(word_answer(&parent_call(contract, "rollup")).to_address(), Some(contracts.rollup), "{contract}");
    }
    assert_eq!(U256::from(word_answer(&parent_call("Rollup", "chainId"))), U256::from(Network::Testnet.chain_id()));
    let period = parent_call("Rollup", "confirmPeriodBlocks");
    sent_as(&confirm_period_blocks(&contracts), &period);
    assert_eq!(confirm_period_blocks(&contracts).decode(&bytes(&result(text(&period["answer"])))), Ok(10));
}

#[test]
fn the_latest_confirmed_assertion_reads_its_creation_block_and_status() {
    let contracts = Network::Testnet.contracts();
    let latest = parent_call("Rollup", "latestConfirmed");
    sent_as(&latest_confirmed(&contracts), &latest);
    let hash = latest_confirmed(&contracts).decode(&bytes(&result(text(&latest["answer"])))).unwrap();
    assert_eq!(hash, word_answer(&latest));
    let asked = parent_call("Rollup", "getAssertion");
    sent_as(&assertion(&contracts, hash), &asked);
    let node = assertion(&contracts, hash).decode(&bytes(&result(text(&asked["answer"])))).unwrap();
    let said = asked["decoded"][0].as_array().unwrap();
    assert_eq!(node.first_child_block, said[0].as_u64().unwrap());
    assert_eq!(node.second_child_block, said[1].as_u64().unwrap());
    assert_eq!(node.created_at_block, 67_848_830);
    assert!(node.is_first_child);
    assert_eq!(node.status, AssertionStatus::Confirmed);
    assert_eq!(node.config_hash.to_hex(), text(&said[5]));
    // And viem's reading of the same struct, at another moment.
    let file = test_support::arbitrum_sdk();
    let answer = file["answers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| text(&one["signature"]).starts_with("function getAssertion("))
        .unwrap()
        .clone();
    let node = assertion(&contracts, B256::ZERO).decode(&bytes(&answer["data"])).unwrap();
    assert_eq!(node.created_at_block.to_string(), text(&answer["decoded"]["createdAtBlock"]));
    assert_eq!(node.first_child_block.to_string(), text(&answer["decoded"]["firstChildBlock"]));
    assert_eq!(node.status, AssertionStatus::Confirmed);
}

/// The `AssertionCreated` log of the assertion the withdrawal's journey found confirmed.
#[test]
fn an_assertion_created_log_names_the_block_and_send_root_it_confirms() {
    let contracts = Network::Testnet.contracts();
    let journey = &test_support::elysium()["journeys"]["withdrawal"];
    let hash: B256 = text(&result(text(&journey["latest_confirmed"]["answer"]))).parse().unwrap();
    let filter = assertion_created_filter(&contracts, hash, 67_848_830);
    let sent: Value = serde_json::from_str(&eth::logs(&filter).body()).unwrap();
    assert_eq!(sent["params"], journey["assertion_created"]["params"], "one block, the rollup's, by its hash");
    let found: Vec<Log> = serde_json::from_value(result(text(&journey["assertion_created"]["answer"]))).unwrap();
    let [log] = found.as_slice() else { panic!("one log: {found:?}") };
    assert_eq!(log.topics[0], ASSERTION_CREATED.topic());
    let confirmed = read_assertion_created(log).unwrap();
    let block = result(text(&journey["confirmed_block"]["answer"]));
    assert_eq!(confirmed.assertion, hash);
    assert_eq!(confirmed.block_hash.to_hex(), text(&block["hash"]));
    assert_eq!(confirmed.send_root.to_hex(), text(&block["sendRoot"]));
    // The outbox knows that root as that block's.
    let root_call = &journey["root"];
    let read = root(&contracts, confirmed.send_root);
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&root_call["params"][0]["data"]));
    assert_eq!(read.decode(&bytes(&result(text(&root_call["answer"])))), Ok(confirmed.block_hash));
    // And viem's reading of a live log and its confirmation.
    let file = test_support::arbitrum_sdk();
    let event = |prefix: &str| {
        file["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| text(&one["signature"]).starts_with(prefix))
            .unwrap()
            .clone()
    };
    let created = event("event AssertionCreated(");
    assert_eq!(ASSERTION_CREATED.topic().to_hex(), text(&created["topic"]));
    let log: Log = serde_json::from_value(created["log"].clone()).unwrap();
    let confirmed = read_assertion_created(&log).unwrap();
    let after = &created["decoded"]["assertion"]["afterState"]["globalState"]["bytes32Vals"];
    assert_eq!(confirmed.assertion.to_hex(), text(&created["decoded"]["assertionHash"]));
    assert_eq!([confirmed.block_hash.to_hex(), confirmed.send_root.to_hex()], [text(&after[0]), text(&after[1])]);
    let confirmation = event("event AssertionConfirmed(");
    assert_eq!(ASSERTION_CONFIRMED.topic().to_hex(), text(&confirmation["topic"]));
    let tokens = ASSERTION_CONFIRMED.decode(&serde_json::from_value(confirmation["log"].clone()).unwrap()).unwrap();
    assert_eq!(
        hex::encode_prefixed(tokens[1].as_bytes().unwrap()),
        confirmed.block_hash.to_hex(),
        "the same assertion"
    );
    assert_eq!(hex::encode_prefixed(tokens[2].as_bytes().unwrap()), confirmed.send_root.to_hex());
    for (event, prefix) in [
        (SEND_ROOT_UPDATED, "event SendRootUpdated("),
        (OUT_BOX_TRANSACTION_EXECUTED, "event OutBoxTransactionExecuted("),
    ] {
        assert_eq!(event.topic().to_hex(), text(&self::event_topic(&file, prefix)), "{prefix}");
    }
    let mut wrong = log;
    wrong.topics = vec![SEND_ROOT_UPDATED.topic()];
    assert!(read_assertion_created(&wrong).is_err(), "another event's log");
}

fn event_topic(file: &Value, prefix: &str) -> Value {
    file["events"].as_array().unwrap().iter().find(|one| text(&one["signature"]).starts_with(prefix)).unwrap()["topic"]
        .clone()
}

/// The recorded withdrawal, its proof, and the claim HyperEVM ran in simulation.
fn journey_withdrawal() -> (Withdrawal, OutboxProof) {
    let sends = Withdrawal::in_logs(&logs(&step("withdrawal", "receipt"))).unwrap();
    let proof = read_outbox_proof(&bytes(&result(&step("withdrawal", "proof")))).unwrap();
    (sends[0].clone(), proof)
}

use crate::nitro::node::{OutboxProof, read_outbox_proof};

#[test]
fn a_confirmed_withdrawal_is_executed_with_the_call_hyperevm_ran() {
    let contracts = Network::Testnet.contracts();
    let (withdrawal, proof) = journey_withdrawal();
    let journey = &test_support::elysium()["journeys"]["withdrawal"];
    let block = result(text(&journey["confirmed_block"]["answer"]));
    assert_eq!(proof.root.to_hex(), text(&block["sendRoot"]), "a proof against the confirmed root");
    assert_eq!(proof.send, withdrawal.leaf(), "of this leaf");
    let call = execute(&contracts, &withdrawal, &proof.proof);
    let simulated = &journey["execute_simulated"]["params"][0];
    assert_eq!(call.to.map(|to| to.to_hex()), Some(text(&simulated["to"]).to_owned()));
    assert_eq!(hex::encode_prefixed(&call.data), text(&simulated["data"]), "the call HyperEVM ran without a revert");
    assert_eq!(result(text(&journey["execute_simulated"]["answer"])), json!("0x"));
    let spent = &journey["is_spent"];
    let read = is_spent(&contracts, withdrawal.position);
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&spent["params"][0]["data"]));
    assert_eq!(read.decode(&bytes(&result(text(&spent["answer"])))), Ok(false));
}

/// HyperEVM testnet's own claim of a gateway's withdrawal: its calldata rebuilt from the Elysium
/// log and the proof it carried, and the outbox's log of it.
#[test]
fn a_claim_hyperevm_executed_is_rebuilt_byte_for_byte_from_its_log() {
    let contracts = Network::Testnet.contracts();
    let sends = Withdrawal::in_logs(&logs(&step("claimed_withdrawal", "receipt"))).unwrap();
    let [withdrawal] = sends.as_slice() else { panic!("one send: {sends:?}") };
    assert_eq!(withdrawal.position, 746_823);
    assert!(!withdrawal.data.is_empty(), "a call on the parent chain, not plain HYPE");
    let parent = result(&step("claimed_withdrawal", "parent_transaction"));
    assert_eq!(address(text(&parent["to"])), contracts.outbox);
    let input = bytes(&parent["input"]);
    let params = abi::params(
        "bytes32[] proof, uint256 index, address l2Sender, address to, uint256 l2Block, uint256 l1Block, \
         uint256 l2Timestamp, uint256 value, bytes data",
    )
    .unwrap();
    let tokens = abi::decode(&params, &input[4..]).unwrap();
    let proof: Vec<B256> = tokens[0]
        .as_items()
        .unwrap()
        .iter()
        .map(|item| B256(<[u8; 32]>::try_from(item.as_bytes().unwrap()).unwrap()))
        .collect();
    assert_eq!(execute(&contracts, withdrawal, &proof).data, input);
    let receipt = result(&step("claimed_withdrawal", "parent_receipt"));
    let outbox_logs: Vec<Log> = serde_json::from_value(receipt["logs"].clone()).unwrap();
    let executed = outbox_logs.iter().find(|log| OUT_BOX_TRANSACTION_EXECUTED.matches(log)).unwrap();
    let tokens = OUT_BOX_TRANSACTION_EXECUTED.decode(executed).unwrap();
    assert_eq!(tokens[0].as_address(), Some(withdrawal.destination));
    assert_eq!(tokens[1].as_address(), Some(withdrawal.caller));
    assert_eq!(tokens[3].as_u64(), Some(withdrawal.position), "the leaf it spent");
    let spent = test_support::elysium()["journeys"]["claimed_withdrawal"]["is_spent"].clone();
    assert_eq!(
        hex::encode_prefixed(&is_spent(&contracts, withdrawal.position).call.data),
        text(&spent["params"][0]["data"])
    );
    assert_eq!(is_spent(&contracts, 0).decode(&bytes(&result(text(&spent["answer"])))), Ok(true));
}

/// viem's calldata for the outbox: a synthetic three-word proof, and a live claim's.
#[test]
fn every_outbox_call_is_viem_s_calldata() {
    let contracts = Network::Testnet.contracts();
    let file = test_support::arbitrum_sdk();
    let calls: Vec<Value> =
        file["calldata"].as_array().unwrap().iter().filter(|one| one["contract"] == "Outbox").cloned().collect();
    let executes: Vec<&Value> =
        calls.iter().filter(|one| text(&one["signature"]).starts_with("function executeTransaction(")).collect();
    assert_eq!(executes.len(), 2);
    for entry in executes {
        let args = entry["args"].as_array().unwrap();
        let proof: Vec<B256> = args[0].as_array().unwrap().iter().map(|word| text(word).parse().unwrap()).collect();
        let number = |at: usize| -> U256 { text(&args[at]).parse().unwrap() };
        let withdrawal = Withdrawal {
            caller: address(text(&args[2])),
            destination: address(text(&args[3])),
            hash: U256::ZERO,
            position: number(1).to_u64().unwrap(),
            arb_block: number(4).to_u64().unwrap(),
            eth_block: number(5).to_u64().unwrap(),
            timestamp: number(6).to_u64().unwrap(),
            value: number(7),
            data: hex::decode(text(&args[8])).unwrap(),
        };
        assert_eq!(
            hex::encode_prefixed(&execute(&contracts, &withdrawal, &proof).data),
            text(&entry["calldata"]),
            "{} words",
            proof.len()
        );
    }
    let one = |prefix: &str| calls.iter().find(|one| text(&one["signature"]).starts_with(prefix)).unwrap().clone();
    let spent = one("function isSpent(");
    assert_eq!(
        hex::encode_prefixed(&is_spent(&contracts, text(&spent["args"][0]).parse().unwrap()).call.data),
        text(&spent["calldata"])
    );
    let roots = one("function roots(");
    assert_eq!(
        hex::encode_prefixed(&root(&contracts, text(&roots["args"][0]).parse().unwrap()).call.data),
        text(&roots["calldata"])
    );
    let rollup: Vec<Value> =
        file["calldata"].as_array().unwrap().iter().filter(|one| one["contract"] == "Rollup").cloned().collect();
    let find = |prefix: &str| rollup.iter().find(|one| text(&one["signature"]).starts_with(prefix)).unwrap().clone();
    assert_eq!(
        hex::encode_prefixed(&latest_confirmed(&contracts).call.data),
        text(&find("function latestConfirmed(")["calldata"])
    );
    let asked = find("function getAssertion(");
    let hash: B256 = text(&asked["args"][0]).parse().unwrap();
    assert_eq!(hex::encode_prefixed(&assertion(&contracts, hash).call.data), text(&asked["calldata"]));
    assert_eq!(
        hex::encode_prefixed(&confirm_period_blocks(&contracts).call.data),
        text(&find("function confirmPeriodBlocks(")["calldata"])
    );
}

/// A wire answering each recorded step of the withdrawal journey, in turn.
fn answering(steps: &[&str]) -> Wire {
    Wire::answering(steps.iter().map(|name| ok(&step("withdrawal", name))))
}

fn parent(wire: Wire) -> Rpc<Wire> {
    Rpc::new(wire, Network::Testnet.hyperliquid().evm_rpc_url())
}

/// Every call `claim` made, as its method and params.
fn asked(wire: &Wire) -> Vec<(String, Value)> {
    wire.bodies().into_iter().map(|body| (text(&body["method"]).to_owned(), body["params"].clone())).collect()
}

fn recorded(name: &str) -> (String, Value) {
    let found = &test_support::elysium()["journeys"]["withdrawal"][name];
    let mut params = found["params"].clone();
    // The journey's recorder wrote addresses as the chain spells them; the crate writes lowercase.
    if let Some(call) = params.get_mut(0).and_then(Value::as_object_mut)
        && let Some(Value::String(to)) = call.get_mut("to")
    {
        *to = to.to_lowercase();
    }
    (text(&found["method"]).to_owned(), params)
}

#[test]
fn a_claim_reads_both_chains_to_the_call_that_executes_it() {
    let (withdrawal, _) = journey_withdrawal();
    let elysium = Client::reader(answering(&["confirmed_block", "proof"]), Network::Testnet);
    let hyperevm = parent(answering(&["is_spent", "latest_confirmed", "assertion", "assertion_created"]));
    let claimed = block_on(claim(&elysium, &hyperevm, &withdrawal)).unwrap();
    let Claim::Ready { call, confirmed } = claimed else { panic!("{claimed:?}") };
    let simulated = &test_support::elysium()["journeys"]["withdrawal"]["execute_simulated"]["params"][0];
    assert_eq!(hex::encode_prefixed(&call.data), text(&simulated["data"]));
    assert_eq!((call.to, call.value), (Some(Network::Testnet.contracts().outbox), U256::ZERO));
    assert_eq!(confirmed.block_hash.to_hex(), text(&result(&step("withdrawal", "confirmed_block"))["hash"]));
    assert_eq!(
        asked(hyperevm.http()),
        ["is_spent", "latest_confirmed", "assertion", "assertion_created"].map(recorded)
    );
    assert_eq!(asked(elysium.http()), ["confirmed_block", "proof"].map(recorded));
}

#[test]
fn a_spent_withdrawal_is_spent_and_one_past_the_confirmed_tree_waits() {
    let (withdrawal, _) = journey_withdrawal();
    let spent =
        test_support::elysium()["journeys"]["claimed_withdrawal"]["is_spent"]["answer"].as_str().unwrap().to_owned();
    let elysium = Client::reader(Wire::answering([]), Network::Testnet);
    let hyperevm = parent(Wire::answering([ok(&spent)]));
    assert_eq!(block_on(claim(&elysium, &hyperevm, &withdrawal)), Ok(Claim::Spent));
    assert!(elysium.http().sent().is_empty(), "nothing more is read");

    let past = Withdrawal { position: 746_936, ..withdrawal };
    let elysium = Client::reader(answering(&["confirmed_block"]), Network::Testnet);
    let hyperevm = parent(answering(&["is_spent", "latest_confirmed", "assertion", "assertion_created"]));
    assert_eq!(block_on(claim(&elysium, &hyperevm, &past)), Ok(Claim::Waiting { confirmed: 746_936 }));
}

#[test]
fn a_claim_that_does_not_add_up_is_refused() {
    let (withdrawal, _) = journey_withdrawal();
    let run = |elysium_answers: Vec<String>, parent_answers: Vec<String>| {
        let elysium = Client::reader(Wire::answering(elysium_answers.iter().map(|one| ok(one))), Network::Testnet);
        let hyperevm = parent(Wire::answering(parent_answers.iter().map(|one| ok(one))));
        block_on(claim(&elysium, &hyperevm, &withdrawal))
    };
    let steps = |names: &[&str]| names.iter().map(|name| step("withdrawal", name)).collect::<Vec<_>>();
    let up_to_logs = steps(&["is_spent", "latest_confirmed", "assertion"]);
    let no_logs = [up_to_logs.clone(), vec![r#"{"jsonrpc":"2.0","id":1,"result":[]}"#.to_owned()]].concat();
    assert!(matches!(run(Vec::new(), no_logs), Err(ClaimError::NoAssertionLog { block: 67_848_830, .. })));
    let parent_answers = steps(&["is_spent", "latest_confirmed", "assertion", "assertion_created"]);
    let unknown = vec![r#"{"jsonrpc":"2.0","id":1,"result":null}"#.to_owned()];
    assert!(matches!(run(unknown, parent_answers.clone()), Err(ClaimError::UnknownBlock(_))));
    let mut block: Value = serde_json::from_str(&step("withdrawal", "confirmed_block")).unwrap();
    block["result"]["sendRoot"] = json!(B256::ZERO.to_hex());
    let moved = vec![block.to_string()];
    assert!(matches!(run(moved, parent_answers.clone()), Err(ClaimError::Mismatch { .. })), "a send root that moved");
    let mut proof: Value = serde_json::from_str(&step("withdrawal", "proof")).unwrap();
    let text_proof = text(&proof["result"]).to_owned();
    proof["result"] = json!(format!("0x{}{}", &text_proof[2..66], "00".repeat(32)) + &text_proof[130..]);
    let wrong = vec![step("withdrawal", "confirmed_block"), proof.to_string()];
    assert!(matches!(run(wrong, parent_answers), Err(ClaimError::Mismatch { .. })), "a proof against another root");
}
