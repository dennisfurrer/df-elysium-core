//! ArbRetryableTx's typed reads against Elysium testnet's answers (`elysium_testnet.json`), a
//! missing ticket's revert as the node wrote it, its calls against viem's `encodeFunctionData` and
//! its events against live logs and viem's `decodeEventLog` (`arbitrum_sdk.json`).

use crate::client::Log;
use crate::client::rpc::NodeError;
use crate::nitro::support::{decoded, number, recorded, sdk_arg, sdk_data, sdk_event, sdk_one, sent, word};
use crate::test_support::text;
use crate::utils::hex;

use super::*;

#[test]
fn a_live_ticket_reads_its_timeout_and_beneficiary_and_a_week_s_lifetime() {
    assert_eq!(sent(&lifetime(), "ArbRetryableTx", "getLifetime", 0), U256::from(604_800u64), "a week");
    assert_eq!(sent(&current_redeemer(), "ArbRetryableTx", "getCurrentRedeemer", 0), Address::ZERO, "outside a redeem");
    let ticket = word(&recorded("ArbRetryableTx", "getTimeout", 0), 0);
    assert_eq!(
        sent(&timeout(ticket), "ArbRetryableTx", "getTimeout", 0),
        number(&decoded("ArbRetryableTx", "getTimeout", 0)[0])
    );
    let beneficiary = sent(&beneficiary(ticket), "ArbRetryableTx", "getBeneficiary", 0);
    assert_eq!(beneficiary.to_hex(), text(&decoded("ArbRetryableTx", "getBeneficiary", 0)[0]).to_lowercase());
}

/// `NoTicketWithID()`, as the node refused a read of ticket 1.
#[test]
fn a_ticket_that_is_gone_reverts_with_no_ticket_with_id() {
    assert_eq!(no_ticket_selector(), [0x80, 0x69, 0x84, 0x56]);
    let refused = recorded("ArbRetryableTx", "getTimeout", 1);
    let read = timeout(word(&refused, 0));
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&refused["params"][0]["data"]));
    let answer: serde_json::Value = serde_json::from_str(text(&refused["answer"])).unwrap();
    let error = NodeError {
        code: answer["error"]["code"].as_i64().unwrap(),
        message: text(&answer["error"]["message"]).to_owned(),
        data: answer["error"]["data"].as_str().map(ToOwned::to_owned),
    };
    let gone = ReadError::Rpc(RpcError::Node { method: "eth_call", error: error.clone() });
    assert!(no_ticket(&gone));
    let other = ReadError::Rpc(RpcError::Node {
        method: "eth_call",
        error: NodeError { data: Some("0x6f809c4e".to_owned()), ..error.clone() },
    });
    assert!(!no_ticket(&other), "ProgramNotActivated() is another refusal");
    assert!(!no_ticket(&ReadError::Rpc(RpcError::Node {
        method: "eth_call",
        error: NodeError { data: None, ..error }
    })));
}

#[test]
fn redeem_keepalive_and_cancel_are_viem_s_calldata() {
    for (name, call) in
        [("redeem", redeem as fn(B256) -> crate::client::Call), ("keepalive", keepalive), ("cancel", cancel)]
    {
        let entry = sdk_one("ArbRetryableTx", name);
        let ticket: B256 = sdk_arg(&entry, 0).parse().unwrap();
        let call = call(ticket);
        assert_eq!((call.to, call.value), (Some(ARB_RETRYABLE_TX), U256::ZERO), "{name}");
        assert_eq!(hex::encode_prefixed(&call.data), sdk_data(&entry), "{name}");
    }
    for (name, read) in [
        ("getTimeout", timeout(B256([0x12; 32])).call),
        ("getBeneficiary", beneficiary(B256([0x12; 32])).call),
        ("getLifetime", lifetime().call),
        ("getCurrentRedeemer", current_redeemer().call),
    ] {
        assert_eq!(hex::encode_prefixed(&read.data), sdk_data(&sdk_one("ArbRetryableTx", name)), "{name}");
    }
    let mut answer = [0u8; 32];
    answer[0] = 0xba;
    assert_eq!(redeem_hash(&answer).map(|hash| hash.0), Some(answer), "what a redeem answers");
    assert_eq!(redeem_hash(&[0u8; 31]), None);
}

fn log(event: &serde_json::Value) -> Log {
    serde_json::from_value(event["log"].clone()).unwrap()
}

/// A live ticket's creation and its redeem, and synthetic cancels and extensions.
#[test]
fn each_event_is_logged_under_its_topic_and_reads_as_viem_read_it() {
    for (event, name) in [
        (TICKET_CREATED, "TicketCreated"),
        (LIFETIME_EXTENDED, "LifetimeExtended"),
        (REDEEM_SCHEDULED, "RedeemScheduled"),
        (CANCELED, "Canceled"),
    ] {
        let said = sdk_event("ArbRetryableTx", name);
        assert_eq!(event.topic().to_hex(), text(&said["topic"]), "{name}");
        let tokens = event.decode(&log(&said)).unwrap();
        assert_eq!(hex::encode_prefixed(tokens[0].as_bytes().unwrap()), text(&said["decoded"]["ticketId"]), "{name}");
    }
    let scheduled = sdk_event("ArbRetryableTx", "RedeemScheduled");
    let tokens = REDEEM_SCHEDULED.decode(&log(&scheduled)).unwrap();
    let said = &scheduled["decoded"];
    assert_eq!(hex::encode_prefixed(tokens[1].as_bytes().unwrap()), text(&said["retryTxHash"]));
    assert_eq!(tokens[2].as_u64().unwrap().to_string(), text(&said["sequenceNum"]));
    assert_eq!(tokens[3].as_u64().unwrap().to_string(), text(&said["donatedGas"]));
    assert_eq!(tokens[4].as_address().unwrap().to_hex(), text(&said["gasDonor"]));
    assert_eq!(tokens[5].as_u256().unwrap().to_string(), text(&said["maxRefund"]));
    assert_eq!(tokens[6].as_u256().unwrap().to_string(), text(&said["submissionFeeRefund"]));
    let extended = sdk_event("ArbRetryableTx", "LifetimeExtended");
    let tokens = LIFETIME_EXTENDED.decode(&log(&extended)).unwrap();
    assert_eq!(tokens[1].as_u256().unwrap().to_string(), text(&extended["decoded"]["newTimeout"]));
}
