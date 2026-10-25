//! The bridge, read only, across Elysium testnet and HyperEVM testnet: the rollup's contracts are
//! the ones the crate names, the latest confirmed assertion names a block Elysium has, a recent
//! real withdrawal is claimed and its call simulated on HyperEVM, where it must run, and a deposit
//! is simulated there from an address that holds HYPE.

use df_elysium_core::bridge::claim::{self, Claim};
use df_elysium_core::bridge::{Withdrawal, deposit_wei};
use df_elysium_core::client::read::Read;
use df_elysium_core::client::{BlockTag, CallRequest, LogFilter, Method};
use df_elysium_core::nitro::ARB_SYS;
use df_elysium_core::nitro::sys::L2_TO_L1_TX;
use df_elysium_core::utils::Address;
use df_elysium_core::{Network, U256};
use df_hyperliquid_core::client::evm::abi::{self, AbiError, Token};

use crate::support::address;
use crate::tap::{parent, reader};

/// HyperEVM's HYPE system address, which holds HYPE on the parent chain: a sender for simulations.
const RICH: &str = "0x2222222222222222222222222222222222222222";

fn an_address(answer: &[u8]) -> Result<Address, AbiError> {
    abi::decode(&abi::params("address")?, answer)?
        .first()
        .and_then(Token::as_address)
        .ok_or(AbiError::Short(answer.len()))
}

#[tokio::test]
#[ignore = "testnet"]
async fn the_rollup_names_the_contracts_the_crate_names() {
    let parent = parent();
    let contracts = Network::Testnet.contracts();
    for (getter, expected) in [
        ("inbox()", contracts.inbox),
        ("outbox()", contracts.outbox),
        ("bridge()", contracts.bridge),
        ("sequencerInbox()", contracts.sequencer_inbox),
        ("rollupEventInbox()", contracts.rollup_event_inbox),
        ("challengeManager()", contracts.challenge_manager),
    ] {
        let read = Read::new(contracts.rollup, &Method::new(getter, "address"), &[], an_address);
        assert_eq!(read.on(&parent, BlockTag::Latest).await.unwrap(), expected, "{getter}");
    }
    assert_eq!(claim::confirm_period_blocks(&contracts).on(&parent, BlockTag::Latest).await, Ok(10));
}

#[tokio::test]
#[ignore = "testnet"]
async fn a_recent_withdrawal_is_claimed_and_its_call_runs_on_hyperevm() {
    let (client, parent) = (reader(), parent());
    let contracts = Network::Testnet.contracts();
    let confirmed = claim::latest(&contracts, &parent).await.unwrap();
    let block = client.block_by_hash(confirmed.block_hash, false).await.unwrap().unwrap();
    assert_eq!(block.send_root, confirmed.send_root, "the assertion's root is the block's");

    let head = client.block_number().await.unwrap();
    let filter = LogFilter {
        from_block: Some(BlockTag::Number(head - 30_000)),
        to_block: Some(BlockTag::Number(head)),
        address: vec![ARB_SYS],
        topics: vec![Some(vec![L2_TO_L1_TX.topic()])],
        ..LogFilter::default()
    };
    let withdrawals = Withdrawal::in_logs(&client.logs(&filter).await.unwrap()).unwrap();
    assert!(!withdrawals.is_empty(), "sends to HyperEVM are common on testnet");
    let (mut spent, mut waiting) = (0, 0);
    for withdrawal in &withdrawals {
        match claim::claim(&client, &parent, withdrawal).await.unwrap() {
            Claim::Spent => spent += 1,
            Claim::Waiting { confirmed } => {
                assert!(withdrawal.position >= confirmed);
                waiting += 1;
                break;
            }
            Claim::Ready { call, .. } => {
                assert_eq!(call.to, Some(contracts.outbox));
                let simulated = parent.call(&CallRequest::of(Some(address(RICH)), &call), BlockTag::Latest).await;
                assert_eq!(simulated, Ok(Vec::new()), "position {} runs on the outbox", withdrawal.position);
                assert!(!claim::is_spent(&contracts, withdrawal.position).on(&parent, BlockTag::Latest).await.unwrap());
                return;
            }
        }
    }
    panic!("no claimable withdrawal among {}: {spent} spent, {waiting} waiting", withdrawals.len());
}

#[tokio::test]
#[ignore = "testnet"]
async fn a_deposit_simulated_on_hyperevm_answers_its_message_number() {
    let parent = parent();
    let call = deposit_wei(Network::Testnet, U256::from(1_000_000_000_000_000_000u64));
    let answer = parent.call(&CallRequest::of(Some(address(RICH)), &call), BlockTag::Latest).await.unwrap();
    let number = U256::from_be_slice(&answer).unwrap();
    assert!(number > U256::from(1_493_518u64), "past the recorded deposit's message: {number}");
    let gas = parent.estimate_gas(&CallRequest::of(Some(address(RICH)), &call)).await.unwrap();
    assert!(gas > 21_000 && gas < 200_000, "{gas}");
}
