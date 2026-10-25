//! ArbGasInfo at `0x6c`: Nitro's two-dimensional gas model. A transaction pays Elysium's base fee
//! for its execution and, folded into the same gas, the cost of posting its bytes to HyperEVM at
//! the parent chain's data price ArbOS estimates. There is no tip: priority fees are not paid.
//!
//! Prices are in wei of HYPE; "arb gas" is gas on Elysium.

use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::U256;
use crate::client::read::{Fields, one};
use crate::client::{Method, Read};
use crate::nitro::table::ARB_GAS_INFO;
use crate::utils::Address;

pub const GET_PRICES_IN_WEI: Method =
    Method::new("getPricesInWei()", "uint256, uint256, uint256, uint256, uint256, uint256");
pub const GET_PRICES_IN_WEI_WITH_AGGREGATOR: Method =
    Method::new("getPricesInWeiWithAggregator(address)", "uint256, uint256, uint256, uint256, uint256, uint256");
pub const GET_PRICES_IN_ARB_GAS: Method = Method::new("getPricesInArbGas()", "uint256, uint256, uint256");
pub const GET_GAS_ACCOUNTING_PARAMS: Method =
    Method::new("getGasAccountingParams()", "uint256 speedLimitPerSecond, uint256 gasPoolMax, uint256 maxTxGasLimit");
pub const GET_MINIMUM_GAS_PRICE: Method = Method::new("getMinimumGasPrice()", "uint256");
pub const GET_L1_BASE_FEE_ESTIMATE: Method = Method::new("getL1BaseFeeEstimate()", "uint256");
pub const GET_L1_BASE_FEE_ESTIMATE_INERTIA: Method = Method::new("getL1BaseFeeEstimateInertia()", "uint64");
pub const GET_L1_REWARD_RATE: Method = Method::new("getL1RewardRate()", "uint64");
pub const GET_L1_REWARD_RECIPIENT: Method = Method::new("getL1RewardRecipient()", "address");
pub const GET_L1_GAS_PRICE_ESTIMATE: Method = Method::new("getL1GasPriceEstimate()", "uint256");
pub const GET_CURRENT_TX_L1_GAS_FEES: Method = Method::new("getCurrentTxL1GasFees()", "uint256");
pub const GET_GAS_BACKLOG: Method = Method::new("getGasBacklog()", "uint64");
pub const GET_PRICING_INERTIA: Method = Method::new("getPricingInertia()", "uint64");
pub const GET_GAS_BACKLOG_TOLERANCE: Method = Method::new("getGasBacklogTolerance()", "uint64");
pub const GET_L1_PRICING_SURPLUS: Method = Method::new("getL1PricingSurplus()", "int256");
pub const GET_PER_BATCH_GAS_CHARGE: Method = Method::new("getPerBatchGasCharge()", "int64");
pub const GET_AMORTIZED_COST_CAP_BIPS: Method = Method::new("getAmortizedCostCapBips()", "uint64");
pub const GET_L1_FEES_AVAILABLE: Method = Method::new("getL1FeesAvailable()", "uint256");
pub const GET_L1_PRICING_EQUILIBRATION_UNITS: Method = Method::new("getL1PricingEquilibrationUnits()", "uint256");
pub const GET_LAST_L1_PRICING_UPDATE_TIME: Method = Method::new("getLastL1PricingUpdateTime()", "uint64");
pub const GET_L1_PRICING_FUNDS_DUE_FOR_REWARDS: Method = Method::new("getL1PricingFundsDueForRewards()", "uint256");
pub const GET_L1_PRICING_UNITS_SINCE_UPDATE: Method = Method::new("getL1PricingUnitsSinceUpdate()", "uint64");
pub const GET_LAST_L1_PRICING_SURPLUS: Method = Method::new("getLastL1PricingSurplus()", "int256");
pub const GET_PRICES_IN_ARB_GAS_WITH_AGGREGATOR: Method =
    Method::new("getPricesInArbGasWithAggregator(address)", "uint256, uint256, uint256");
pub const GET_MAX_TX_GAS_LIMIT: Method = Method::new("getMaxTxGasLimit()", "uint256");
pub const GET_MAX_BLOCK_GAS_LIMIT: Method = Method::new("getMaxBlockGasLimit()", "uint64");
pub const GET_GAS_PRICING_CONSTRAINTS: Method = Method::new("getGasPricingConstraints()", "uint64[3][] constraints");
pub const GET_MULTI_GAS_BASE_FEE: Method = Method::new("getMultiGasBaseFee()", "uint256[] baseFees");

/// Every method of ArbGasInfo this crate reads.
pub const METHODS: &[Method] = &[
    GET_PRICES_IN_WEI,
    GET_PRICES_IN_WEI_WITH_AGGREGATOR,
    GET_PRICES_IN_ARB_GAS,
    GET_PRICES_IN_ARB_GAS_WITH_AGGREGATOR,
    GET_GAS_ACCOUNTING_PARAMS,
    GET_MINIMUM_GAS_PRICE,
    GET_L1_BASE_FEE_ESTIMATE,
    GET_L1_BASE_FEE_ESTIMATE_INERTIA,
    GET_L1_REWARD_RATE,
    GET_L1_REWARD_RECIPIENT,
    GET_L1_GAS_PRICE_ESTIMATE,
    GET_CURRENT_TX_L1_GAS_FEES,
    GET_GAS_BACKLOG,
    GET_PRICING_INERTIA,
    GET_GAS_BACKLOG_TOLERANCE,
    GET_L1_PRICING_SURPLUS,
    GET_PER_BATCH_GAS_CHARGE,
    GET_AMORTIZED_COST_CAP_BIPS,
    GET_L1_FEES_AVAILABLE,
    GET_L1_PRICING_EQUILIBRATION_UNITS,
    GET_LAST_L1_PRICING_UPDATE_TIME,
    GET_L1_PRICING_FUNDS_DUE_FOR_REWARDS,
    GET_L1_PRICING_UNITS_SINCE_UPDATE,
    GET_LAST_L1_PRICING_SURPLUS,
    GET_MAX_TX_GAS_LIMIT,
    GET_MAX_BLOCK_GAS_LIMIT,
    GET_GAS_PRICING_CONSTRAINTS,
    GET_MULTI_GAS_BASE_FEE,
];

/// What a transaction is charged, in wei, by [`prices_in_wei`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricesInWei {
    /// Per transaction, for its share of a batch.
    pub per_l2_tx: U256,
    /// Per unit of calldata posted to HyperEVM.
    pub per_l1_calldata_unit: U256,
    /// Per storage slot allocated.
    pub per_storage_allocation: U256,
    /// Per unit of gas: the minimum,
    pub per_arb_gas_base: U256,
    /// what congestion adds over it,
    pub per_arb_gas_congestion: U256,
    /// and the two together, the base fee.
    pub per_arb_gas_total: U256,
}

fn prices(name: &'static str, method: &Method, answer: &[u8]) -> Result<PricesInWei, AbiError> {
    let tokens = method.answer(answer)?;
    let mut fields = Fields::of(name, &tokens);
    Ok(PricesInWei {
        per_l2_tx: fields.uint()?,
        per_l1_calldata_unit: fields.uint()?,
        per_storage_allocation: fields.uint()?,
        per_arb_gas_base: fields.uint()?,
        per_arb_gas_congestion: fields.uint()?,
        per_arb_gas_total: fields.uint()?,
    })
}

/// The prices a transaction pays now, through the chain's batch poster.
#[must_use]
pub fn prices_in_wei() -> Read<PricesInWei> {
    Read::new(ARB_GAS_INFO, &GET_PRICES_IN_WEI, &[], |answer| prices("getPricesInWei", &GET_PRICES_IN_WEI, answer))
}

/// The same, through `aggregator`.
#[must_use]
pub fn prices_in_wei_with(aggregator: Address) -> Read<PricesInWei> {
    let args = [crate::client::Arg::Address(aggregator)];
    Read::new(ARB_GAS_INFO, &GET_PRICES_IN_WEI_WITH_AGGREGATOR, &args, |answer| {
        prices("getPricesInWeiWithAggregator", &GET_PRICES_IN_WEI_WITH_AGGREGATOR, answer)
    })
}

/// The parent chain's share of a transaction, in gas on Elysium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricesInArbGas {
    pub per_l2_tx: U256,
    pub per_l1_calldata_unit: U256,
    pub per_storage_allocation: U256,
}

#[must_use]
pub fn prices_in_arb_gas() -> Read<PricesInArbGas> {
    Read::new(ARB_GAS_INFO, &GET_PRICES_IN_ARB_GAS, &[], |answer| {
        let tokens = GET_PRICES_IN_ARB_GAS.answer(answer)?;
        let mut fields = Fields::of("getPricesInArbGas", &tokens);
        Ok(PricesInArbGas {
            per_l2_tx: fields.uint()?,
            per_l1_calldata_unit: fields.uint()?,
            per_storage_allocation: fields.uint()?,
        })
    })
}

/// How much gas the chain takes: its target per second, the pool, and one transaction's most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasAccounting {
    pub speed_limit_per_second: U256,
    pub gas_pool_max: U256,
    pub max_tx_gas_limit: U256,
}

#[must_use]
pub fn gas_accounting_params() -> Read<GasAccounting> {
    Read::new(ARB_GAS_INFO, &GET_GAS_ACCOUNTING_PARAMS, &[], |answer| {
        let tokens = GET_GAS_ACCOUNTING_PARAMS.answer(answer)?;
        let mut fields = Fields::of("getGasAccountingParams", &tokens);
        Ok(GasAccounting {
            speed_limit_per_second: fields.uint()?,
            gas_pool_max: fields.uint()?,
            max_tx_gas_limit: fields.uint()?,
        })
    })
}

/// The base fee's floor, in wei.
#[must_use]
pub fn minimum_gas_price() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_MINIMUM_GAS_PRICE, &[], |answer| one(&GET_MINIMUM_GAS_PRICE, answer, Token::as_u256))
}

/// The parent chain's data price ArbOS charges calldata against, in wei per unit.
#[must_use]
pub fn l1_base_fee_estimate() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_L1_BASE_FEE_ESTIMATE, &[], |answer| {
        one(&GET_L1_BASE_FEE_ESTIMATE, answer, Token::as_u256)
    })
}

#[must_use]
pub fn l1_base_fee_estimate_inertia() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_L1_BASE_FEE_ESTIMATE_INERTIA, &[], |answer| {
        one(&GET_L1_BASE_FEE_ESTIMATE_INERTIA, answer, Token::as_u64)
    })
}

/// The batch poster's reward, in wei per unit of gas.
#[must_use]
pub fn l1_reward_rate() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_L1_REWARD_RATE, &[], |answer| one(&GET_L1_REWARD_RATE, answer, Token::as_u64))
}

#[must_use]
pub fn l1_reward_recipient() -> Read<Address> {
    Read::new(ARB_GAS_INFO, &GET_L1_REWARD_RECIPIENT, &[], |answer| {
        one(&GET_L1_REWARD_RECIPIENT, answer, Token::as_address)
    })
}

#[must_use]
pub fn l1_gas_price_estimate() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_L1_GAS_PRICE_ESTIMATE, &[], |answer| {
        one(&GET_L1_GAS_PRICE_ESTIMATE, answer, Token::as_u256)
    })
}

/// What the transaction making the call pays for HyperEVM's data, in wei; in an `eth_call`, the
/// simulated transaction's.
#[must_use]
pub fn current_tx_l1_gas_fees() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_CURRENT_TX_L1_GAS_FEES, &[], |answer| {
        one(&GET_CURRENT_TX_L1_GAS_FEES, answer, Token::as_u256)
    })
}

/// Gas used past the speed limit and not yet worked off: what congestion pricing reads.
#[must_use]
pub fn gas_backlog() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_GAS_BACKLOG, &[], |answer| one(&GET_GAS_BACKLOG, answer, Token::as_u64))
}

#[must_use]
pub fn pricing_inertia() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_PRICING_INERTIA, &[], |answer| one(&GET_PRICING_INERTIA, answer, Token::as_u64))
}

#[must_use]
pub fn gas_backlog_tolerance() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_GAS_BACKLOG_TOLERANCE, &[], |answer| {
        one(&GET_GAS_BACKLOG_TOLERANCE, answer, Token::as_u64)
    })
}

/// What the chain has collected for data over what posting it cost, in wei; negative when short.
#[must_use]
pub fn l1_pricing_surplus() -> Read<i128> {
    Read::new(ARB_GAS_INFO, &GET_L1_PRICING_SURPLUS, &[], |answer| one(&GET_L1_PRICING_SURPLUS, answer, Token::as_i128))
}

#[must_use]
pub fn per_batch_gas_charge() -> Read<i64> {
    Read::new(ARB_GAS_INFO, &GET_PER_BATCH_GAS_CHARGE, &[], |answer| {
        one(&GET_PER_BATCH_GAS_CHARGE, answer, Token::as_i64)
    })
}

#[must_use]
pub fn amortized_cost_cap_bips() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_AMORTIZED_COST_CAP_BIPS, &[], |answer| {
        one(&GET_AMORTIZED_COST_CAP_BIPS, answer, Token::as_u64)
    })
}

#[must_use]
pub fn l1_fees_available() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_L1_FEES_AVAILABLE, &[], |answer| one(&GET_L1_FEES_AVAILABLE, answer, Token::as_u256))
}

/// The most gas one transaction may take: 32 million on Elysium. A block header's gas limit is not
/// the real one.
#[must_use]
pub fn max_tx_gas_limit() -> Read<U256> {
    Read::new(ARB_GAS_INFO, &GET_MAX_TX_GAS_LIMIT, &[], |answer| one(&GET_MAX_TX_GAS_LIMIT, answer, Token::as_u256))
}

/// The most gas one block may take.
#[must_use]
pub fn max_block_gas_limit() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_MAX_BLOCK_GAS_LIMIT, &[], |answer| {
        one(&GET_MAX_BLOCK_GAS_LIMIT, answer, Token::as_u64)
    })
}

/// One window the base fee is priced over: a gas target per second, how long the window is in
/// seconds, and how much backlog it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricingConstraint {
    pub target: u64,
    pub window: u64,
    pub backlog: u64,
}

/// The windows the base fee is priced over, ArbOS's newer pricing.
#[must_use]
pub fn gas_pricing_constraints() -> Read<Vec<PricingConstraint>> {
    Read::new(ARB_GAS_INFO, &GET_GAS_PRICING_CONSTRAINTS, &[], |answer| {
        let tokens = GET_GAS_PRICING_CONSTRAINTS.answer(answer)?;
        let bad = || AbiError::Type("getGasPricingConstraints: not a list of uint64[3]".to_owned());
        let [Token::Array(rows)] = tokens.as_slice() else { return Err(bad()) };
        rows.iter()
            .map(|row| match row.as_items() {
                Some([target, window, backlog]) => Ok(PricingConstraint {
                    target: target.as_u64().ok_or_else(bad)?,
                    window: window.as_u64().ok_or_else(bad)?,
                    backlog: backlog.as_u64().ok_or_else(bad)?,
                }),
                _ => Err(bad()),
            })
            .collect()
    })
}

/// The base fee of each resource ArbOS prices on its own, in wei.
#[must_use]
pub fn multi_gas_base_fee() -> Read<Vec<U256>> {
    Read::new(ARB_GAS_INFO, &GET_MULTI_GAS_BASE_FEE, &[], |answer| {
        let tokens = GET_MULTI_GAS_BASE_FEE.answer(answer)?;
        let bad = || AbiError::Type("getMultiGasBaseFee: not a list of uint256".to_owned());
        let [Token::Array(fees)] = tokens.as_slice() else { return Err(bad()) };
        fees.iter().map(|fee| fee.as_u256().ok_or_else(bad)).collect()
    })
}

/// When the parent chain's data price was last updated, in Unix seconds.
#[must_use]
pub fn last_l1_pricing_update_time() -> Read<u64> {
    Read::new(ARB_GAS_INFO, &GET_LAST_L1_PRICING_UPDATE_TIME, &[], |answer| {
        one(&GET_LAST_L1_PRICING_UPDATE_TIME, answer, Token::as_u64)
    })
}

#[cfg(test)]
#[path = "tests/gas_info.rs"]
mod tests;
