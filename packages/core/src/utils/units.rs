//! HYPE on Elysium: the native coin, bridged 1:1 from HyperEVM, eighteen decimals there as here.
//! A balance or a fee in wei reads as an exact [`Dec`], and a typed amount becomes wei exactly or
//! is refused.

use df_hyperliquid_core::client::evm::{BadAmount, NATIVE_DECIMALS, U256};

use crate::utils::Dec;

/// HYPE's decimals on Elysium and on HyperEVM.
pub const HYPE_DECIMALS: u32 = NATIVE_DECIMALS;

/// Wei as HYPE, exactly.
pub fn hype(wei: U256) -> Result<Dec, BadAmount> {
    wei.to_dec(HYPE_DECIMALS)
}

/// HYPE as wei, exactly: a nineteenth place is refused, never rounded.
pub fn wei(hype: Dec) -> Result<U256, BadAmount> {
    U256::from_dec(hype, HYPE_DECIMALS)
}

#[cfg(test)]
#[path = "tests/units.rs"]
mod tests;
