//! The fee cap and an estimate's costs, against the NodeInterface's live split of a HYPE transfer
//! on Elysium testnet (`elysium_testnet.json`) and integer arithmetic done by hand.

use crate::nitro::node::GAS_ESTIMATE_COMPONENTS;
use crate::test_support::{elysium, text};

use super::*;

#[test]
fn the_fee_cap_is_twice_the_base_and_there_is_no_tip() {
    let fees = fees(U256::from_u64(100_020_000));
    assert_eq!((fees.max_priority_fee, fees.max_fee), (U256::ZERO, U256::from_u64(200_040_000)));
    assert_eq!(super::fees(U256::MAX).max_fee, U256::MAX, "it saturates, never wraps");
    // 21,173 gas at the cap, and 0.5 HYPE.
    let spent = most_spent(21_173, fees, U256::from_u64(500_000_000_000_000_000)).unwrap();
    assert_eq!(spent, Dec::parse("0.50000423544692").unwrap());
}

#[test]
fn a_live_estimate_splits_into_execution_and_data() {
    let file = elysium();
    let call = file["nitro"].as_array().unwrap().iter().find(|call| call["name"] == "gasEstimateComponents").unwrap();
    let answer: serde_json::Value = serde_json::from_str(text(&call["answer"])).unwrap();
    let to: Address = "0x8499e50474b6e8c988b232f52583439abf37a8a1".parse().unwrap();
    let from: Address = "0xa16a365ae8f34edadb8157849d4ace274bba9164".parse().unwrap();
    let read = components(Some(from), &Call { to: Some(to), value: U256::ONE, data: Vec::new() });
    assert_eq!(
        crate::utils::hex::encode_prefixed(&read.call.data[..4]),
        crate::utils::hex::encode_prefixed(&GAS_ESTIMATE_COMPONENTS.selector())
    );
    let estimate = read.decode(&crate::utils::hex::decode(text(&answer["result"])).unwrap()).unwrap();
    assert!(estimate.gas > 21_000 && estimate.gas_for_l1 > 0, "{estimate:?}");
    assert_eq!(estimate.gas_for_l2() + estimate.gas_for_l1, estimate.gas);
    assert_eq!(estimate.cost(), estimate.base_fee.checked_mul(U256::from_u64(estimate.gas)).unwrap());
    assert_eq!(estimate.cost_for_l1(), estimate.base_fee.checked_mul(U256::from_u64(estimate.gas_for_l1)).unwrap());
    assert_eq!(estimate.hype().unwrap(), crate::utils::hype(estimate.cost()).unwrap());
    assert!(estimate.hype().unwrap() < Dec::parse("0.00001").unwrap(), "a transfer costs a few millionths of a HYPE");
}
