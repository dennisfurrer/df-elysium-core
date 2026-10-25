//! Deposits against @arbitrum/sdk 4.1.0's `EthDepositMessage.calculateDepositTxId`
//! (`arbitrum_sdk.json`, one of them a live deposit), and a live deposit journey
//! (`elysium_testnet.json`): the parent chain's receipt read into its message and the deposit's
//! hash on Elysium, which is the hash of the transaction Elysium ran.

use crate::client::Network;
use crate::client::records::{Transaction, TxKind};
use crate::test_support::{self, address, logs, result, step, text};
use crate::utils::hex;

use super::*;

fn number(value: &serde_json::Value) -> U256 {
    text(value).parse().unwrap()
}

#[test]
fn every_deposit_hashes_as_the_sdk_hashes_it() {
    let file = test_support::arbitrum_sdk();
    let cases = file["deposits"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let deposit = Deposit {
            message_number: number(&case["messageNumber"]),
            from: address(text(&case["from"])),
            to: address(text(&case["to"])),
            value: number(&case["value"]),
        };
        let chain: u64 = text(&case["chainId"]).parse().unwrap();
        assert_eq!(deposit.tx_hash(chain).to_hex(), text(&case["txHash"]), "message {}", deposit.message_number);
    }
}

/// HyperEVM testnet's receipt for an `Inbox.depositEth()` and the deposit Elysium ran for it.
#[test]
fn a_parent_receipt_reads_into_the_deposit_elysium_ran() {
    let contracts = Network::Testnet.contracts();
    let messages = Message::in_logs(&logs(&step("deposit", "parent_receipt")), &contracts).unwrap();
    let [message] = messages.as_slice() else { panic!("one message: {messages:?}") };
    assert_eq!(message.kind, MessageKind::EthDeposit);
    assert_eq!(message.number, U256::from(1_493_518u64));
    let deposit = Deposit::from_message(message).unwrap();
    let ran: Transaction = serde_json::from_value(result(&step("deposit", "transaction"))).unwrap();
    assert_eq!(ran.tx_kind(), TxKind::Deposit);
    assert_eq!(ran.message_number(), Some(deposit.message_number), "its requestId is the message's number");
    assert_eq!((deposit.from, Some(deposit.to), deposit.value), (ran.from, ran.to, ran.value));
    assert_eq!(deposit.tx_hash(Network::Testnet.chain_id()), ran.hash, "known before it lands");
    let receipt = result(&step("deposit", "receipt"));
    assert_eq!(text(&receipt["transactionHash"]), ran.hash.to_hex());
    assert_eq!(text(&receipt["status"]), "0x1");
    let parent = result(&step("deposit", "parent_transaction"));
    assert_eq!(address(text(&parent["to"])), contracts.inbox);
    assert_eq!(number(&parent["value"]), deposit.value, "the HYPE the parent transaction carried");
}

#[test]
fn a_deposit_is_depositeth_on_the_inbox_with_the_hype_as_its_value() {
    let file = test_support::arbitrum_sdk();
    let entry = file["calldata"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["contract"] == "Inbox" && text(&entry["signature"]).starts_with("function depositEth("))
        .unwrap()
        .clone();
    let call = deposit(Network::Testnet, Dec::parse("0.0135").unwrap()).unwrap();
    assert_eq!(hex::encode_prefixed(&call.data), text(&entry["calldata"]));
    assert_eq!(call.to, Some(Network::Testnet.contracts().inbox));
    assert_eq!(call.value, U256::from(13_500_000_000_000_000u64));
    assert_eq!(deposit_wei(Network::Testnet, U256::ONE).data, call.data);
    let wei = deposit(Network::Testnet, Dec::parse("0.000000000000000001").unwrap()).unwrap();
    assert_eq!(wei.value, U256::ONE, "eighteen places, exactly");
    assert!(Dec::parse("0.0000000000000000001").is_err(), "a nineteenth place never becomes an amount");
    assert!(deposit(Network::Testnet, Dec::ZERO).is_err());
    assert!(deposit(Network::Testnet, Dec::parse("-1").unwrap()).is_err());
}

/// A message of another kind, or with no data, makes no deposit.
#[test]
fn only_a_deposit_message_with_its_data_is_a_deposit() {
    let messages = Message::in_logs(&logs(&step("deposit", "parent_receipt")), &Network::Testnet.contracts()).unwrap();
    let mut other = messages[0].clone();
    other.kind = MessageKind::SubmitRetryable;
    assert_eq!(Deposit::from_message(&other), None);
    let mut short = messages[0].clone();
    short.data = Some(vec![0; 20]);
    assert_eq!(Deposit::from_message(&short), None);
    short.data = None;
    assert_eq!(Deposit::from_message(&short), None);
    for byte in [3, 7, 9, 11, 12, 13, 42] {
        assert_eq!(MessageKind::from_byte(byte).byte(), byte);
    }
}
