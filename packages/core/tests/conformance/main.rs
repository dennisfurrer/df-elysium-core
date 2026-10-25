//! Encoding against other implementations: transactions, calldata, CREATE2 addresses and event
//! logs built or read through this crate's builders, compared byte for byte with what viem 2.57.4
//! and @arbitrum/sdk 4.1.0 produce for the same input (`vectors/arbitrum_sdk.json`), several of
//! them taken from Elysium and HyperEVM testnet themselves.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

mod calldata;
mod events;
mod transactions;
