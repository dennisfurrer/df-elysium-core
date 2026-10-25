//! Bridging across both chains against the fake node answering as Elysium testnet and HyperEVM
//! testnet did (`journeys` of `vectors/elysium_testnet.json`): a real HYPE deposit and a real
//! retryable ticket read from their parent transactions and found on Elysium by the hashes the
//! crate computes, and real withdrawals claimed: one ready with the call that was simulated on
//! HyperEVM, one already spent, one not yet under the confirmed assertion.

use df_elysium_core::bridge::claim::{self, Claim};
use df_elysium_core::bridge::deposit::DEPOSIT_ETH;
use df_elysium_core::bridge::{Deposit, Message, MessageKind, Ticket, Withdrawal, apply_alias, deposit, deposit_wei};
use df_elysium_core::client::{Rpc, TxKind};
use df_elysium_core::utils::hype;
use df_elysium_core::{Client, Network, U256};

use crate::support::node::{Chain, Node, Shared};
use crate::support::{address, bytes, step, text, word};

fn parent(node: &Shared) -> Rpc<Shared> {
    Rpc::new(node.clone(), Network::Testnet.hyperliquid().evm_rpc_url())
}

fn hash_of(entry: &serde_json::Value) -> df_elysium_core::B256 {
    word(text(&entry["params"][0]))
}

#[tokio::test]
async fn a_deposit_read_on_hyperevm_is_found_on_elysium_by_the_hash_computed_for_it() {
    let journey = |name: &str| step("deposit", name);
    let node = Node::testnet()
        .recorded(Chain::HyperEvm, &journey("parent_transaction"))
        .recorded(Chain::HyperEvm, &journey("parent_receipt"))
        .recorded(Chain::Elysium, &journey("transaction"))
        .recorded(Chain::Elysium, &journey("receipt"))
        .shared();
    let (parent, client) = (parent(&node), Client::reader(node.clone(), Network::Testnet));
    let contracts = Network::Testnet.contracts();

    // What the depositor sent on HyperEVM is the call the crate builds.
    let sent = parent.transaction(hash_of(&journey("parent_transaction"))).await.unwrap().unwrap();
    let call = deposit_wei(Network::Testnet, sent.value);
    assert_eq!((call.to, &call.data, call.value), (sent.to, &sent.input, sent.value));
    assert_eq!(call.data, DEPOSIT_ETH.selector());
    assert_eq!(deposit(Network::Testnet, hype(sent.value).unwrap()), Ok(call), "the same from HYPE as a decimal");
    assert_eq!(sent.chain_id, Some(Network::Testnet.parent_chain_id()));

    let receipt = parent.receipt(hash_of(&journey("parent_receipt"))).await.unwrap().unwrap();
    let messages = Message::in_logs(&receipt.logs, &contracts).unwrap();
    let [message] = messages.as_slice() else { panic!("one message: {messages:?}") };
    assert_eq!((message.number, message.kind), (U256::from(1_493_518u64), MessageKind::EthDeposit));
    let deposit = Deposit::from_message(message).unwrap();
    assert_eq!(
        (deposit.from, deposit.to, deposit.value),
        (apply_alias(sent.from), sent.from, sent.value),
        "from the alias, to the depositor"
    );

    let hash = deposit.tx_hash(Network::Testnet.chain_id());
    assert_eq!(hash, hash_of(&journey("transaction")), "known before it lands");
    let landed = client.transaction(hash).await.unwrap().unwrap();
    assert_eq!(landed.tx_kind(), TxKind::Deposit);
    assert_eq!((landed.from, landed.to, landed.value), (deposit.from, Some(deposit.to), deposit.value));
    assert_eq!(landed.message_number(), Some(message.number));
    assert!(client.receipt(hash).await.unwrap().unwrap().succeeded());
}

#[tokio::test]
async fn a_retryable_read_on_hyperevm_is_found_on_elysium_by_its_ticket_id_and_its_redeem_names_it() {
    let journey = |name: &str| step("retryable", name);
    let node = Node::testnet()
        .recorded(Chain::HyperEvm, &journey("parent_receipt"))
        .recorded(Chain::Elysium, &journey("submit_transaction"))
        .recorded(Chain::Elysium, &journey("submit_receipt"))
        .recorded(Chain::Elysium, &journey("retry_transaction"))
        .shared();
    let (parent, client) = (parent(&node), Client::reader(node.clone(), Network::Testnet));

    let receipt = parent.receipt(hash_of(&journey("parent_receipt"))).await.unwrap().unwrap();
    let messages = Message::in_logs(&receipt.logs, &Network::Testnet.contracts()).unwrap();
    let tickets: Vec<Ticket> = messages.iter().filter_map(Ticket::from_message).collect();
    assert!(!tickets.is_empty() && tickets.len() == messages.len(), "every message here is a ticket");
    let submitted = hash_of(&journey("submit_transaction"));
    let ticket = tickets.iter().find(|ticket| ticket.id(Network::Testnet.chain_id()) == submitted);
    let ticket = ticket.unwrap_or_else(|| panic!("no ticket hashes to {submitted}: {tickets:?}"));
    assert_eq!(ticket.message_number, U256::from(1_493_519u64));

    let submission = client.transaction(submitted).await.unwrap().unwrap();
    assert_eq!(submission.tx_kind(), TxKind::SubmitRetryable);
    assert_eq!(submission.message_number(), Some(ticket.message_number));
    assert_eq!(submission.from, ticket.from, "the sender's alias");
    assert_eq!(submission.deposit_value, Some(ticket.deposit));
    assert_eq!(submission.beneficiary, Some(ticket.retryable.call_value_refund_address));
    assert_eq!(submission.retry_to, Some(ticket.retryable.to));
    assert_eq!(submission.retry_data.as_deref(), Some(ticket.retryable.data.as_slice()));
    let redeem = client.transaction(hash_of(&journey("retry_transaction"))).await.unwrap().unwrap();
    assert_eq!((redeem.tx_kind(), redeem.ticket_id), (TxKind::Retry, Some(submitted)));
}

/// A node answering every read of the recorded withdrawal's claim.
fn claim_node() -> Shared {
    let journey = |name: &str| step("withdrawal", name);
    Node::testnet()
        .recorded(Chain::Elysium, &journey("receipt"))
        .recorded(Chain::Elysium, &journey("confirmed_block"))
        .recorded(Chain::Elysium, &journey("proof"))
        .recorded(Chain::HyperEvm, &journey("latest_confirmed"))
        .recorded(Chain::HyperEvm, &journey("assertion"))
        .recorded(Chain::HyperEvm, &journey("assertion_created"))
        .recorded(Chain::HyperEvm, &journey("is_spent"))
        .recorded(Chain::Elysium, &step("claimed_withdrawal", "receipt"))
        .recorded(Chain::HyperEvm, &step("claimed_withdrawal", "is_spent"))
        // Any other leaf: not spent.
        .answer_to(
            Chain::HyperEvm,
            "eth_call",
            &Network::Testnet.contracts().outbox.to_hex(),
            r#"{"jsonrpc":"2.0","id":1,"result":"0x0000000000000000000000000000000000000000000000000000000000000000"}"#,
        )
        .shared()
}

async fn withdrawal(client: &Client<Shared>, journey: &str) -> Withdrawal {
    let receipt = client.receipt(hash_of(&step(journey, "receipt"))).await.unwrap().unwrap();
    let withdrawals = Withdrawal::in_logs(&receipt.logs).unwrap();
    let [withdrawal] = withdrawals.as_slice() else { panic!("one send: {withdrawals:?}") };
    withdrawal.clone()
}

#[tokio::test]
async fn a_confirmed_withdrawal_is_claimed_with_the_call_hyperevm_ran_and_a_spent_one_is_left() {
    let node = claim_node();
    let (parent, client) = (parent(&node), Client::reader(node.clone(), Network::Testnet));
    let contracts = Network::Testnet.contracts();

    let sent = withdrawal(&client, "withdrawal").await;
    assert_eq!(sent.position, 746_935);
    assert_eq!(sent.value, U256::from(2_000_000_000_000_000u64), "0.002 HYPE");
    let Claim::Ready { call, confirmed } = claim::claim(&client, &parent, &sent).await.unwrap() else {
        panic!("a leaf under the confirmed assertion")
    };
    let simulated = step("withdrawal", "execute_simulated");
    assert_eq!(call.to, Some(contracts.outbox));
    assert_eq!(call.data, bytes(&simulated["params"][0]["data"]), "the call HyperEVM ran without a revert");
    assert_eq!(call.value, U256::ZERO);
    assert_eq!(confirmed.block_hash, hash_of(&step("withdrawal", "confirmed_block")));
    assert_eq!(confirmed.assertion, word(text(&crate::support::result(&step("withdrawal", "latest_confirmed")))));
    let parents = node.methods(Chain::HyperEvm);
    assert_eq!(parents, ["eth_call", "eth_call", "eth_call", "eth_getLogs"], "spent, latest, its assertion, its log");

    let claimed = withdrawal(&client, "claimed_withdrawal").await;
    assert_eq!(claim::claim(&client, &parent, &claimed).await, Ok(Claim::Spent));
}

#[tokio::test]
async fn a_withdrawal_past_the_confirmed_send_count_waits() {
    let node = claim_node();
    let (parent, client) = (parent(&node), Client::reader(node.clone(), Network::Testnet));
    // The first leaf the confirmed assertion does not cover: the confirmed block's send count.
    let later = Withdrawal { position: 746_936, ..withdrawal(&client, "withdrawal").await };
    assert_eq!(claim::claim(&client, &parent, &later).await, Ok(Claim::Waiting { confirmed: 746_936 }));
    let latest = claim::latest(&Network::Testnet.contracts(), &parent).await.unwrap();
    assert_eq!(latest.block_hash, hash_of(&step("withdrawal", "confirmed_block")));
}

#[test]
fn the_alias_of_the_recorded_depositor_is_the_deposit_s_sender() {
    let depositor = address("0x966351de426254d7af0534a626f2e2d351d51eea");
    assert_eq!(apply_alias(depositor), address("0xa77451de426254d7af0534a626f2e2d351d52ffb"));
}
