//! An ERC-20 on Elysium: its reads as typed [`Read`]s and its writes as [`Call`]s, amounts in the
//! token's own units, and a units-to-decimal reading at its decimals. The calldata of `transfer`,
//! `balanceOf`, `approve`, `decimals` and `symbol` is df-hyperliquid-core's.

use df_hyperliquid_core::client::evm::abi::Token;
use df_hyperliquid_core::client::evm::{BadAmount, U256};
use df_hyperliquid_core::transfer::erc20 as calldata;

use crate::client::read::one;
use crate::client::{Arg, Call, Method, Read};
use crate::utils::{Address, Dec};

pub const TRANSFER: Method = Method::new("transfer(address,uint256)", "bool");
pub const TRANSFER_FROM: Method = Method::new("transferFrom(address,address,uint256)", "bool");
pub const APPROVE: Method = Method::new("approve(address,uint256)", "bool");
pub const BALANCE_OF: Method = Method::new("balanceOf(address)", "uint256");
pub const ALLOWANCE: Method = Method::new("allowance(address,address)", "uint256");
pub const TOTAL_SUPPLY: Method = Method::new("totalSupply()", "uint256");
pub const DECIMALS: Method = Method::new("decimals()", "uint8");
pub const SYMBOL: Method = Method::new("symbol()", "string");
pub const NAME: Method = Method::new("name()", "string");

/// `owner`'s balance of `token`, in its units.
#[must_use]
pub fn balance(token: Address, owner: Address) -> Read<U256> {
    Read::new(token, &BALANCE_OF, &[Arg::Address(owner)], |answer| one(&BALANCE_OF, answer, Token::as_u256))
}

/// What `spender` may still move of `owner`'s `token`.
#[must_use]
pub fn allowance(token: Address, owner: Address, spender: Address) -> Read<U256> {
    Read::new(token, &ALLOWANCE, &[Arg::Address(owner), Arg::Address(spender)], |answer| {
        one(&ALLOWANCE, answer, Token::as_u256)
    })
}

#[must_use]
pub fn total_supply(token: Address) -> Read<U256> {
    Read::new(token, &TOTAL_SUPPLY, &[], |answer| one(&TOTAL_SUPPLY, answer, Token::as_u256))
}

#[must_use]
pub fn decimals(token: Address) -> Read<u8> {
    Read::new(token, &DECIMALS, &[], |answer| one(&DECIMALS, answer, Token::as_u8))
}

#[must_use]
pub fn symbol(token: Address) -> Read<String> {
    Read::new(token, &SYMBOL, &[], |answer| one(&SYMBOL, answer, |token| token.as_str().map(ToOwned::to_owned)))
}

#[must_use]
pub fn name(token: Address) -> Read<String> {
    Read::new(token, &NAME, &[], |answer| one(&NAME, answer, |token| token.as_str().map(ToOwned::to_owned)))
}

/// Sends `amount` of `token`, in its units, to `to`.
#[must_use]
pub fn transfer(token: Address, to: Address, amount: U256) -> Call {
    Call { to: Some(token), value: U256::ZERO, data: calldata::transfer(to, amount) }
}

/// Lets `spender` move up to `amount` of `token`.
#[must_use]
pub fn approve(token: Address, spender: Address, amount: U256) -> Call {
    Call { to: Some(token), value: U256::ZERO, data: calldata::approve(spender, amount) }
}

/// Moves `amount` of `from`'s `token` to `to`, under an allowance.
#[must_use]
pub fn transfer_from(token: Address, from: Address, to: Address, amount: U256) -> Call {
    let data = TRANSFER_FROM.calldata(&[Arg::Address(from), Arg::Address(to), Arg::Uint(amount)]);
    Call { to: Some(token), value: U256::ZERO, data }
}

/// An amount in a token's units at its `decimals`, exactly.
pub fn amount(units: U256, decimals: u8) -> Result<Dec, BadAmount> {
    units.to_dec(u32::from(decimals))
}

/// A typed amount in a token's units, refused rather than rounded past its `decimals`.
pub fn units(amount: Dec, decimals: u8) -> Result<U256, BadAmount> {
    U256::from_dec(amount, u32::from(decimals))
}

#[cfg(test)]
#[path = "tests/erc20.rs"]
mod tests;
