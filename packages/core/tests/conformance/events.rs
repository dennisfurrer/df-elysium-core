//! Event logs against viem's `decodeEventLog` with the ABIs of @arbitrum/nitro-contracts 3.2.0:
//! each log read through the crate's [`Event`] for it, every parameter (tuples of tuples included)
//! equal to viem's by name, and its topic equal to viem's. Most logs are real ones from Elysium and
//! HyperEVM testnet; the rest viem built. ArbSys's deprecated `L2ToL1Transaction` and
//! ArbRetryableTx's `Redeemed` have no `Event` in the crate and are left out.

use df_elysium_core::bridge::{Withdrawal, claim, message};
use df_elysium_core::client::{Event, Log};
use df_elysium_core::nitro::{retryable, sys};
use df_hyperliquid_core::client::evm::abi::{self, Kind, Param, Token};
use serde_json::Value;

use crate::support::{arbitrum_sdk, text, word};

/// The crate's event for a vector, by name.
fn event(name: &str) -> Option<Event> {
    Some(match name {
        "L2ToL1Tx" => sys::L2_TO_L1_TX,
        "SendMerkleUpdate" => sys::SEND_MERKLE_UPDATE,
        "TicketCreated" => retryable::TICKET_CREATED,
        "RedeemScheduled" => retryable::REDEEM_SCHEDULED,
        "Canceled" => retryable::CANCELED,
        "LifetimeExtended" => retryable::LIFETIME_EXTENDED,
        "MessageDelivered" => message::MESSAGE_DELIVERED,
        "InboxMessageDelivered" => message::INBOX_MESSAGE_DELIVERED,
        "InboxMessageDeliveredFromOrigin" => message::INBOX_MESSAGE_DELIVERED_FROM_ORIGIN,
        "OutBoxTransactionExecuted" => claim::OUT_BOX_TRANSACTION_EXECUTED,
        "SendRootUpdated" => claim::SEND_ROOT_UPDATED,
        "AssertionConfirmed" => claim::ASSERTION_CONFIRMED,
        "AssertionCreated" => claim::ASSERTION_CREATED,
        _ => return None,
    })
}

/// What viem wrote for a value: a string, or a JSON boolean or number as its text.
fn said(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_ascii_lowercase(),
        other => other.to_string(),
    }
}

/// Whether `token` of `kind` is what viem decoded as `value`, at `path`.
fn same(kind: &Kind, token: &Token, value: &Value, path: &str) {
    match (kind, token) {
        (Kind::Tuple(params), Token::Tuple(items)) => same_params(params, items, value, path),
        (Kind::Array(inner) | Kind::Sized(inner, _), Token::Array(items)) => {
            let values = value.as_array().unwrap_or_else(|| panic!("{path}: viem has no list"));
            assert_eq!(items.len(), values.len(), "{path}");
            for (at, (item, value)) in items.iter().zip(values).enumerate() {
                same(inner, item, value, &format!("{path}[{at}]"));
            }
        }
        _ => assert_eq!(token.show(kind).to_ascii_lowercase(), said(value), "{path}"),
    }
}

/// A parameter list against viem's object of them by name, or its list by place.
fn same_params(params: &[Param], tokens: &[Token], value: &Value, path: &str) {
    assert_eq!(params.len(), tokens.len(), "{path}");
    for (at, (param, token)) in params.iter().zip(tokens).enumerate() {
        let name = param.name.clone().unwrap_or_else(|| at.to_string());
        let viem = match value {
            Value::Object(fields) => &fields[&name],
            Value::Array(items) => &items[at],
            other => panic!("{path}: viem wrote {other}"),
        };
        same(&param.kind, token, viem, &format!("{path}.{name}"));
    }
}

#[test]
fn every_log_decodes_as_viem_decodes_it_and_its_topic_is_viem_s() {
    let file = arbitrum_sdk();
    let mut decoded = Vec::new();
    for case in file["events"].as_array().unwrap() {
        let signature = text(&case["signature"]);
        let name = signature.trim_start_matches("event ").split('(').next().unwrap();
        let Some(event) = event(name) else { continue };
        assert_eq!(event.topic(), word(text(&case["topic"])), "{name}: topic");
        assert_eq!(event.name(), name);
        let log: Log = serde_json::from_value(case["log"].clone()).unwrap();
        assert!(event.matches(&log), "{name}");
        let tokens = event.decode(&log).unwrap_or_else(|error| panic!("{name}: {error}"));
        same_params(&abi::params(event.params).unwrap(), &tokens, &case["decoded"], name);
        decoded.push(name);
    }
    assert_eq!(decoded.len(), 13, "{decoded:?}");
}

#[test]
fn the_real_logs_read_into_the_crate_s_records() {
    let file = arbitrum_sdk();
    let live = |name: &str| {
        let case =
            file["events"].as_array().unwrap().iter().find(|case| {
                text(&case["signature"]).starts_with(&format!("event {name}(")) && case.get("live").is_some()
            });
        let case = case.unwrap_or_else(|| panic!("no live {name}"));
        (serde_json::from_value::<Log>(case["log"].clone()).unwrap(), case["decoded"].clone())
    };

    // A withdrawal on Elysium: everything its claim needs but the proof.
    let (log, said) = live("L2ToL1Tx");
    let sent = Withdrawal::from_log(&log).unwrap();
    let read = [
        sent.caller.to_hex(),
        sent.destination.to_hex(),
        sent.hash.to_string(),
        sent.position.to_string(),
        sent.arb_block.to_string(),
        sent.eth_block.to_string(),
        sent.timestamp.to_string(),
        sent.value.to_string(),
    ];
    let viem = ["caller", "destination", "hash", "position", "arbBlockNum", "ethBlockNum", "timestamp", "callvalue"]
        .map(|key| text(&said[key]).to_owned());
    assert_eq!(read, viem);
    assert!(sent.data.is_empty(), "a plain withdrawal calls nothing");
    assert_eq!(Withdrawal::in_logs(std::slice::from_ref(&log)).unwrap(), [sent]);

    // A deposit on HyperEVM: the bridge's and the inbox's logs joined into the deposit Elysium runs.
    let (delivered, said) = live("MessageDelivered");
    let (data, _) = live("InboxMessageDelivered");
    let contracts = df_elysium_core::Network::Testnet.contracts();
    let messages = message::Message::in_logs(&[delivered, data], &contracts).unwrap();
    let [message] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(message.number.to_string(), text(&said["messageIndex"]));
    assert_eq!(
        (message.kind, message.sender.to_hex()),
        (message::MessageKind::EthDeposit, text(&said["sender"]).to_owned())
    );
    let deposit = df_elysium_core::bridge::Deposit::from_message(message).unwrap();
    let recorded =
        file["deposits"].as_array().unwrap().iter().find(|case| case["messageNumber"] == said["messageIndex"]);
    let recorded = recorded.unwrap();
    assert_eq!(
        deposit.tx_hash(99_801),
        word(text(&recorded["txHash"])),
        "the hash @arbitrum/sdk computes, and the chain's"
    );
    assert_eq!(
        (deposit.to.to_hex(), deposit.value.to_string()),
        (text(&recorded["to"]).to_ascii_lowercase(), text(&recorded["value"]).to_owned())
    );

    // A confirmed assertion on HyperEVM: the Elysium block and send root it names.
    let (log, said) = live("AssertionCreated");
    let confirmed = claim::read_assertion_created(&log).unwrap();
    let words = &said["assertion"]["afterState"]["globalState"]["bytes32Vals"];
    assert_eq!(confirmed.assertion, word(text(&said["assertionHash"])));
    assert_eq!((confirmed.block_hash, confirmed.send_root), (word(text(&words[0])), word(text(&words[1]))));
}
