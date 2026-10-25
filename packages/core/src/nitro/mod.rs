//! ArbOS's precompiles and the NodeInterface: the chain-specific surface of an Arbitrum Nitro
//! chain, as Elysium serves it (ArbOS 61, Stylus on). Each is a list of [`Method`]s as the Nitro
//! interfaces declare them, typed [`Read`]s for the views, and [`Call`]s for what a transaction
//! does.
//!
//! A precompile's code reads `0xfe`; `eth_call` with the declared selectors works. The
//! NodeInterface at `0xc8` has no code at all and answers only `eth_call` and `eth_estimateGas`.
//! HyperEVM's HyperCore read precompiles (`0x...0800` on) and CoreWriter are not on Elysium: a call
//! to them answers empty, as any address without code does.
//!
//! [`Method`]: crate::client::Method
//! [`Read`]: crate::client::Read
//! [`Call`]: crate::client::Call

pub mod address_table;
pub mod aggregator;
pub mod gas_info;
pub mod info;
pub mod node;
pub mod owner;
pub mod retryable;
pub mod statistics;
pub mod sys;
pub mod table;
pub mod wasm;

pub use table::{
    ALL, ARB_ADDRESS_TABLE, ARB_AGGREGATOR, ARB_GAS_INFO, ARB_INFO, ARB_OWNER, ARB_OWNER_PUBLIC, ARB_RETRYABLE_TX,
    ARB_STATISTICS, ARB_SYS, ARB_WASM, ARB_WASM_CACHE, ARBOS, ARBOS_VERSION_OFFSET, NODE_INTERFACE, Precompile,
    precompile,
};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
