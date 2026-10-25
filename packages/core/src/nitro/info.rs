//! ArbInfo at `0x65`: an account's balance and code, read by a contract the way a precompile
//! reads them.

use df_hyperliquid_core::client::evm::abi::Token;

use crate::U256;
use crate::client::read::one;
use crate::client::{Arg, Method, Read};
use crate::nitro::table::ARB_INFO;
use crate::utils::Address;

pub const GET_BALANCE: Method = Method::new("getBalance(address)", "uint256");
pub const GET_CODE: Method = Method::new("getCode(address)", "bytes");

/// Every method of ArbInfo.
pub const METHODS: &[Method] = &[GET_BALANCE, GET_CODE];

/// An account's HYPE, in wei.
#[must_use]
pub fn balance(account: Address) -> Read<U256> {
    Read::new(ARB_INFO, &GET_BALANCE, &[Arg::Address(account)], |answer| one(&GET_BALANCE, answer, Token::as_u256))
}

/// An account's code; empty for one that is not a contract.
#[must_use]
pub fn code(account: Address) -> Read<Vec<u8>> {
    Read::new(ARB_INFO, &GET_CODE, &[Arg::Address(account)], |answer| {
        one(&GET_CODE, answer, |token| token.as_bytes().map(<[u8]>::to_vec))
    })
}

#[cfg(test)]
#[path = "tests/info.rs"]
mod tests;
