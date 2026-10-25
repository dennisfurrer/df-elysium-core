//! Elysium SDK for Rust, on df-hyperliquid-core.
//!
//! - [`client`]: the [`Client`], the network, the JSON-RPC Nitro adds, typed contract calls and
//!   reads.
//! - [`nitro`]: ArbOS's precompiles and the NodeInterface.
//! - [`gas`]: what a transaction costs, Elysium's gas and HyperEVM's data split.
//! - [`account`]: signers, balances and nonces.
//! - [`transfer`]: HYPE, ERC-20s and Multicall3.
//! - [`bridge`]: HYPE and messages between HyperEVM and Elysium, both ways.
//! - [`deploy`]: contracts, CREATE2, Stylus and source verification.
//! - [`live`]: new blocks and logs, polled.
//! - [`utils`]: addresses, exact decimals, hex and HYPE's units.
//!
//! Testnet only until Kinetiq publishes mainnet. Money is exact integers and decimals, never
//! floats. Features: `sign` and `native` (default), `web`, `offline`.

pub mod account;
pub mod bridge;
pub mod client;
pub mod deploy;
pub mod gas;
pub mod live;
pub mod nitro;
pub mod transfer;
pub mod utils;

#[cfg(test)]
pub(crate) mod test_support;

pub use client::{Client, Network};
pub use df_hyperliquid_core::client::evm::{B256, U256};
pub use utils::{Address, Dec};
