//! Typed JSON-RPC to Elysium. Ethereum's methods are df-hyperliquid-core's [`Rpc`] and [`eth`]
//! queries, pointed at Elysium's node; [`nitro`] holds the ones Nitro answers its own way or adds,
//! as [`Query`]s that [`run`] sends through the same [`Rpc`].
//!
//! Elysium's public node has no websocket and no mempool (a pending filter never fires), and
//! answers an unfiltered `eth_getLogs` over at most 2,000 blocks ([`pages()`]).

pub mod nitro;
pub mod pages;
pub mod query;

pub use df_hyperliquid_core::client::evm::rpc::{
    BlockTag, CallRequest, ErrorKind, LogFilter, NodeError, Revert, Rpc, RpcError, eth, read_revert,
};
pub use pages::{Pages, is_narrow, pages, window};
pub use query::{Query, run};

/// Whether the node refused a read because the backend that took it had not seen the block yet
/// (`header not found`). Kinetiq's public node is several backends behind one balancer, and now and
/// then one lags a block: nothing was done, and the same read a moment later goes through. Nothing
/// here retries it; the caller decides.
#[must_use]
pub fn lagging(error: &RpcError) -> bool {
    matches!(error, RpcError::Node { error, .. } if error.message == "header not found")
}
