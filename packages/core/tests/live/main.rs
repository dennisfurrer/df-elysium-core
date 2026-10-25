//! Against Elysium testnet and HyperEVM testnet, read only, run by hand with `just live`: every
//! [`Client`](df_elysium_core::Client) read, a sweep of the precompiles, a real withdrawal's claim
//! simulated on HyperEVM, a deposit simulated there, and Sourcify. Nothing is ever sent.
//!
//! Each read must parse into its type, and its JSON-RPC answer must have the shape of the recording
//! the unit and mocked suites read ([`tap::same_shape`]), so a field Nitro adds shows up here
//! before it reaches a user.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

mod shape;
mod tap;

mod bridge;
mod client;
mod nitro;
mod verify;
