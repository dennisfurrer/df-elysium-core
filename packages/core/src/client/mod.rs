//! Connecting to Elysium: the [`Client`] that reads, signs and sends over one network's JSON-RPC,
//! the [`network`], the methods Nitro adds ([`rpc`]) and the records it answers with
//! ([`records`]), typed contract calls ([`method`], [`event`], [`read`]), and the native and
//! browser wires from df-hyperliquid-core.

pub mod event;
pub mod method;
pub mod network;
pub mod read;
pub mod records;
pub mod rpc;

#[cfg(feature = "native")]
pub use df_hyperliquid_core::client::native;
#[cfg(feature = "web")]
pub use df_hyperliquid_core::client::web;

pub use df_hyperliquid_core::client::evm::tx::{Call, Eip1559, Fees, create_address, tx_hash, with_headroom};
pub use df_hyperliquid_core::client::{Http, Method as HttpMethod, OFFLINE, Request, Response, TransportError};
pub use event::{BadLog, Event};
pub use method::{Arg, Method};
pub use network::{Network, TESTNET_CHAIN_ID, UnknownNetwork};
pub use read::{Read, ReadError};
pub use records::{Block, BlockTransactions, FeeHistory, Log, Receipt, Transaction, TxKind};
pub use rpc::{BlockTag, CallRequest, LogFilter, Query, Rpc, RpcError};

use crate::account::Nonces;
#[cfg(feature = "sign")]
use crate::account::{SignError, Signer, verify};
use crate::gas::{self, Estimate};
use crate::utils::{Address, Dec};
use crate::{B256, U256};

/// Why a transaction was not sent. One whose answer never came
/// ([`TransportError::Unreachable`]) may have landed, so it is never resent here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[cfg(feature = "sign")]
    #[error(transparent)]
    Sign(#[from] SignError),
    #[error(transparent)]
    Rpc(#[from] RpcError),
}

/// Reads for any module (`nitro::sys::arb_os_version()` through [`Client::read`]) and, with a
/// [`Signer`], transactions. `S` is `()` for a client that only reads.
///
/// [`Client::parent`] is the same client on HyperEVM, Elysium's parent, for the bridge's calls
/// there: deposits, retryable tickets and claims.
pub struct Client<H, S = ()> {
    rpc: Rpc<H>,
    network: Network,
    chain: u64,
    signer: S,
    nonces: Nonces,
}

/// Its network, chain and node; never its signer or its wire.
impl<H: Http, S> std::fmt::Debug for Client<H, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("network", &self.network)
            .field("chain", &self.chain)
            .field("url", &self.rpc.url())
            .finish_non_exhaustive()
    }
}

impl<H: Http> Client<H> {
    /// A client that reads and never signs, through Kinetiq's public node.
    pub fn reader(http: H, network: Network) -> Self {
        Client::new(http, network, ())
    }
}

impl<H: Http, S> Client<H, S> {
    /// One network through Kinetiq's public node ([`Network::rpc_url`]), signing with `signer`.
    pub fn new(http: H, network: Network, signer: S) -> Self {
        Client::at(http, network.rpc_url(), network, signer)
    }

    /// One network through another node: Conduit's, or a provider's with its key in the URL.
    pub fn at(http: H, url: impl Into<String>, network: Network, signer: S) -> Self {
        Client { rpc: Rpc::new(http, url), network, chain: network.chain_id(), signer, nonces: Nonces::new() }
    }

    /// The network's parent, HyperEVM, through its own node
    /// ([`df_hyperliquid_core::Network::evm_rpc_url`]), signing for its chain
    /// ([`Network::parent_chain_id`]): for the bridge's calls there. Sends and Ethereum's reads work
    /// as on Elysium, and receipts and transactions read; Nitro's own (blocks, [`Client::estimate`])
    /// are Elysium's, so read HyperEVM's blocks through [`Client::rpc`].
    pub fn parent(http: H, network: Network, signer: S) -> Self {
        Client {
            rpc: Rpc::new(http, network.hyperliquid().evm_rpc_url()),
            network,
            chain: network.parent_chain_id(),
            signer,
            nonces: Nonces::new(),
        }
    }

    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }

    /// The chain this client signs for: Elysium's, or HyperEVM's for [`Client::parent`].
    #[must_use]
    pub const fn chain(&self) -> u64 {
        self.chain
    }

    /// The node, for any of df-hyperliquid-core's `eth_` reads.
    #[must_use]
    pub const fn rpc(&self) -> &Rpc<H> {
        &self.rpc
    }

    #[must_use]
    pub const fn http(&self) -> &H {
        self.rpc.http()
    }

    #[must_use]
    pub const fn signer(&self) -> &S {
        &self.signer
    }

    /// The nonce kept between sends.
    #[must_use]
    pub const fn nonces(&self) -> &Nonces {
        &self.nonces
    }

    /// Any query, sent once.
    pub async fn run<T>(&self, query: Query<T>) -> Result<T, RpcError> {
        rpc::run(&self.rpc, query).await
    }

    /// A typed read at the latest block.
    pub async fn read<T>(&self, read: &Read<T>) -> Result<T, ReadError> {
        read.on(&self.rpc, BlockTag::Latest).await
    }

    pub async fn chain_id(&self) -> Result<u64, RpcError> {
        self.rpc.chain_id().await
    }

    pub async fn block_number(&self) -> Result<u64, RpcError> {
        self.rpc.block_number().await
    }

    /// An address's HYPE in wei, at the latest block.
    pub async fn balance(&self, owner: Address) -> Result<U256, RpcError> {
        self.rpc.balance(owner, BlockTag::Latest).await
    }

    /// An address's HYPE, exactly.
    pub async fn hype(&self, owner: Address) -> Result<Dec, RpcError> {
        let wei = self.balance(owner).await?;
        crate::utils::hype(wei)
            .map_err(|error| RpcError::Unreadable { method: "eth_getBalance", detail: error.to_string() })
    }

    /// The next nonce `owner` may use, counting what the sequencer has taken.
    pub async fn nonce(&self, owner: Address) -> Result<u64, RpcError> {
        self.rpc.transaction_count(owner, BlockTag::Pending).await
    }

    /// Empty for an account that is not a contract; `0xfe` for a precompile.
    pub async fn code(&self, at: Address) -> Result<Vec<u8>, RpcError> {
        self.rpc.code(at, BlockTag::Latest).await
    }

    /// What one unit of gas costs now, in wei: the base fee. There is no tip to add.
    pub async fn gas_price(&self) -> Result<U256, RpcError> {
        self.rpc.gas_price().await
    }

    /// The gas `call` takes when `from` sends it, split into Elysium's execution and HyperEVM's
    /// data, at the base fees it would pay ([`crate::nitro::node::gas_estimate_components`]).
    pub async fn estimate(&self, from: Option<Address>, call: &Call) -> Result<Estimate, ReadError> {
        self.read(&gas::components(from, call)).await
    }

    pub async fn receipt(&self, hash: B256) -> Result<Option<Receipt>, RpcError> {
        self.run(rpc::nitro::receipt(hash)).await
    }

    pub async fn transaction(&self, hash: B256) -> Result<Option<Transaction>, RpcError> {
        self.run(rpc::nitro::transaction(hash)).await
    }

    pub async fn block(&self, block: BlockTag, full: bool) -> Result<Option<Block>, RpcError> {
        self.run(rpc::nitro::block(block, full)).await
    }

    pub async fn block_by_hash(&self, hash: B256, full: bool) -> Result<Option<Block>, RpcError> {
        self.run(rpc::nitro::block_by_hash(hash, full)).await
    }

    pub async fn block_receipts(&self, block: BlockTag) -> Result<Option<Vec<Receipt>>, RpcError> {
        self.run(rpc::nitro::block_receipts(block)).await
    }

    /// The logs `filter` matches. A filter that names no address and no topic is sent in windows
    /// of [`Network::log_span`] blocks, its tags read as the head; any other is sent whole.
    pub async fn logs(&self, filter: &LogFilter) -> Result<Vec<Log>, RpcError> {
        if filter.block_hash.is_some() || rpc::is_narrow(filter) {
            return self.rpc.logs(filter).await;
        }
        let fixed = |tag: Option<BlockTag>| match tag {
            Some(BlockTag::Number(number)) => Some(number),
            Some(BlockTag::Earliest) => Some(0),
            _ => None,
        };
        let head = match (fixed(filter.from_block), fixed(filter.to_block)) {
            (Some(_), Some(_)) => 0,
            _ => self.rpc.block_number().await?,
        };
        let from = fixed(filter.from_block).unwrap_or(head);
        let to = fixed(filter.to_block).unwrap_or(head);
        let mut logs = Vec::new();
        for (first, last) in rpc::pages(from, to, self.network.log_span()) {
            logs.extend(self.rpc.logs(&rpc::window(filter, first, last)).await?);
        }
        Ok(logs)
    }
}

/// Sending needs the `sign` feature: each signature is recovered and must match the signer before
/// anything is sent, since a remote signer can sign as another address.
#[cfg(feature = "sign")]
impl<H: Http, S: Signer> Client<H, S> {
    /// The address this client sends from.
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// The transaction that makes `call`: this client's chain, the next nonce, the node's gas
    /// estimate with [`with_headroom`], and fees over the current base fee ([`gas::fees`]).
    pub async fn prepare(&self, call: Call) -> Result<Eip1559, SendError> {
        let from = self.signer.address();
        let gas = self.rpc.estimate_gas(&CallRequest::of(Some(from), &call)).await?;
        let base = self.rpc.gas_price().await?;
        let nonce = match self.nonces.kept() {
            Some(nonce) => nonce,
            None => self.nonces.read(self.nonce(from).await?),
        };
        Ok(call.transaction(self.chain, nonce, with_headroom(gas), gas::fees(base)))
    }

    /// The bytes a node is sent: signed by the signer, the signature recovered and checked.
    pub async fn sign(&self, tx: &Eip1559) -> Result<Vec<u8>, SignError> {
        let hash = tx.signing_hash();
        let signature = self.signer.sign_hash(&hash).await?;
        verify(&hash.0, &signature, self.signer.address())?;
        tx.signed(&signature)
    }

    /// Prepares, signs and sends `call` once; the hash the node took it under.
    pub async fn send(&self, call: Call) -> Result<B256, SendError> {
        let tx = self.prepare(call).await?;
        self.send_prepared(&tx).await
    }

    /// Signs and sends a prepared transaction once.
    pub async fn send_prepared(&self, tx: &Eip1559) -> Result<B256, SendError> {
        let sent = async { Ok::<_, SendError>(self.rpc.send_raw_transaction(&self.sign(tx).await?).await?) };
        self.kept(tx.nonce, sent.await)
    }

    /// Prepares, signs and sends `call` once with `eth_sendRawTransactionSync`: the answer is its
    /// receipt, a reverted one included. The node gives up after `timeout_ms` (its own default
    /// when `None`); then the transaction may still land.
    pub async fn send_sync(&self, call: Call, timeout_ms: Option<u64>) -> Result<Receipt, SendError> {
        let tx = self.prepare(call).await?;
        self.send_prepared_sync(&tx, timeout_ms).await
    }

    /// Signs and sends a prepared transaction once, answering with its receipt.
    pub async fn send_prepared_sync(&self, tx: &Eip1559, timeout_ms: Option<u64>) -> Result<Receipt, SendError> {
        let sent = async {
            let raw = self.sign(tx).await?;
            Ok::<_, SendError>(self.run(rpc::nitro::send_raw_transaction_sync(&raw, timeout_ms)).await?)
        };
        self.kept(tx.nonce, sent.await)
    }

    /// Keeps the nonce past a sent transaction, or forgets it when the send failed.
    fn kept<T>(&self, nonce: u64, sent: Result<T, SendError>) -> Result<T, SendError> {
        match &sent {
            Ok(_) => self.nonces.sent(nonce),
            Err(_) => self.nonces.forget(),
        }
        sent
    }
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
