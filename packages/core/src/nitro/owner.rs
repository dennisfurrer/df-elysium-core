//! ArbOwnerPublic at `0x6b`: who runs the chain, where its fees go, and the ArbOS upgrade it has
//! scheduled, for anyone to read. Changing any of it is ArbOwner's, the chain owner's alone.

use df_hyperliquid_core::client::evm::abi::Token;

use crate::client::read::{Fields, one};
use crate::client::{Arg, Method, Read};
use crate::nitro::table::ARB_OWNER_PUBLIC;
use crate::utils::Address;

pub const GET_ALL_CHAIN_OWNERS: Method = Method::new("getAllChainOwners()", "address[]");
pub const IS_CHAIN_OWNER: Method = Method::new("isChainOwner(address)", "bool");
pub const GET_NETWORK_FEE_ACCOUNT: Method = Method::new("getNetworkFeeAccount()", "address");
pub const GET_INFRA_FEE_ACCOUNT: Method = Method::new("getInfraFeeAccount()", "address");
pub const GET_BROTLI_COMPRESSION_LEVEL: Method = Method::new("getBrotliCompressionLevel()", "uint64");
pub const GET_SCHEDULED_UPGRADE: Method =
    Method::new("getScheduledUpgrade()", "uint64 arbosVersion, uint64 scheduledForTimestamp");
pub const GET_PARENT_GAS_FLOOR_PER_TOKEN: Method = Method::new("getParentGasFloorPerToken()", "uint64");
pub const IS_CALLDATA_PRICE_INCREASE_ENABLED: Method = Method::new("isCalldataPriceIncreaseEnabled()", "bool");
pub const GET_COLLECT_TIPS: Method = Method::new("getCollectTips()", "bool");
pub const GET_MAX_STYLUS_CONTRACT_FRAGMENTS: Method = Method::new("getMaxStylusContractFragments()", "uint8");
pub const GET_NATIVE_TOKEN_MANAGEMENT_FROM: Method = Method::new("getNativeTokenManagementFrom()", "uint64");
pub const IS_NATIVE_TOKEN_OWNER: Method = Method::new("isNativeTokenOwner(address)", "bool");
pub const GET_ALL_NATIVE_TOKEN_OWNERS: Method = Method::new("getAllNativeTokenOwners()", "address[]");
pub const GET_TRANSACTION_FILTERING_FROM: Method = Method::new("getTransactionFilteringFrom()", "uint64");
pub const IS_TRANSACTION_FILTERER: Method = Method::new("isTransactionFilterer(address)", "bool");
pub const GET_ALL_TRANSACTION_FILTERERS: Method = Method::new("getAllTransactionFilterers()", "address[]");
pub const GET_FILTERED_FUNDS_RECIPIENT: Method = Method::new("getFilteredFundsRecipient()", "address");

/// Every method of ArbOwnerPublic this crate reads.
pub const METHODS: &[Method] = &[
    GET_ALL_CHAIN_OWNERS,
    IS_CHAIN_OWNER,
    GET_NETWORK_FEE_ACCOUNT,
    GET_INFRA_FEE_ACCOUNT,
    GET_BROTLI_COMPRESSION_LEVEL,
    GET_SCHEDULED_UPGRADE,
    GET_PARENT_GAS_FLOOR_PER_TOKEN,
    IS_CALLDATA_PRICE_INCREASE_ENABLED,
    GET_COLLECT_TIPS,
    GET_MAX_STYLUS_CONTRACT_FRAGMENTS,
    GET_NATIVE_TOKEN_MANAGEMENT_FROM,
    IS_NATIVE_TOKEN_OWNER,
    GET_ALL_NATIVE_TOKEN_OWNERS,
    GET_TRANSACTION_FILTERING_FROM,
    IS_TRANSACTION_FILTERER,
    GET_ALL_TRANSACTION_FILTERERS,
    GET_FILTERED_FUNDS_RECIPIENT,
];

#[must_use]
pub fn chain_owners() -> Read<Vec<Address>> {
    Read::new(ARB_OWNER_PUBLIC, &GET_ALL_CHAIN_OWNERS, &[], |answer| {
        let tokens = GET_ALL_CHAIN_OWNERS.answer(answer)?;
        Fields::of("getAllChainOwners", &tokens).addresses()
    })
}

#[must_use]
pub fn is_chain_owner(address: Address) -> Read<bool> {
    Read::new(ARB_OWNER_PUBLIC, &IS_CHAIN_OWNER, &[Arg::Address(address)], |answer| {
        one(&IS_CHAIN_OWNER, answer, Token::as_bool)
    })
}

/// Where the base fee on Elysium's execution goes.
#[must_use]
pub fn network_fee_account() -> Read<Address> {
    Read::new(ARB_OWNER_PUBLIC, &GET_NETWORK_FEE_ACCOUNT, &[], |answer| {
        one(&GET_NETWORK_FEE_ACCOUNT, answer, Token::as_address)
    })
}

/// Where the infrastructure's share of fees goes.
#[must_use]
pub fn infra_fee_account() -> Read<Address> {
    Read::new(ARB_OWNER_PUBLIC, &GET_INFRA_FEE_ACCOUNT, &[], |answer| {
        one(&GET_INFRA_FEE_ACCOUNT, answer, Token::as_address)
    })
}

/// The brotli level batches are compressed at before they are posted.
#[must_use]
pub fn brotli_compression_level() -> Read<u64> {
    Read::new(ARB_OWNER_PUBLIC, &GET_BROTLI_COMPRESSION_LEVEL, &[], |answer| {
        one(&GET_BROTLI_COMPRESSION_LEVEL, answer, Token::as_u64)
    })
}

/// An ArbOS upgrade the owner has scheduled; both zero when none is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledUpgrade {
    pub arbos_version: u64,
    /// Unix seconds.
    pub at: u64,
}

impl ScheduledUpgrade {
    #[must_use]
    pub const fn is_scheduled(&self) -> bool {
        self.arbos_version != 0
    }
}

#[must_use]
pub fn scheduled_upgrade() -> Read<ScheduledUpgrade> {
    Read::new(ARB_OWNER_PUBLIC, &GET_SCHEDULED_UPGRADE, &[], |answer| {
        let tokens = GET_SCHEDULED_UPGRADE.answer(answer)?;
        let mut fields = Fields::of("getScheduledUpgrade", &tokens);
        Ok(ScheduledUpgrade { arbos_version: fields.u64()?, at: fields.u64()? })
    })
}

/// Whether priority fees are collected. Ordering stays first come, first served either way, so a
/// tip buys nothing.
#[must_use]
pub fn collect_tips() -> Read<bool> {
    Read::new(ARB_OWNER_PUBLIC, &GET_COLLECT_TIPS, &[], |answer| one(&GET_COLLECT_TIPS, answer, Token::as_bool))
}

/// How many fragments a Stylus program's code may be split into.
#[must_use]
pub fn max_stylus_contract_fragments() -> Read<u8> {
    Read::new(ARB_OWNER_PUBLIC, &GET_MAX_STYLUS_CONTRACT_FRAGMENTS, &[], |answer| {
        one(&GET_MAX_STYLUS_CONTRACT_FRAGMENTS, answer, Token::as_u8)
    })
}

/// Whether calldata is priced at EIP-7623's floor.
#[must_use]
pub fn is_calldata_price_increase_enabled() -> Read<bool> {
    Read::new(ARB_OWNER_PUBLIC, &IS_CALLDATA_PRICE_INCREASE_ENABLED, &[], |answer| {
        one(&IS_CALLDATA_PRICE_INCREASE_ENABLED, answer, Token::as_bool)
    })
}

/// Who may mint and burn the native coin, once native token management is on.
#[must_use]
pub fn native_token_owners() -> Read<Vec<Address>> {
    Read::new(ARB_OWNER_PUBLIC, &GET_ALL_NATIVE_TOKEN_OWNERS, &[], |answer| {
        let tokens = GET_ALL_NATIVE_TOKEN_OWNERS.answer(answer)?;
        Fields::of("getAllNativeTokenOwners", &tokens).addresses()
    })
}

/// Who may filter transactions, once filtering is on.
#[must_use]
pub fn transaction_filterers() -> Read<Vec<Address>> {
    Read::new(ARB_OWNER_PUBLIC, &GET_ALL_TRANSACTION_FILTERERS, &[], |answer| {
        let tokens = GET_ALL_TRANSACTION_FILTERERS.answer(answer)?;
        Fields::of("getAllTransactionFilterers", &tokens).addresses()
    })
}

#[cfg(test)]
#[path = "tests/owner.rs"]
mod tests;
