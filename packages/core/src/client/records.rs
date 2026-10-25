//! Receipts, transactions and blocks as Elysium's Nitro node answers them: Ethereum's fields and
//! Nitro's own. A receipt says how much of its gas paid for posting it to HyperEVM; a block names
//! the parent block it was sequenced at and the outbox's send count and root; a transaction may
//! be one of Nitro's own types (a deposit, a retryable and its redeem, ArbOS's start of block).
//! Logs, access lists and fee history are df-hyperliquid-core's.

use serde::{Deserialize, Deserializer, de};

use df_hyperliquid_core::account::Signature;
use df_hyperliquid_core::client::evm::tx::{AccessItem, EIP1559, Eip1559};

pub use df_hyperliquid_core::client::evm::records::{FeeHistory, Log};

use crate::utils::{Address, hex};
use crate::{B256, U256};

/// A transaction's type byte, Ethereum's and Nitro's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TxKind {
    /// 0x00: legacy, EIP-155 on Elysium.
    Legacy,
    /// 0x01: EIP-2930.
    AccessList,
    /// 0x02: EIP-1559.
    DynamicFee,
    /// 0x04: EIP-7702.
    SetCode,
    /// 0x64: HYPE deposited from HyperEVM (`Inbox.depositEth`).
    Deposit,
    /// 0x65: an unsigned transaction from the parent chain.
    Unsigned,
    /// 0x66: a contract's transaction from the parent chain.
    Contract,
    /// 0x68: a redeem of a retryable ticket, scheduled by its submission or by `redeem`.
    Retry,
    /// 0x69: a retryable ticket submitted on the parent chain.
    SubmitRetryable,
    /// 0x6a: ArbOS's own, at the start of every block.
    Internal,
    Other(u8),
}

impl TxKind {
    #[must_use]
    pub const fn from_byte(byte: u8) -> TxKind {
        match byte {
            0x00 => TxKind::Legacy,
            0x01 => TxKind::AccessList,
            0x02 => TxKind::DynamicFee,
            0x04 => TxKind::SetCode,
            0x64 => TxKind::Deposit,
            0x65 => TxKind::Unsigned,
            0x66 => TxKind::Contract,
            0x68 => TxKind::Retry,
            0x69 => TxKind::SubmitRetryable,
            0x6a => TxKind::Internal,
            other => TxKind::Other(other),
        }
    }

    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            TxKind::Legacy => 0x00,
            TxKind::AccessList => 0x01,
            TxKind::DynamicFee => 0x02,
            TxKind::SetCode => 0x04,
            TxKind::Deposit => 0x64,
            TxKind::Unsigned => 0x65,
            TxKind::Contract => 0x66,
            TxKind::Retry => 0x68,
            TxKind::SubmitRetryable => 0x69,
            TxKind::Internal => 0x6a,
            TxKind::Other(byte) => byte,
        }
    }

    /// One of Nitro's own: it came from the parent chain or from ArbOS, never from a signer here.
    #[must_use]
    pub const fn is_nitro(self) -> bool {
        matches!(self.byte(), 0x64..=0x6a)
    }
}

/// What `eth_getTransactionReceipt` answers once a transaction is in a block.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub transaction_hash: B256,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<B256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    /// The contract a creation made; `None` for a call.
    #[serde(default)]
    pub contract_address: Option<Address>,
    /// Everything the transaction paid for, [`Receipt::gas_used_for_l1`] included.
    #[serde(deserialize_with = "quantity")]
    pub gas_used: u64,
    #[serde(deserialize_with = "quantity")]
    pub cumulative_gas_used: u64,
    #[serde(default)]
    pub effective_gas_price: Option<U256>,
    /// `true` when it succeeded, `false` when it reverted.
    #[serde(default, deserialize_with = "opt_status")]
    pub status: Option<bool>,
    pub logs: Vec<Log>,
    #[serde(default, rename = "type", deserialize_with = "opt_byte")]
    pub kind: Option<u8>,
    /// Of [`Receipt::gas_used`], the units that paid for posting the transaction to HyperEVM.
    #[serde(default, deserialize_with = "quantity")]
    pub gas_used_for_l1: u64,
    /// The parent chain's block when it was sequenced: what `block.number` reads inside a contract.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub l1_block_number: Option<u64>,
}

impl Receipt {
    /// In a block and not reverted.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.status == Some(true)
    }

    /// What it cost the sender, in wei: every unit of gas at the price it paid.
    #[must_use]
    pub fn fee(&self) -> Option<U256> {
        self.effective_gas_price?.checked_mul(U256::from_u64(self.gas_used))
    }

    /// The gas Elysium's execution took, without HyperEVM's data.
    #[must_use]
    pub const fn gas_used_for_l2(&self) -> u64 {
        self.gas_used.saturating_sub(self.gas_used_for_l1)
    }

    /// Its type, when the node names one.
    #[must_use]
    pub fn tx_kind(&self) -> Option<TxKind> {
        self.kind.map(TxKind::from_byte)
    }
}

/// What `eth_getTransactionByHash` answers. Nitro's own types carry the fields that tie them to the
/// parent chain; each is `None` on the types that lack it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: B256,
    #[serde(default, rename = "type", deserialize_with = "byte")]
    pub kind: u8,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub chain_id: Option<u64>,
    #[serde(default, deserialize_with = "quantity")]
    pub nonce: u64,
    #[serde(default)]
    pub block_hash: Option<B256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    /// Unix seconds of its block.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_timestamp: Option<u64>,
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    #[serde(default)]
    pub value: U256,
    #[serde(default, deserialize_with = "quantity")]
    pub gas: u64,
    #[serde(default)]
    pub gas_price: Option<U256>,
    #[serde(default)]
    pub max_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub access_list: Vec<AccessItem>,
    #[serde(default, deserialize_with = "bytes")]
    pub input: Vec<u8>,
    #[serde(default)]
    pub r: Option<U256>,
    #[serde(default)]
    pub s: Option<U256>,
    #[serde(default)]
    pub v: Option<U256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub y_parity: Option<u64>,
    /// A deposit's or a retryable's message number in the parent's inbox, as a word.
    #[serde(default)]
    pub request_id: Option<B256>,
    /// A redeem's ticket: the hash of the retryable it runs.
    #[serde(default)]
    pub ticket_id: Option<B256>,
    /// Where a retryable's or a redeem's unspent gas is refunded.
    #[serde(default)]
    pub refund_to: Option<Address>,
    /// A redeem's most refunded gas, in wei.
    #[serde(default)]
    pub max_refund: Option<U256>,
    /// A redeem's refund of the submission fee, in wei.
    #[serde(default)]
    pub submission_fee_refund: Option<U256>,
    /// A retryable's parent base fee when submitted.
    #[serde(default)]
    pub l1_base_fee: Option<U256>,
    /// What a retryable brought from the parent, in wei.
    #[serde(default)]
    pub deposit_value: Option<U256>,
    #[serde(default)]
    pub retry_to: Option<Address>,
    #[serde(default)]
    pub retry_value: Option<U256>,
    #[serde(default, deserialize_with = "opt_bytes")]
    pub retry_data: Option<Vec<u8>>,
    /// Who may cancel a retryable, and receives its call value if it never runs.
    #[serde(default)]
    pub beneficiary: Option<Address>,
    #[serde(default)]
    pub max_submission_fee: Option<U256>,
}

impl Transaction {
    #[must_use]
    pub const fn tx_kind(&self) -> TxKind {
        TxKind::from_byte(self.kind)
    }

    /// The type 2 transaction this record is of: rebuilt, it hashes to [`Transaction::hash`] once
    /// signed with [`Transaction::signature`].
    #[must_use]
    pub fn to_eip1559(&self) -> Option<Eip1559> {
        if self.kind != EIP1559 {
            return None;
        }
        Some(Eip1559 {
            chain_id: self.chain_id?,
            nonce: self.nonce,
            max_priority_fee: self.max_priority_fee_per_gas?,
            max_fee: self.max_fee_per_gas?,
            gas: self.gas,
            to: self.to,
            value: self.value,
            data: self.input.clone(),
            access_list: self.access_list.clone(),
        })
    }

    /// A type 2 transaction's signature, `v` as 27 or 28.
    #[must_use]
    pub fn signature(&self) -> Option<Signature> {
        let parity = u8::try_from(self.y_parity?).ok().filter(|parity| *parity <= 1)?;
        Some(Signature::from_parts(self.r?.to_be_bytes(), self.s?.to_be_bytes(), 27 + parity))
    }

    /// The parent inbox's message number a deposit or a retryable came in as.
    #[must_use]
    pub fn message_number(&self) -> Option<U256> {
        self.request_id.map(U256::from)
    }
}

/// What `eth_getBlockByNumber` and `eth_getBlockByHash` answer on Nitro.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(deserialize_with = "quantity")]
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    /// Unix seconds.
    #[serde(deserialize_with = "quantity")]
    pub timestamp: u64,
    #[serde(deserialize_with = "quantity")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "quantity")]
    pub gas_used: u64,
    #[serde(default)]
    pub base_fee_per_gas: Option<U256>,
    /// The sequencer's fee account, not a miner.
    pub miner: Address,
    pub transactions: BlockTransactions,
    /// The parent chain's block this one was sequenced at.
    #[serde(deserialize_with = "quantity")]
    pub l1_block_number: u64,
    /// How many messages to the parent chain had been sent by the end of this block: the size of
    /// the outbox's tree.
    #[serde(deserialize_with = "quantity")]
    pub send_count: u64,
    /// The root of the outbox's tree after this block, what a confirmed assertion commits to.
    pub send_root: B256,
    /// Nitro packs the send count, the parent block and ArbOS's version into it.
    pub mix_hash: B256,
}

impl Block {
    /// ArbOS's version when the block was made: the third eight bytes of its mix hash.
    /// `ArbSys.arbOSVersion()` answers it plus 55.
    #[must_use]
    pub fn arbos_version(&self) -> u64 {
        let mut eight = [0u8; 8];
        eight.copy_from_slice(&self.mix_hash.0[16..24]);
        u64::from_be_bytes(eight)
    }
}

/// A block's transactions: their hashes, or the whole records when asked for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum BlockTransactions {
    Hashes(Vec<B256>),
    Full(Vec<Transaction>),
}

/// A quantity that fits a `u64`: block numbers, nonces, gas.
pub(crate) fn quantity<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = U256::deserialize(deserializer)?;
    value.to_u64().ok_or_else(|| de::Error::custom(format!("{value} does not fit 64 bits")))
}

fn opt_quantity<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    match Option::<U256>::deserialize(deserializer)? {
        Some(value) => {
            value.to_u64().map(Some).ok_or_else(|| de::Error::custom(format!("{value} does not fit 64 bits")))
        }
        None => Ok(None),
    }
}

fn byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let value = quantity(deserializer)?;
    u8::try_from(value).map_err(|_| de::Error::custom(format!("{value} is not a transaction type")))
}

fn opt_byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u8>, D::Error> {
    match opt_quantity(deserializer)? {
        Some(value) => {
            u8::try_from(value).map(Some).map_err(|_| de::Error::custom(format!("{value} is not a transaction type")))
        }
        None => Ok(None),
    }
}

fn opt_status<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    match opt_quantity(deserializer)? {
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        Some(other) => Err(de::Error::custom(format!("{other} is not a receipt status"))),
        None => Ok(None),
    }
}

/// `0x` and hex bytes.
pub(crate) fn bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
    let digits = text.strip_prefix("0x").ok_or_else(|| de::Error::custom(format!("`{text}` is not 0x and hex")))?;
    hex::decode(digits).map_err(de::Error::custom)
}

fn opt_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error> {
    match Option::<std::borrow::Cow<'de, str>>::deserialize(deserializer)? {
        Some(text) => {
            let digits =
                text.strip_prefix("0x").ok_or_else(|| de::Error::custom(format!("`{text}` is not 0x and hex")))?;
            hex::decode(digits).map(Some).map_err(de::Error::custom)
        }
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "tests/records.rs"]
mod tests;
