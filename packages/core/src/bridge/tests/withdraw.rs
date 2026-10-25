//! Withdrawals against viem's `encodeFunctionData` and `decodeEventLog` (`arbitrum_sdk.json`) and
//! a live withdrawal's receipt (`elysium_testnet.json`), its send read from the `L2ToL1Tx` log.

use crate::client::Log;
use crate::nitro::table::ARB_SYS;
use crate::test_support::{self, address, logs, result, step, text};
use crate::utils::hex;

use super::*;

#[test]
fn a_withdrawal_is_withdraweth_on_arbsys_with_the_hype_as_its_value() {
    let file = test_support::arbitrum_sdk();
    let entry = file["calldata"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["contract"] == "ArbSys" && text(&entry["signature"]).starts_with("function withdrawEth("))
        .unwrap()
        .clone();
    let destination = address(text(&entry["args"][0]));
    let call = withdraw(destination, Dec::parse("0.002").unwrap()).unwrap();
    assert_eq!(hex::encode_prefixed(&call.data), text(&entry["calldata"]));
    assert_eq!((call.to, call.value), (Some(ARB_SYS), U256::from(2_000_000_000_000_000u64)));
    assert_eq!(withdraw(destination, Dec::parse("0.000000000000000001").unwrap()).unwrap().value, U256::ONE);
    assert!(withdraw(destination, Dec::parse("-0.002").unwrap()).is_err());
    assert!(withdraw(destination, Dec::ZERO).is_err());
}

/// The recorded withdrawal of 0.002 HYPE at leaf 746935.
#[test]
fn a_receipt_reads_into_the_send_its_log_records() {
    let receipt = step("withdrawal", "receipt");
    let sends = Withdrawal::in_logs(&logs(&receipt)).unwrap();
    let [send] = sends.as_slice() else { panic!("one send: {sends:?}") };
    let transaction = result(&step("withdrawal", "transaction"));
    assert_eq!(send.caller, address(text(&transaction["from"])));
    assert_eq!(send.position, 746_935);
    assert_eq!(send.value, U256::from(2_000_000_000_000_000u64));
    assert_eq!(send.value, text(&transaction["value"]).parse().unwrap(), "what the transaction carried");
    let block = result(&step("withdrawal", "block"));
    assert_eq!(Some(send.arb_block), U256::from_quantity(text(&block["number"])).unwrap().to_u64());
    assert_eq!(Some(send.eth_block), U256::from_quantity(text(&block["l1BlockNumber"])).unwrap().to_u64());
    assert_eq!(Some(send.timestamp), U256::from_quantity(text(&block["timestamp"])).unwrap().to_u64());
    assert!(send.data.is_empty(), "a plain withdrawal calls nothing");
    let leaf = text(&test_support::elysium()["journeys"]["withdrawal"]["item_hash"]["answer"]).to_owned();
    assert_eq!(send.leaf().to_hex(), text(&result(&leaf)), "the outbox's own item hash");
}

/// A live `L2ToL1Tx` as viem decoded it, and a log that is not one.
#[test]
fn a_send_reads_as_viem_read_it_and_nothing_else_reads_as_one() {
    let file = test_support::arbitrum_sdk();
    let event = file["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| text(&event["topic"]).starts_with("0x3e7aafa7"))
        .unwrap()
        .clone();
    let log: Log = serde_json::from_value(event["log"].clone()).unwrap();
    let send = Withdrawal::from_log(&log).unwrap();
    let said = &event["decoded"];
    assert_eq!(send.caller.to_hex(), text(&said["caller"]));
    assert_eq!(send.destination.to_hex(), text(&said["destination"]));
    assert_eq!(send.hash.to_string(), text(&said["hash"]));
    assert_eq!(send.position.to_string(), text(&said["position"]));
    assert_eq!(send.arb_block.to_string(), text(&said["arbBlockNum"]));
    assert_eq!(send.eth_block.to_string(), text(&said["ethBlockNum"]));
    assert_eq!(send.timestamp.to_string(), text(&said["timestamp"]));
    assert_eq!(send.value.to_string(), text(&said["callvalue"]));
    assert_eq!(hex::encode_prefixed(&send.data), text(&said["data"]));
    let elsewhere = Log { address: Address::ZERO, ..log.clone() };
    assert!(Withdrawal::from_log(&elsewhere).is_err(), "only ArbSys writes them");
    let update = file["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| text(&event["topic"]).starts_with("0xe9e13da3"))
        .unwrap()
        .clone();
    let update: Log = serde_json::from_value(update["log"].clone()).unwrap();
    assert!(Withdrawal::from_log(&update).is_err(), "a tree update is not a send");
    assert_eq!(Withdrawal::in_logs(&[update, log]).unwrap(), [send]);
}
