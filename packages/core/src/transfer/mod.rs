//! Moving value on Elysium: HYPE, the native coin ([`hype`]), ERC-20 tokens ([`erc20`]), and many
//! reads in one call through Multicall3 ([`multicall`]). Uniswap's Permit2 is predeployed at its
//! canonical address. Moving HYPE to or from HyperEVM is [`crate::bridge`].

pub mod erc20;
pub mod multicall;

use df_hyperliquid_core::client::evm::BadAmount;

use crate::U256;
use crate::client::Call;
use crate::utils::{Address, Dec, wei};

/// Uniswap's Permit2, at its canonical address.
pub const PERMIT2: Address = match Address::parse_const("0x000000000022D473030F116dDEE9F6B43aC78BA3") {
    Some(address) => address,
    None => Address::ZERO,
};

/// Sends `amount` HYPE to `to`. Exact: a nineteenth place is refused, never rounded. The gas is a
/// little over 21,000: let the client estimate it.
pub fn hype(to: Address, amount: Dec) -> Result<Call, BadAmount> {
    if !amount.is_positive() {
        return Err(BadAmount::NotPositive(amount));
    }
    Ok(hype_wei(to, wei(amount)?))
}

/// Sends `value` wei of HYPE to `to`.
#[must_use]
pub fn hype_wei(to: Address, value: U256) -> Call {
    Call { to: Some(to), value, data: Vec::new() }
}

#[cfg(test)]
#[path = "tests/transfer.rs"]
mod tests;
