//! ArbAggregator at `0x6d`: who posts Elysium's batches to HyperEVM, and who is paid for it. The
//! sequencer's own address, `0xa4b0...73657175656e636572` ("sequencer"), is one of them.

use df_hyperliquid_core::client::evm::abi::Token;

use crate::U256;
use crate::client::read::{Fields, one};
use crate::client::{Arg, Method, Read};
use crate::nitro::table::ARB_AGGREGATOR;
use crate::utils::Address;

pub const GET_PREFERRED_AGGREGATOR: Method = Method::new("getPreferredAggregator(address)", "address, bool");
pub const GET_DEFAULT_AGGREGATOR: Method = Method::new("getDefaultAggregator()", "address");
pub const GET_BATCH_POSTERS: Method = Method::new("getBatchPosters()", "address[]");
pub const GET_FEE_COLLECTOR: Method = Method::new("getFeeCollector(address)", "address");
pub const GET_TX_BASE_FEE: Method = Method::new("getTxBaseFee(address)", "uint256");

/// Every method of ArbAggregator this crate reads.
pub const METHODS: &[Method] =
    &[GET_PREFERRED_AGGREGATOR, GET_DEFAULT_AGGREGATOR, GET_BATCH_POSTERS, GET_FEE_COLLECTOR, GET_TX_BASE_FEE];

/// The sequencer's address as a batch poster: "sequencer" in ASCII after `0xa4b0`.
pub const SEQUENCER: Address = match Address::parse_const("0xa4b000000000000000000073657175656e636572") {
    Some(address) => address,
    None => Address::ZERO,
};

#[must_use]
pub fn batch_posters() -> Read<Vec<Address>> {
    Read::new(ARB_AGGREGATOR, &GET_BATCH_POSTERS, &[], |answer| {
        let tokens = GET_BATCH_POSTERS.answer(answer)?;
        Fields::of("getBatchPosters", &tokens).addresses()
    })
}

#[must_use]
pub fn default_aggregator() -> Read<Address> {
    Read::new(ARB_AGGREGATOR, &GET_DEFAULT_AGGREGATOR, &[], |answer| {
        one(&GET_DEFAULT_AGGREGATOR, answer, Token::as_address)
    })
}

/// The aggregator `address` prefers, and whether that is the default rather than its own choice.
#[must_use]
pub fn preferred_aggregator(address: Address) -> Read<(Address, bool)> {
    Read::new(ARB_AGGREGATOR, &GET_PREFERRED_AGGREGATOR, &[Arg::Address(address)], |answer| {
        let tokens = GET_PREFERRED_AGGREGATOR.answer(answer)?;
        let mut fields = Fields::of("getPreferredAggregator", &tokens);
        Ok((fields.address()?, fields.bool()?))
    })
}

/// Where a batch poster's fees go.
#[must_use]
pub fn fee_collector(batch_poster: Address) -> Read<Address> {
    Read::new(ARB_AGGREGATOR, &GET_FEE_COLLECTOR, &[Arg::Address(batch_poster)], |answer| {
        one(&GET_FEE_COLLECTOR, answer, Token::as_address)
    })
}

/// What `aggregator` charges per transaction, in wei, as ArbOS reports it.
#[must_use]
pub fn tx_base_fee(aggregator: Address) -> Read<U256> {
    Read::new(ARB_AGGREGATOR, &GET_TX_BASE_FEE, &[Arg::Address(aggregator)], |answer| {
        one(&GET_TX_BASE_FEE, answer, Token::as_u256)
    })
}

#[cfg(test)]
#[path = "tests/aggregator.rs"]
mod tests;
