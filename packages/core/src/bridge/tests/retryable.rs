//! Retryable tickets against @arbitrum/sdk 4.1.0's `ParentToChildMessage.calculateSubmitRetryableId`
//! (`arbitrum_sdk.json`, one of them live) and the Inbox's own fee formula, calls against viem's
//! `encodeFunctionData`, and a live ticket journey (`elysium_testnet.json`): the parent chain's
//! receipt read into its tickets, each id the hash of the submission Elysium ran, and its redeem
//! naming it.

use crate::client::Network;
use crate::client::records::{Transaction, TxKind};
use crate::test_support::{self, address, logs, result, step, text};
use crate::utils::hex;

use super::*;

fn number(value: &serde_json::Value) -> U256 {
    text(value).parse().unwrap()
}

#[test]
fn every_ticket_id_is_the_sdk_s() {
    let file = test_support::arbitrum_sdk();
    let cases = file["retryables"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let inputs = &case["inputs"];
        let ticket = Ticket {
            message_number: number(&inputs["messageNumber"]),
            from: address(text(&inputs["fromAddress"])),
            l1_base_fee: number(&inputs["l1BaseFee"]),
            deposit: number(&inputs["l1Value"]),
            retryable: Retryable {
                to: address(text(&inputs["destAddress"])),
                l2_call_value: number(&inputs["l2CallValue"]),
                max_submission_cost: number(&inputs["maxSubmissionFee"]),
                excess_fee_refund_address: address(text(&inputs["excessFeeRefundAddress"])),
                call_value_refund_address: address(text(&inputs["callValueRefundAddress"])),
                gas_limit: number(&inputs["gasLimit"]),
                max_fee_per_gas: number(&inputs["maxFeePerGas"]),
                data: hex::decode(text(&inputs["data"])).unwrap(),
            },
        };
        let chain: u64 = text(&inputs["chainId"]).parse().unwrap();
        assert_eq!(ticket.id(chain).to_hex(), text(&case["ticketId"]), "message {}", ticket.message_number);
    }
}

#[test]
fn the_submission_fee_is_the_inbox_s_formula() {
    let file = test_support::arbitrum_sdk();
    for case in file["submissionFees"].as_array().unwrap() {
        let length: usize = text(&case["dataLength"]).parse().unwrap();
        assert_eq!(submission_fee(length, number(&case["baseFee"])), number(&case["fee"]), "{length} bytes");
    }
    assert_eq!(submission_fee(usize::MAX, U256::MAX), U256::MAX, "it saturates, never wraps");
    // The inbox's own answers on HyperEVM testnet: at the base fee named, and at zero, which
    // stands for the block's and is zero inside a call.
    let journeys = test_support::elysium();
    let simulated = &journeys["journeys"]["simulated_deposit"];
    let answered = |step: &str| U256::from_quantity(text(&result(text(&simulated[step]["answer"])))).unwrap();
    let asked = test_support::bytes(&simulated["submission_fee_100"]["params"][0]["data"]);
    let named = U256::from_be_slice(&asked[36..68]).unwrap();
    let read = submission_fee_read(Network::Testnet, 100, named);
    assert_eq!(read.call.data, asked);
    assert_eq!(read.call.to, Some(Network::Testnet.contracts().inbox));
    assert_eq!(answered("submission_fee_100"), submission_fee(100, named));
    assert_eq!(answered("submission_fee_100_at_zero"), U256::ZERO);
}

fn sdk_calldata(name: &str) -> Vec<serde_json::Value> {
    let file = test_support::arbitrum_sdk();
    let prefix = format!("function {name}(");
    file["calldata"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["contract"] == "Inbox" && text(&entry["signature"]).starts_with(&prefix))
        .cloned()
        .collect()
}

fn retryable_of(args: &[serde_json::Value]) -> Retryable {
    Retryable {
        to: address(text(&args[0])),
        l2_call_value: number(&args[1]),
        max_submission_cost: number(&args[2]),
        excess_fee_refund_address: address(text(&args[3])),
        call_value_refund_address: address(text(&args[4])),
        gas_limit: number(&args[5]),
        max_fee_per_gas: number(&args[6]),
        data: hex::decode(text(&args[7])).unwrap(),
    }
}

#[test]
fn a_ticket_is_createretryableticket_on_the_inbox_carrying_its_deposit() {
    let cases = sdk_calldata("createRetryableTicket");
    assert_eq!(cases.len(), 2);
    for case in &cases {
        let ticket = retryable_of(case["args"].as_array().unwrap());
        let call = ticket.call(Network::Testnet).unwrap();
        assert_eq!(hex::encode_prefixed(&call.data), text(&case["calldata"]));
        assert_eq!(call.to, Some(Network::Testnet.contracts().inbox));
        let gas = ticket.gas_limit.checked_mul(ticket.max_fee_per_gas).unwrap();
        let deposit = gas.checked_add(ticket.max_submission_cost).unwrap().checked_add(ticket.l2_call_value).unwrap();
        assert_eq!(call.value, deposit, "the submission, the call's value and all its gas at the cap");
        assert_eq!(ticket.deposit(), Some(deposit));
    }
    let unsafe_case = &sdk_calldata("unsafeCreateRetryableTicket")[0];
    let ticket = retryable_of(unsafe_case["args"].as_array().unwrap());
    let args = [
        crate::client::Arg::Address(ticket.to),
        crate::client::Arg::Uint(ticket.l2_call_value),
        crate::client::Arg::Uint(ticket.max_submission_cost),
        crate::client::Arg::Address(ticket.excess_fee_refund_address),
        crate::client::Arg::Address(ticket.call_value_refund_address),
        crate::client::Arg::Uint(ticket.gas_limit),
        crate::client::Arg::Uint(ticket.max_fee_per_gas),
        crate::client::Arg::Bytes(&ticket.data),
    ];
    assert_eq!(hex::encode_prefixed(&UNSAFE_CREATE_RETRYABLE_TICKET.calldata(&args)), text(&unsafe_case["calldata"]));
    let overflowing = Retryable { gas_limit: U256::MAX, max_fee_per_gas: U256::from(2u8), ..Retryable::default() };
    assert_eq!((overflowing.deposit(), overflowing.call(Network::Testnet)), (None, None), "past 256 bits");
    for case in sdk_calldata("calculateRetryableSubmissionFee") {
        let length: usize = text(&case["args"][0]).parse().unwrap();
        let read = submission_fee_read(Network::Testnet, length, number(&case["args"][1]));
        assert_eq!(hex::encode_prefixed(&read.call.data), text(&case["calldata"]), "{length} bytes");
    }
}

/// HyperEVM testnet's receipt for a transaction that made tickets, and the submission and redeem
/// Elysium ran for the one recorded.
#[test]
fn a_parent_receipt_reads_into_the_tickets_elysium_ran() {
    let contracts = Network::Testnet.contracts();
    let messages = Message::in_logs(&logs(&step("retryable", "parent_receipt")), &contracts).unwrap();
    assert!(!messages.is_empty());
    let submitted: Transaction = serde_json::from_value(result(&step("retryable", "submit_transaction"))).unwrap();
    assert_eq!(submitted.tx_kind(), TxKind::SubmitRetryable);
    let tickets: Vec<Ticket> = messages.iter().filter_map(Ticket::from_message).collect();
    assert_eq!(tickets.len(), messages.len(), "every message is a ticket");
    let ticket = tickets.iter().find(|ticket| Some(ticket.message_number) == submitted.message_number()).unwrap();
    assert_eq!(ticket.message_number, U256::from(1_493_519u64));
    assert_eq!(ticket.id(Network::Testnet.chain_id()), submitted.hash, "a ticket's id is its submission's hash");
    assert_eq!(Some(ticket.from), Some(submitted.from), "the sender's alias");
    assert_eq!(Some(ticket.l1_base_fee), submitted.l1_base_fee);
    assert_eq!(Some(ticket.deposit), submitted.deposit_value);
    assert_eq!(Some(ticket.retryable.to), submitted.retry_to);
    assert_eq!(Some(ticket.retryable.l2_call_value), submitted.retry_value);
    assert_eq!(Some(&ticket.retryable.data), submitted.retry_data.as_ref());
    assert_eq!(Some(ticket.retryable.call_value_refund_address), submitted.beneficiary);
    assert_eq!(Some(ticket.retryable.max_submission_cost), submitted.max_submission_fee);
    assert_eq!(Some(ticket.retryable.max_fee_per_gas), submitted.max_fee_per_gas);
    assert_eq!(U256::from(submitted.gas), ticket.retryable.gas_limit);
    let redeem: Transaction = serde_json::from_value(result(&step("retryable", "retry_transaction"))).unwrap();
    assert_eq!(redeem.tx_kind(), TxKind::Retry);
    assert_eq!(redeem.ticket_id, Some(ticket.id(Network::Testnet.chain_id())), "the redeem names its ticket");
    assert_eq!(redeem.refund_to, Some(ticket.retryable.excess_fee_refund_address));
    assert_eq!((redeem.to, &redeem.input), (Some(ticket.retryable.to), &ticket.retryable.data), "the call it runs");
    // A message of another kind, or one cut short, makes no ticket.
    let mut other = messages[0].clone();
    other.kind = MessageKind::EthDeposit;
    assert_eq!(Ticket::from_message(&other), None);
    let mut short = messages[0].clone();
    short.data.as_mut().unwrap().pop();
    assert_eq!(Ticket::from_message(&short), None);
}
