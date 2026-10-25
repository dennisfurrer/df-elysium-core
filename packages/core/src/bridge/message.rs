//! A message from HyperEVM to Elysium as the parent chain logs it: the bridge's
//! `MessageDelivered` (its number, kind and aliased sender) joined with the inbox's
//! `InboxMessageDelivered` (its data). Every deposit and retryable ticket is one.

use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::bridge::Contracts;
use crate::client::read::Fields;
use crate::client::{BadLog, Event, Log};
use crate::utils::Address;
use crate::{B256, U256};

/// Logged by the bridge for every message, whatever its kind.
pub const MESSAGE_DELIVERED: Event = Event::new(
    "MessageDelivered(uint256,bytes32,address,uint8,address,bytes32,uint256,uint64)",
    "uint256 indexed messageIndex, bytes32 indexed beforeInboxAcc, address inbox, uint8 kind, address sender, \
     bytes32 messageDataHash, uint256 baseFeeL1, uint64 timestamp",
);

/// Logged by the inbox with the message's data.
pub const INBOX_MESSAGE_DELIVERED: Event =
    Event::new("InboxMessageDelivered(uint256,bytes)", "uint256 indexed messageNum, bytes data");

/// Logged by the inbox instead, when the data is the sending transaction's own calldata.
pub const INBOX_MESSAGE_DELIVERED_FROM_ORIGIN: Event =
    Event::new("InboxMessageDeliveredFromOrigin(uint256)", "uint256 indexed messageNum");

/// What a message is, as the bridge numbers kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {
    /// 3: a signed transaction sent through the inbox.
    L2Message,
    /// 7: a transaction funded on the parent chain.
    L2FundedByL1,
    /// 9: a retryable ticket.
    SubmitRetryable,
    /// 11: the chain's initialization.
    Initialization,
    /// 12: HYPE deposited.
    EthDeposit,
    /// 13: the batch poster's report of what posting cost.
    BatchPostingReport,
    Other(u8),
}

impl MessageKind {
    #[must_use]
    pub const fn from_byte(byte: u8) -> MessageKind {
        match byte {
            3 => MessageKind::L2Message,
            7 => MessageKind::L2FundedByL1,
            9 => MessageKind::SubmitRetryable,
            11 => MessageKind::Initialization,
            12 => MessageKind::EthDeposit,
            13 => MessageKind::BatchPostingReport,
            other => MessageKind::Other(other),
        }
    }

    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            MessageKind::L2Message => 3,
            MessageKind::L2FundedByL1 => 7,
            MessageKind::SubmitRetryable => 9,
            MessageKind::Initialization => 11,
            MessageKind::EthDeposit => 12,
            MessageKind::BatchPostingReport => 13,
            MessageKind::Other(byte) => byte,
        }
    }
}

/// One message delivered to Elysium's inbox on the parent chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Its number in the inbox: a deposit's or a ticket's `requestId` on Elysium.
    pub number: U256,
    pub kind: MessageKind,
    /// The sender as Elysium sees it: already aliased ([`crate::bridge::alias`]).
    pub sender: Address,
    /// The parent chain's base fee when it was delivered.
    pub base_fee_l1: U256,
    /// Unix seconds.
    pub timestamp: u64,
    pub data_hash: B256,
    /// The inbox's data; `None` when it was the parent transaction's own calldata.
    pub data: Option<Vec<u8>>,
}

impl Message {
    /// Every message a parent-chain receipt delivered to `contracts`' inbox, in the order logged.
    pub fn in_logs(logs: &[Log], contracts: &Contracts) -> Result<Vec<Message>, BadLog> {
        let mut messages = Vec::new();
        for log in logs.iter().filter(|log| log.address == contracts.bridge && MESSAGE_DELIVERED.matches(log)) {
            let tokens = MESSAGE_DELIVERED.decode(log)?;
            let abi = |error: AbiError| BadLog::Abi { event: "MessageDelivered", error };
            let mut fields = Fields::of("MessageDelivered", &tokens);
            let number = fields.uint().map_err(abi)?;
            let _before_inbox_acc = fields.word().map_err(abi)?;
            let inbox = fields.address().map_err(abi)?;
            if inbox != contracts.inbox {
                continue;
            }
            let kind = MessageKind::from_byte(fields.u8().map_err(abi)?);
            let sender = fields.address().map_err(abi)?;
            let data_hash = fields.word().map_err(abi)?;
            let base_fee_l1 = fields.uint().map_err(abi)?;
            let timestamp = fields.u64().map_err(abi)?;
            let data = inbox_data(logs, contracts.inbox, number)?;
            messages.push(Message { number, kind, sender, base_fee_l1, timestamp, data_hash, data });
        }
        Ok(messages)
    }
}

/// The data the inbox logged for message `number`.
fn inbox_data(logs: &[Log], inbox: Address, number: U256) -> Result<Option<Vec<u8>>, BadLog> {
    for log in logs.iter().filter(|log| log.address == inbox && INBOX_MESSAGE_DELIVERED.matches(log)) {
        let tokens = INBOX_MESSAGE_DELIVERED.decode(log)?;
        if let [Token::Uint(logged), Token::Bytes(data)] = tokens.as_slice()
            && *logged == number
        {
            return Ok(Some(data.clone()));
        }
    }
    Ok(None)
}
