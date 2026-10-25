//! Whole journeys through the public [`Client`](df_elysium_core::Client) and df-hyperliquid-core's
//! `Rpc` to HyperEVM, against a fake of both chains that answers as testnet did
//! ([`support::node::Node`], from `vectors/elysium_testnet.json` and `vectors/hyperevm_testnet.json`
//! verbatim) and Sourcify's answers recorded under `fixtures/sourcify/`. Each test is one thing a
//! user does, end to end; the units it passes through have their own tests next to their code.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

mod bridge;
mod client;
mod live;
mod logs;
mod send;
mod verify;
