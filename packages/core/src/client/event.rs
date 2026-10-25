//! An event by its declaration: the topic it is logged under, and a log read back into its
//! parameters in declared order, the indexed ones from the topics and the rest from the data.

use df_hyperliquid_core::client::evm::abi::{self, AbiError, Param, Token};

use crate::B256;
use crate::client::records::Log;
use crate::utils::keccak256;

/// An event: its canonical `Name(types)` and its parameters as declared, `indexed` marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Event {
    /// `Name(t1,t2)`, as the topic hashes it.
    pub signature: &'static str,
    /// The parameters, `indexed` kept: `address caller, address indexed destination`.
    pub params: &'static str,
}

/// Why a log is not the event it was read as.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadLog {
    #[error("the log is not {0}")]
    NotThisEvent(&'static str),
    #[error("{event}: {error}")]
    Abi { event: &'static str, error: AbiError },
}

impl Event {
    #[must_use]
    pub const fn new(signature: &'static str, params: &'static str) -> Event {
        Event { signature, params }
    }

    /// The first topic: `keccak256(signature)`.
    #[must_use]
    pub fn topic(&self) -> B256 {
        B256(keccak256(self.signature.as_bytes()))
    }

    /// The name, before its parameters.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.signature.split_once('(').map_or(self.signature, |(name, _)| name)
    }

    /// Whether the log was written under this event's topic.
    #[must_use]
    pub fn matches(&self, log: &Log) -> bool {
        log.topics.first() == Some(&self.topic())
    }

    /// The log's parameters in declared order. An indexed parameter of a dynamic type is only its
    /// hash in a topic, so it reads as the word.
    pub fn decode(&self, log: &Log) -> Result<Vec<Token>, BadLog> {
        let abi = |error: AbiError| BadLog::Abi { event: self.name(), error };
        if !self.matches(log) {
            return Err(BadLog::NotThisEvent(self.name()));
        }
        let params = abi::params(self.params).map_err(abi)?;
        let unindexed: Vec<Param> = params.iter().filter(|param| !param.indexed).cloned().collect();
        let mut data = abi::decode(&unindexed, &log.data).map_err(abi)?.into_iter();
        let mut topics = log.topics.iter().skip(1);
        let mut tokens = Vec::with_capacity(params.len());
        for param in &params {
            let token = match param.indexed {
                true => {
                    let topic = topics.next().ok_or(AbiError::Short(log.topics.len() * 32)).map_err(abi)?;
                    match param.kind.is_dynamic() {
                        true => Token::Fixed(topic.0.to_vec()),
                        false => abi::decode(std::slice::from_ref(param), &topic.0)
                            .map_err(abi)?
                            .into_iter()
                            .next()
                            .ok_or(AbiError::Short(32))
                            .map_err(abi)?,
                    }
                }
                false => data.next().ok_or(AbiError::Short(log.data.len())).map_err(abi)?,
            };
            tokens.push(token);
        }
        Ok(tokens)
    }
}

#[cfg(test)]
#[path = "tests/event.rs"]
mod tests;
