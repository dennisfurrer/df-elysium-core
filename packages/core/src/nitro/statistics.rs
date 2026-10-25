//! ArbStatistics at `0x6f`: the chain's height, and what Arbitrum Classic carried over at the
//! switch to Nitro (all zero on a chain born on Nitro, as Elysium was).

use crate::U256;
use crate::client::read::Fields;
use crate::client::{Method, Read};
use crate::nitro::table::ARB_STATISTICS;

pub const GET_STATS: Method = Method::new("getStats()", "uint256, uint256, uint256, uint256, uint256, uint256");

/// Every method of ArbStatistics.
pub const METHODS: &[Method] = &[GET_STATS];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub block_number: U256,
    pub classic_accounts: U256,
    pub classic_storage_sum: U256,
    pub classic_gas_sum: U256,
    pub classic_transactions: U256,
    pub classic_contracts: U256,
}

#[must_use]
pub fn stats() -> Read<Stats> {
    Read::new(ARB_STATISTICS, &GET_STATS, &[], |answer| {
        let tokens = GET_STATS.answer(answer)?;
        let mut fields = Fields::of("getStats", &tokens);
        Ok(Stats {
            block_number: fields.uint()?,
            classic_accounts: fields.uint()?,
            classic_storage_sum: fields.uint()?,
            classic_gas_sum: fields.uint()?,
            classic_transactions: fields.uint()?,
            classic_contracts: fields.uint()?,
        })
    })
}

#[cfg(test)]
#[path = "tests/statistics.rs"]
mod tests;
