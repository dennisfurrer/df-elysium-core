//! What an address holds and who signs for it.
//!
//! - [`signer`]: the [`Signer`] a client signs with, and df-hyperliquid-core's keys and signatures.
//! - [`nonce`]: the next nonce, kept between sends.
//!
//! An address's HYPE, nonce and code are [`crate::Client`] reads: [`crate::Client::balance`],
//! [`crate::Client::hype`], [`crate::Client::nonce`], [`crate::Client::code`].

pub mod nonce;
pub mod signer;

pub use nonce::Nonces;
#[cfg(feature = "sign")]
pub use signer::{LocalKey, recover, verify};
pub use signer::{SignError, Signature, Signer};
