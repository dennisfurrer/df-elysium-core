//! A typed [`Read`]: the `eth_call` that makes it and how its answer reads, sent through any
//! [`Rpc`] (Elysium's, or HyperEVM's for the bridge's contracts), and a struct answer's fields read
//! in declared order.

use df_hyperliquid_core::client::Http;
use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::client::method::{Arg, Method};
use crate::client::rpc::{BlockTag, CallRequest, Rpc, RpcError};
use crate::utils::Address;
use crate::{B256, U256};

/// A typed read: the call that makes it, and how its answer reads.
#[derive(Debug, Clone)]
pub struct Read<T> {
    pub call: CallRequest,
    decode: fn(&[u8]) -> Result<T, AbiError>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error("the answer does not fit what the contract declares: {0}")]
    Abi(#[from] AbiError),
}

impl ReadError {
    /// Whether the node refused it for a lagging backend ([`crate::client::rpc::lagging`]).
    #[must_use]
    pub fn is_lagging(&self) -> bool {
        matches!(self, ReadError::Rpc(error) if crate::client::rpc::lagging(error))
    }
}

impl<T> Read<T> {
    /// `method` called on `at` with `args`, its answer read by `decode`.
    #[must_use]
    pub fn new(at: Address, method: &Method, args: &[Arg<'_>], decode: fn(&[u8]) -> Result<T, AbiError>) -> Read<T> {
        Read { call: CallRequest::to(at, method.calldata(args)), decode }
    }

    /// The same read as sent by `from`: what `msg.sender` is to the contract.
    #[must_use]
    pub fn from(mut self, from: Address) -> Read<T> {
        self.call.from = Some(from);
        self
    }

    /// The same read carrying `value` wei.
    #[must_use]
    pub fn with_value(mut self, value: U256) -> Read<T> {
        self.call.value = Some(value).filter(|value| !value.is_zero());
        self
    }

    /// The answer's bytes, read against what the contract declares.
    pub fn decode(&self, answer: &[u8]) -> Result<T, AbiError> {
        (self.decode)(answer)
    }

    /// Sends it to `rpc` at `block` and reads the answer.
    pub async fn on<H: Http>(&self, rpc: &Rpc<H>, block: BlockTag) -> Result<T, ReadError> {
        let answer = rpc.call(&self.call, block).await?;
        Ok(self.decode(&answer)?)
    }
}

/// The fields of a struct answer, read in order.
pub(crate) struct Fields<'a> {
    name: &'static str,
    items: std::slice::Iter<'a, Token>,
}

impl<'a> Fields<'a> {
    /// The fields of an answer that is a list of return values.
    pub(crate) fn of(name: &'static str, tokens: &'a [Token]) -> Fields<'a> {
        Fields { name, items: tokens.iter() }
    }

    /// The fields of the one tuple an answer is.
    pub(crate) fn tuple(name: &'static str, tokens: &'a [Token]) -> Result<Fields<'a>, AbiError> {
        match tokens {
            [Token::Tuple(items)] => Ok(Fields { name, items: items.iter() }),
            _ => Err(AbiError::Type(format!("{name}: not one struct"))),
        }
    }

    fn take<T>(&mut self, read: impl FnOnce(&'a Token) -> Option<T>) -> Result<T, AbiError> {
        self.items
            .next()
            .and_then(read)
            .ok_or_else(|| AbiError::Type(format!("{}: a field of another type", self.name)))
    }

    pub(crate) fn uint(&mut self) -> Result<U256, AbiError> {
        self.take(Token::as_u256)
    }

    pub(crate) fn u64(&mut self) -> Result<u64, AbiError> {
        self.take(Token::as_u64)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, AbiError> {
        self.take(Token::as_u16)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, AbiError> {
        self.take(Token::as_u8)
    }

    pub(crate) fn bool(&mut self) -> Result<bool, AbiError> {
        self.take(Token::as_bool)
    }

    pub(crate) fn address(&mut self) -> Result<Address, AbiError> {
        self.take(Token::as_address)
    }

    pub(crate) fn word(&mut self) -> Result<B256, AbiError> {
        self.take(|token| <[u8; 32]>::try_from(token.as_bytes()?).ok().map(B256))
    }

    pub(crate) fn bytes(&mut self) -> Result<Vec<u8>, AbiError> {
        self.take(|token| token.as_bytes().map(<[u8]>::to_vec))
    }

    pub(crate) fn items(&mut self) -> Result<&'a [Token], AbiError> {
        self.take(Token::as_items)
    }

    pub(crate) fn addresses(&mut self) -> Result<Vec<Address>, AbiError> {
        let name = self.name;
        self.items()?
            .iter()
            .map(|item| item.as_address().ok_or_else(|| AbiError::Type(format!("{name}: not a list of addresses"))))
            .collect()
    }

    pub(crate) fn words(&mut self) -> Result<Vec<B256>, AbiError> {
        let name = self.name;
        self.items()?
            .iter()
            .map(|item| {
                item.as_bytes()
                    .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
                    .map(B256)
                    .ok_or_else(|| AbiError::Type(format!("{name}: not a list of words")))
            })
            .collect()
    }
}

/// An answer that is one value, read by `read`.
pub(crate) fn one<T>(method: &Method, answer: &[u8], read: impl FnOnce(&Token) -> Option<T>) -> Result<T, AbiError> {
    let tokens = method.answer(answer)?;
    match tokens.as_slice() {
        [token] => read(token).ok_or_else(|| AbiError::Type(format!("{}: another type", method.signature))),
        _ => Err(AbiError::Type(format!("{}: not one value", method.signature))),
    }
}

/// An answer that is one word.
pub(crate) fn one_word(method: &Method, answer: &[u8]) -> Result<B256, AbiError> {
    one(method, answer, |token| token.as_bytes().and_then(|bytes| <[u8; 32]>::try_from(bytes).ok()).map(B256))
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;
