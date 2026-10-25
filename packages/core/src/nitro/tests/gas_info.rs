//! ArbGasInfo's typed reads against Elysium testnet's answers (`elysium_testnet.json`, cast's
//! decoding beside them), against viem's `decodeFunctionResult` of a live `getPricesInWei`, and its
//! calls against viem's `encodeFunctionData` (`arbitrum_sdk.json`).

use crate::nitro::support::{decoded, number, sdk_answer, sdk_data, sdk_one, sent};
use crate::test_support::{bytes, text};
use crate::utils::hex;

use super::*;

fn n(name: &str) -> U256 {
    number(&decoded("ArbGasInfo", name, 0)[0])
}

#[test]
fn prices_read_in_wei_and_in_gas_as_the_node_answered_them() {
    let prices = sent(&prices_in_wei(), "ArbGasInfo", "getPricesInWei", 0);
    let said = decoded("ArbGasInfo", "getPricesInWei", 0);
    let fields = [
        prices.per_l2_tx,
        prices.per_l1_calldata_unit,
        prices.per_storage_allocation,
        prices.per_arb_gas_base,
        prices.per_arb_gas_congestion,
        prices.per_arb_gas_total,
    ];
    assert_eq!(fields.to_vec(), said.iter().map(number).collect::<Vec<_>>());
    assert_eq!(prices.per_arb_gas_total, U256::from(100_000_000u64), "the 0.1 gwei floor, no congestion");
    assert_eq!(
        prices.per_arb_gas_base.checked_add(prices.per_arb_gas_congestion),
        Some(prices.per_arb_gas_total),
        "the base fee is the floor and what congestion adds"
    );
    let poster = Address::parse_const("0xa4b000000000000000000073657175656e636572").unwrap();
    let through = sent(&prices_in_wei_with(poster), "ArbGasInfo", "getPricesInWeiWithAggregator", 0);
    assert_eq!(through.per_arb_gas_total, number(&decoded("ArbGasInfo", "getPricesInWeiWithAggregator", 0)[5]));
    let in_gas = sent(&prices_in_arb_gas(), "ArbGasInfo", "getPricesInArbGas", 0);
    let said = decoded("ArbGasInfo", "getPricesInArbGas", 0);
    assert_eq!(
        [in_gas.per_l2_tx, in_gas.per_l1_calldata_unit, in_gas.per_storage_allocation].to_vec(),
        said.iter().map(number).collect::<Vec<_>>()
    );
    assert_eq!(sent(&minimum_gas_price(), "ArbGasInfo", "getMinimumGasPrice", 0), n("getMinimumGasPrice"));
    assert_eq!(sent(&l1_base_fee_estimate(), "ArbGasInfo", "getL1BaseFeeEstimate", 0), n("getL1BaseFeeEstimate"));
    assert_eq!(sent(&l1_gas_price_estimate(), "ArbGasInfo", "getL1GasPriceEstimate", 0), n("getL1GasPriceEstimate"));
    assert_eq!(sent(&current_tx_l1_gas_fees(), "ArbGasInfo", "getCurrentTxL1GasFees", 0), n("getCurrentTxL1GasFees"));
    assert_eq!(sent(&l1_fees_available(), "ArbGasInfo", "getL1FeesAvailable", 0), n("getL1FeesAvailable"));
}

/// The live answer viem decoded, at another moment: the same struct.
#[test]
fn a_live_answer_reads_as_viem_read_it() {
    let answer = sdk_answer("ArbGasInfo", "getPricesInWei", 0);
    let prices = prices_in_wei().decode(&bytes(&answer["data"])).unwrap();
    let said: Vec<&str> = answer["decoded"].as_array().unwrap().iter().map(text).collect();
    let ours = [
        prices.per_l2_tx,
        prices.per_l1_calldata_unit,
        prices.per_storage_allocation,
        prices.per_arb_gas_base,
        prices.per_arb_gas_congestion,
        prices.per_arb_gas_total,
    ];
    assert_eq!(ours.map(|value| value.to_string()).to_vec(), said);
}

#[test]
fn the_gas_model_s_limits_backlog_and_surplus_read_as_answered() {
    let accounting = sent(&gas_accounting_params(), "ArbGasInfo", "getGasAccountingParams", 0);
    let said = decoded("ArbGasInfo", "getGasAccountingParams", 0);
    assert_eq!(
        [accounting.speed_limit_per_second, accounting.gas_pool_max, accounting.max_tx_gas_limit].to_vec(),
        said.iter().map(number).collect::<Vec<_>>()
    );
    let limit = sent(&max_tx_gas_limit(), "ArbGasInfo", "getMaxTxGasLimit", 0);
    assert_eq!(limit, U256::from(32_000_000u64), "the real limit, not the header's");
    assert_eq!(sent(&max_block_gas_limit(), "ArbGasInfo", "getMaxBlockGasLimit", 0), 32_000_000);
    assert_eq!(U256::from(sent(&gas_backlog(), "ArbGasInfo", "getGasBacklog", 0)), n("getGasBacklog"));
    assert_eq!(U256::from(sent(&pricing_inertia(), "ArbGasInfo", "getPricingInertia", 0)), n("getPricingInertia"));
    assert_eq!(sent(&gas_backlog_tolerance(), "ArbGasInfo", "getGasBacklogTolerance", 0), 10);
    assert_eq!(sent(&l1_base_fee_estimate_inertia(), "ArbGasInfo", "getL1BaseFeeEstimateInertia", 0), 10);
    assert_eq!(sent(&l1_reward_rate(), "ArbGasInfo", "getL1RewardRate", 0), 10);
    assert_eq!(
        sent(&l1_reward_recipient(), "ArbGasInfo", "getL1RewardRecipient", 0).to_hex(),
        text(&decoded("ArbGasInfo", "getL1RewardRecipient", 0)[0]).to_lowercase()
    );
    assert_eq!(sent(&amortized_cost_cap_bips(), "ArbGasInfo", "getAmortizedCostCapBips", 0), 0);
    assert_eq!(sent(&per_batch_gas_charge(), "ArbGasInfo", "getPerBatchGasCharge", 0), 210_000);
    assert_eq!(
        sent(&l1_pricing_surplus(), "ArbGasInfo", "getL1PricingSurplus", 0).to_string(),
        text(&decoded("ArbGasInfo", "getL1PricingSurplus", 0)[0])
    );
    assert_eq!(
        U256::from(sent(&last_l1_pricing_update_time(), "ArbGasInfo", "getLastL1PricingUpdateTime", 0)),
        n("getLastL1PricingUpdateTime")
    );
}

/// ArbOS's newer pricing: windows of a target, a length and a backlog, and one base fee per
/// resource.
#[test]
fn the_pricing_windows_and_each_resource_s_base_fee_read_as_answered() {
    let windows = sent(&gas_pricing_constraints(), "ArbGasInfo", "getGasPricingConstraints", 0);
    assert_eq!(
        windows,
        [
            PricingConstraint { target: 200_000_000, window: 15, backlog: 90_529 },
            PricingConstraint { target: 100_000_000, window: 86_400, backlog: 90_529 },
        ]
    );
    let fees = sent(&multi_gas_base_fee(), "ArbGasInfo", "getMultiGasBaseFee", 0);
    let said = decoded("ArbGasInfo", "getMultiGasBaseFee", 0);
    assert_eq!(fees, said[0].as_array().unwrap().iter().map(number).collect::<Vec<_>>());
    assert_eq!(fees.len(), 9);
}

#[test]
fn each_read_is_viem_s_calldata() {
    for (name, read) in [
        ("getPricesInWei", prices_in_wei().call),
        ("getL1BaseFeeEstimate", l1_base_fee_estimate().call),
        ("getPricesInArbGas", prices_in_arb_gas().call),
        ("getGasBacklog", gas_backlog().call),
        ("getMinimumGasPrice", minimum_gas_price().call),
    ] {
        assert_eq!(hex::encode_prefixed(&read.data), sdk_data(&sdk_one("ArbGasInfo", name)), "{name}");
        assert_eq!(read.to, Some(ARB_GAS_INFO), "{name}");
    }
}
