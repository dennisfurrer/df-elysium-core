//! Calldata against viem's `encodeFunctionData` with the ABIs of @arbitrum/nitro-contracts 3.2.0,
//! Multicall3 and ERC-20, for the parent chain's rollup contracts (Inbox, Outbox, the BoLD rollup),
//! Multicall3 and an ERC-20, each built by the crate's own builder for that call. One of the Outbox
//! calls is a real claim sent on HyperEVM testnet. `sendL2Message` and
//! `getAssertionCreationBlockForLogLookup` have no builder, so they go through [`Method`] alone.

use df_elysium_core::bridge::Withdrawal;
use df_elysium_core::bridge::claim;
use df_elysium_core::bridge::deposit::{DEPOSIT_ETH, deposit_wei};
use df_elysium_core::bridge::retryable::{self, Retryable, UNSAFE_CREATE_RETRYABLE_TICKET};
use df_elysium_core::client::{Arg, Method};
use df_elysium_core::transfer::{erc20, multicall};
use df_elysium_core::{B256, Network, U256};
use serde_json::Value;

use crate::support::{address, arbitrum_sdk, bytes, number, text, word};

/// A stand-in token: viem's ERC-20 vectors name no contract.
const TOKEN: &str = "0x7d29d8047b905000459c0e80c34a26ceedcb47b2";

fn small(value: &Value) -> u64 {
    number(value).to_u64().unwrap()
}

fn retryable(args: &[Value]) -> Retryable {
    Retryable {
        to: address(text(&args[0])),
        l2_call_value: number(&args[1]),
        max_submission_cost: number(&args[2]),
        excess_fee_refund_address: address(text(&args[3])),
        call_value_refund_address: address(text(&args[4])),
        gas_limit: number(&args[5]),
        max_fee_per_gas: number(&args[6]),
        data: bytes(&args[7]),
    }
}

/// The crate's calldata for one vector, and the method it calls.
fn built(contract: &str, function: &str, args: &[Value]) -> (Vec<u8>, Option<Method>) {
    let network = Network::Testnet;
    let contracts = network.contracts();
    let token = address(TOKEN);
    match (contract, function) {
        ("Inbox", "depositEth") => (deposit_wei(network, U256::ZERO).data, Some(DEPOSIT_ETH)),
        ("Inbox", "createRetryableTicket") => {
            (retryable(args).call(network).unwrap().data, Some(retryable::CREATE_RETRYABLE_TICKET))
        }
        ("Inbox", "unsafeCreateRetryableTicket") => {
            let ticket = retryable(args);
            let data = UNSAFE_CREATE_RETRYABLE_TICKET.calldata(&[
                Arg::Address(ticket.to),
                Arg::Uint(ticket.l2_call_value),
                Arg::Uint(ticket.max_submission_cost),
                Arg::Address(ticket.excess_fee_refund_address),
                Arg::Address(ticket.call_value_refund_address),
                Arg::Uint(ticket.gas_limit),
                Arg::Uint(ticket.max_fee_per_gas),
                Arg::Bytes(&ticket.data),
            ]);
            (data, Some(UNSAFE_CREATE_RETRYABLE_TICKET))
        }
        ("Inbox", "calculateRetryableSubmissionFee") => {
            let length = usize::try_from(small(&args[0])).unwrap();
            let read = retryable::submission_fee_read(network, length, number(&args[1]));
            assert_eq!(read.call.to, Some(contracts.inbox));
            (read.call.data, Some(retryable::CALCULATE_RETRYABLE_SUBMISSION_FEE))
        }
        ("Inbox", "sendL2Message") => {
            let method = Method::new("sendL2Message(bytes)", "uint256");
            (method.calldata(&[Arg::Bytes(&bytes(&args[0]))]), Some(method))
        }
        ("Outbox", "executeTransaction") => {
            let proof: Vec<B256> = args[0].as_array().unwrap().iter().map(|one| word(text(one))).collect();
            let withdrawal = Withdrawal {
                caller: address(text(&args[2])),
                destination: address(text(&args[3])),
                hash: U256::ZERO,
                position: small(&args[1]),
                arb_block: small(&args[4]),
                eth_block: small(&args[5]),
                timestamp: small(&args[6]),
                value: number(&args[7]),
                data: bytes(&args[8]),
            };
            let call = claim::execute(&contracts, &withdrawal, &proof);
            assert_eq!((call.to, call.value), (Some(contracts.outbox), U256::ZERO));
            (call.data, Some(claim::EXECUTE_TRANSACTION))
        }
        ("Outbox", "isSpent") => (claim::is_spent(&contracts, small(&args[0])).call.data, Some(claim::IS_SPENT)),
        ("Outbox", "roots") => (claim::root(&contracts, word(text(&args[0]))).call.data, Some(claim::ROOTS)),
        ("Rollup", "latestConfirmed") => (claim::latest_confirmed(&contracts).call.data, Some(claim::LATEST_CONFIRMED)),
        ("Rollup", "getAssertion") => {
            (claim::assertion(&contracts, word(text(&args[0]))).call.data, Some(claim::GET_ASSERTION))
        }
        ("Rollup", "confirmPeriodBlocks") => {
            (claim::confirm_period_blocks(&contracts).call.data, Some(claim::CONFIRM_PERIOD_BLOCKS))
        }
        ("Rollup", "getAssertionCreationBlockForLogLookup") => {
            let method = Method::new("getAssertionCreationBlockForLogLookup(bytes32)", "uint256");
            (method.calldata(&[Arg::Word(word(text(&args[0])))]), Some(method))
        }
        ("Multicall3", "aggregate3") => {
            let calls: Vec<multicall::Call3> = args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|one| multicall::Call3 {
                    target: address(text(&one[0])),
                    allow_failure: text(&one[1]) == "true",
                    data: bytes(&one[2]),
                })
                .collect();
            (multicall::aggregate3(&calls).call.data, Some(multicall::AGGREGATE3))
        }
        ("ERC20", "allowance") => {
            let read = erc20::allowance(token, address(text(&args[0])), address(text(&args[1])));
            (read.call.data, Some(erc20::ALLOWANCE))
        }
        ("ERC20", "transferFrom") => {
            let call = erc20::transfer_from(token, address(text(&args[0])), address(text(&args[1])), number(&args[2]));
            assert_eq!(call.to, Some(token));
            (call.data, Some(erc20::TRANSFER_FROM))
        }
        ("ERC20", "totalSupply") => (erc20::total_supply(token).call.data, Some(erc20::TOTAL_SUPPLY)),
        ("ERC20", "name") => (erc20::name(token).call.data, Some(erc20::NAME)),
        _ => panic!("no builder for {contract}.{function}"),
    }
}

#[test]
fn every_rollup_multicall_and_token_call_is_viem_s_calldata_byte_for_byte() {
    let file = arbitrum_sdk();
    let mut checked = 0;
    for case in file["calldata"].as_array().unwrap() {
        let contract = text(&case["contract"]);
        if !["Inbox", "Outbox", "Rollup", "Multicall3", "ERC20"].contains(&contract) {
            continue;
        }
        let signature = text(&case["signature"]);
        let function = signature.trim_start_matches("function ").split('(').next().unwrap();
        let (data, method) = built(contract, function, case["args"].as_array().unwrap());
        assert_eq!(data, bytes(&case["calldata"]), "{signature}");
        if let Some(method) = method {
            assert_eq!(method.selector().to_vec(), bytes(&case["selector"]), "{signature}: selector");
        }
        checked += 1;
    }
    assert_eq!(checked, 20, "every Inbox, Outbox, Rollup, Multicall3 and ERC-20 vector");
}

#[test]
fn the_real_claim_sent_on_hyperevm_is_the_crate_s_execute() {
    let file = arbitrum_sdk();
    let real = file["calldata"].as_array().unwrap().iter().find(|case| case.get("live").is_some()).unwrap();
    assert!(text(&real["signature"]).contains("executeTransaction"));
    let (data, _) = built("Outbox", "executeTransaction", real["args"].as_array().unwrap());
    assert_eq!(data, bytes(&real["calldata"]), "{}", real["live"]);
}
