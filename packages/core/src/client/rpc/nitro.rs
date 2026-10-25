//! The methods whose answers Nitro writes its own way, or that df-hyperliquid-core does not carry,
//! as [`Query`]s: receipts with their L1 gas, blocks with the outbox's send root and count,
//! transactions of Nitro's types, a block's receipts at once, and a send that answers with its
//! receipt. Every other method is df-hyperliquid-core's [`eth`](crate::client::rpc::eth).

use crate::B256;
use crate::client::records::{Block, Receipt, Transaction};
use crate::client::rpc::query::{json, tag, word};
use crate::client::rpc::{BlockTag, Query};
use crate::utils::{Json, hex};

/// `eth_getTransactionReceipt`: `None` until it is in a block.
#[must_use]
pub fn receipt(hash: B256) -> Query<Option<Receipt>> {
    Query::new("eth_getTransactionReceipt", vec![word(hash)], json)
}

/// `eth_getTransactionByHash`: `None` for a hash the node does not know.
#[must_use]
pub fn transaction(hash: B256) -> Query<Option<Transaction>> {
    Query::new("eth_getTransactionByHash", vec![word(hash)], json)
}

/// `eth_getBlockByNumber`, with whole transactions when `full`.
#[must_use]
pub fn block(block: BlockTag, full: bool) -> Query<Option<Block>> {
    Query::new("eth_getBlockByNumber", vec![tag(block), Json::Bool(full)], json)
}

/// `eth_getBlockByHash`, with whole transactions when `full`.
#[must_use]
pub fn block_by_hash(hash: B256, full: bool) -> Query<Option<Block>> {
    Query::new("eth_getBlockByHash", vec![word(hash), Json::Bool(full)], json)
}

/// `eth_getBlockReceipts`: every receipt of one block, in order.
#[must_use]
pub fn block_receipts(block: BlockTag) -> Query<Option<Vec<Receipt>>> {
    Query::new("eth_getBlockReceipts", vec![tag(block)], json)
}

/// `eth_sendRawTransactionSync` (EIP-7966): the node holds the answer until the transaction is in
/// a block and answers with its receipt, or gives up after `timeout_ms` (its own default when
/// `None`), which Nitro takes as a JSON number, not a quantity. A give-up is a
/// [`crate::client::rpc::RpcError::Node`] whose data is the hash: the transaction may still land,
/// so look for its receipt before sending it again.
#[must_use]
pub fn send_raw_transaction_sync(raw: &[u8], timeout_ms: Option<u64>) -> Query<Receipt> {
    let mut params = vec![Json::String(hex::encode_prefixed(raw))];
    if let Some(timeout) = timeout_ms {
        params.push(Json::Number(timeout.to_string()));
    }
    Query::new("eth_sendRawTransactionSync", params, json)
}

/// `web3_clientVersion`: `nitro/...` on Elysium.
#[must_use]
pub fn client_version() -> Query<String> {
    Query::new("web3_clientVersion", Vec::new(), json)
}

/// `net_version`: the chain id in decimal.
#[must_use]
pub fn net_version() -> Query<String> {
    Query::new("net_version", Vec::new(), json)
}
