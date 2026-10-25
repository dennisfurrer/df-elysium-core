//! ArbAddressTable at `0x66`: addresses registered once and named after by a short index, so
//! calldata posted to HyperEVM costs less. A registered address's index never changes.

use df_hyperliquid_core::client::evm::abi::Token;

use crate::U256;
use crate::client::read::{Fields, one};
use crate::client::{Arg, Call, Method, Read};
use crate::nitro::table::ARB_ADDRESS_TABLE;
use crate::utils::Address;

pub const ADDRESS_EXISTS: Method = Method::new("addressExists(address)", "bool");
pub const COMPRESS: Method = Method::new("compress(address)", "bytes");
pub const DECOMPRESS: Method = Method::new("decompress(bytes,uint256)", "address, uint256");
pub const LOOKUP: Method = Method::new("lookup(address)", "uint256");
pub const LOOKUP_INDEX: Method = Method::new("lookupIndex(uint256)", "address");
pub const REGISTER: Method = Method::new("register(address)", "uint256");
pub const SIZE: Method = Method::new("size()", "uint256");

/// Every method of ArbAddressTable.
pub const METHODS: &[Method] = &[ADDRESS_EXISTS, COMPRESS, DECOMPRESS, LOOKUP, LOOKUP_INDEX, REGISTER, SIZE];

#[must_use]
pub fn address_exists(address: Address) -> Read<bool> {
    Read::new(ARB_ADDRESS_TABLE, &ADDRESS_EXISTS, &[Arg::Address(address)], |answer| {
        one(&ADDRESS_EXISTS, answer, Token::as_bool)
    })
}

/// The address in its short form: its index when registered, the address itself otherwise.
#[must_use]
pub fn compress(address: Address) -> Read<Vec<u8>> {
    Read::new(ARB_ADDRESS_TABLE, &COMPRESS, &[Arg::Address(address)], |answer| {
        one(&COMPRESS, answer, |token| token.as_bytes().map(<[u8]>::to_vec))
    })
}

/// The address at `offset` in `buffer`, and how many bytes it took.
#[must_use]
pub fn decompress(buffer: &[u8], offset: u64) -> Read<(Address, U256)> {
    Read::new(ARB_ADDRESS_TABLE, &DECOMPRESS, &[Arg::Bytes(buffer), Arg::u64(offset)], |answer| {
        let tokens = DECOMPRESS.answer(answer)?;
        let mut fields = Fields::of("decompress", &tokens);
        Ok((fields.address()?, fields.uint()?))
    })
}

/// A registered address's index; the read reverts for one that is not registered.
#[must_use]
pub fn lookup(address: Address) -> Read<U256> {
    Read::new(ARB_ADDRESS_TABLE, &LOOKUP, &[Arg::Address(address)], |answer| one(&LOOKUP, answer, Token::as_u256))
}

/// The address registered at `index`.
#[must_use]
pub fn lookup_index(index: u64) -> Read<Address> {
    Read::new(ARB_ADDRESS_TABLE, &LOOKUP_INDEX, &[Arg::u64(index)], |answer| {
        one(&LOOKUP_INDEX, answer, Token::as_address)
    })
}

/// How many addresses are registered.
#[must_use]
pub fn size() -> Read<U256> {
    Read::new(ARB_ADDRESS_TABLE, &SIZE, &[], |answer| one(&SIZE, answer, Token::as_u256))
}

/// Registers `address`, or answers its index when it already is.
#[must_use]
pub fn register(address: Address) -> Call {
    Call { to: Some(ARB_ADDRESS_TABLE), value: U256::ZERO, data: REGISTER.calldata(&[Arg::Address(address)]) }
}

#[cfg(test)]
#[path = "tests/address_table.rs"]
mod tests;
