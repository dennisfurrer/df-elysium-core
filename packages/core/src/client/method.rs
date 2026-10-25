//! A contract's function by its signature: the selector it is called by, the calldata of a call
//! and how its answer reads. Every contract this crate calls (the precompiles, the bridge, the
//! tokens) is a list of [`Method`]s, each checked against the reference implementations.

use df_hyperliquid_core::client::evm::abi::{self, AbiError, Token};

use crate::utils::{Address, keccak256};
use crate::{B256, U256};

const WORD: usize = 32;

/// A function: its canonical `name(types)` and what it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Method {
    /// `name(t1,t2)`, as the selector hashes it.
    pub signature: &'static str,
    /// What it answers, as a parameter list: `uint64 gasEstimate, uint64 gasEstimateForL1`.
    pub returns: &'static str,
}

impl Method {
    #[must_use]
    pub const fn new(signature: &'static str, returns: &'static str) -> Method {
        Method { signature, returns }
    }

    /// The four bytes that name it: the start of `keccak256(signature)`.
    #[must_use]
    pub fn selector(&self) -> [u8; 4] {
        let hash = keccak256(self.signature.as_bytes());
        [hash[0], hash[1], hash[2], hash[3]]
    }

    /// The calldata of a call: the selector, then the arguments, encoded as `abi.encode` lays them
    /// out. The arguments are the function's, in order.
    #[must_use]
    pub fn calldata(&self, args: &[Arg<'_>]) -> Vec<u8> {
        let heads = args.len() * WORD;
        let mut head = Vec::with_capacity(4 + heads);
        head.extend_from_slice(&self.selector());
        let mut tail = Vec::new();
        for arg in args {
            match arg {
                Arg::Address(address) => head.extend_from_slice(&B256::from_address(*address).0),
                Arg::Uint(value) => head.extend_from_slice(&value.to_be_bytes()),
                Arg::Word(word) => head.extend_from_slice(&word.0),
                Arg::Bool(value) => head.extend_from_slice(&count(usize::from(*value))),
                Arg::Bytes(bytes) => {
                    head.extend_from_slice(&count(heads + tail.len()));
                    tail.extend_from_slice(&count(bytes.len()));
                    tail.extend_from_slice(bytes);
                    tail.resize(tail.len() + bytes.len().next_multiple_of(WORD) - bytes.len(), 0);
                }
                Arg::Words(words) => {
                    head.extend_from_slice(&count(heads + tail.len()));
                    tail.extend_from_slice(&count(words.len()));
                    for word in *words {
                        tail.extend_from_slice(&word.0);
                    }
                }
            }
        }
        head.extend(tail);
        head
    }

    /// The answer read against [`Method::returns`].
    pub fn answer(&self, data: &[u8]) -> Result<Vec<Token>, AbiError> {
        abi::decode(&abi::params(self.returns)?, data)
    }
}

/// One argument: a static word, or `bytes` and `bytes32[]`, the two dynamic kinds the calls here
/// take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg<'a> {
    Address(Address),
    Uint(U256),
    Word(B256),
    Bool(bool),
    Bytes(&'a [u8]),
    Words(&'a [B256]),
}

impl Arg<'_> {
    /// A `uint` of up to 64 bits.
    #[must_use]
    pub const fn u64(value: u64) -> Arg<'static> {
        Arg::Uint(U256::from_u64(value))
    }
}

fn count(value: usize) -> [u8; WORD] {
    U256::from_u64(value as u64).to_be_bytes()
}

#[cfg(test)]
#[path = "tests/method.rs"]
mod tests;
