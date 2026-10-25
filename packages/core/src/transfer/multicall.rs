//! Multicall3, predeployed at its canonical address: many reads in one `eth_call`, each allowed to
//! fail on its own, so a page of balances or a book of quotes is one round trip at Elysium's
//! block pace.

use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::U256;
use crate::client::read::one;
use crate::client::{Arg, Method, Read};
use crate::utils::Address;

/// Multicall3, at its canonical address.
pub const MULTICALL3: Address = match Address::parse_const("0xcA11bde05977b3631167028862bE2a173976CA11") {
    Some(address) => address,
    None => Address::ZERO,
};

pub const AGGREGATE3: Method =
    Method::new("aggregate3((address,bool,bytes)[])", "(bool success, bytes returnData)[] returnData");
pub const GET_ETH_BALANCE: Method = Method::new("getEthBalance(address)", "uint256 balance");
pub const GET_BLOCK_NUMBER: Method = Method::new("getBlockNumber()", "uint256 blockNumber");

/// One call in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call3 {
    pub target: Address,
    /// Whether the batch goes on when this call reverts.
    pub allow_failure: bool,
    pub data: Vec<u8>,
}

impl Call3 {
    /// The call a typed read makes, its answer read back with [`Read::decode`].
    #[must_use]
    pub fn of<T>(read: &Read<T>, allow_failure: bool) -> Call3 {
        Call3 { target: read.call.to.unwrap_or(Address::ZERO), allow_failure, data: read.call.data.clone() }
    }
}

/// What one call answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub success: bool,
    /// The answer, or a failed call's revert data.
    pub data: Vec<u8>,
}

/// `calls` in one `eth_call`, each answer in order.
#[must_use]
pub fn aggregate3(calls: &[Call3]) -> Read<Vec<Outcome>> {
    let mut read = Read::new(MULTICALL3, &AGGREGATE3, &[], |answer| {
        let tokens = AGGREGATE3.answer(answer)?;
        let bad = || AbiError::Type("aggregate3: not a list of (bool, bytes)".to_owned());
        let [Token::Array(items)] = tokens.as_slice() else { return Err(bad()) };
        items
            .iter()
            .map(|item| match item.as_items() {
                Some([Token::Bool(success), Token::Bytes(data)]) => {
                    Ok(Outcome { success: *success, data: data.clone() })
                }
                _ => Err(bad()),
            })
            .collect()
    });
    read.call.data = aggregate3_calldata(calls);
    read
}

/// `aggregate3`'s calldata: the selector, then one dynamic array of `(address, bool, bytes)`, each
/// tuple behind an offset from the array's first word after its length.
fn aggregate3_calldata(calls: &[Call3]) -> Vec<u8> {
    let word = |value: usize| U256::from_u64(value as u64).to_be_bytes();
    let mut out = AGGREGATE3.selector().to_vec();
    out.extend_from_slice(&word(32));
    out.extend_from_slice(&word(calls.len()));
    let mut tails = Vec::new();
    let mut offsets = Vec::with_capacity(calls.len());
    for call in calls {
        offsets.push(calls.len() * 32 + tails.len());
        let mut tuple = Vec::new();
        tuple.extend_from_slice(&crate::B256::from_address(call.target).0);
        tuple.extend_from_slice(&word(usize::from(call.allow_failure)));
        tuple.extend_from_slice(&word(96));
        tuple.extend_from_slice(&word(call.data.len()));
        tuple.extend_from_slice(&call.data);
        tuple.resize(tuple.len() + call.data.len().next_multiple_of(32) - call.data.len(), 0);
        tails.extend(tuple);
    }
    for offset in offsets {
        out.extend_from_slice(&word(offset));
    }
    out.extend(tails);
    out
}

/// An address's HYPE, as Multicall3 reads it: one more read to put in a batch.
#[must_use]
pub fn eth_balance(owner: Address) -> Read<U256> {
    Read::new(MULTICALL3, &GET_ETH_BALANCE, &[Arg::Address(owner)], |answer| {
        one(&GET_ETH_BALANCE, answer, Token::as_u256)
    })
}

/// `block.number` as a contract reads it: HyperEVM's block, not Elysium's
/// ([`crate::nitro::sys::arb_block_number`]).
#[must_use]
pub fn block_number() -> Read<U256> {
    Read::new(MULTICALL3, &GET_BLOCK_NUMBER, &[], |answer| one(&GET_BLOCK_NUMBER, answer, Token::as_u256))
}

#[cfg(test)]
#[path = "tests/multicall.rs"]
mod tests;
