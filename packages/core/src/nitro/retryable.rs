//! ArbRetryableTx at `0x6e`: retryable tickets, the calls a parent-chain transaction schedules
//! here ([`crate::bridge::retryable`]). A ticket's first redeem runs on its own as it is
//! submitted; if that runs out of gas, anyone may [`redeem`] it until it times out, its
//! beneficiary may [`cancel`] it, and anyone may pay to keep it alive a lifetime longer.
//!
//! Asking about a ticket that was redeemed, cancelled or has expired reverts with
//! `NoTicketWithID()` ([`no_ticket`]).

use df_hyperliquid_core::client::evm::abi::Token;

use crate::client::read::{one, one_word};
use crate::client::rpc::RpcError;
use crate::client::{Arg, Call, Event, Method, Read, ReadError};
use crate::nitro::table::ARB_RETRYABLE_TX;
use crate::utils::{Address, keccak256};
use crate::{B256, U256};

pub const REDEEM: Method = Method::new("redeem(bytes32)", "bytes32");
pub const GET_LIFETIME: Method = Method::new("getLifetime()", "uint256");
pub const GET_TIMEOUT: Method = Method::new("getTimeout(bytes32)", "uint256");
pub const KEEPALIVE: Method = Method::new("keepalive(bytes32)", "uint256");
pub const GET_BENEFICIARY: Method = Method::new("getBeneficiary(bytes32)", "address");
pub const CANCEL: Method = Method::new("cancel(bytes32)", "");
pub const GET_CURRENT_REDEEMER: Method = Method::new("getCurrentRedeemer()", "address");

/// Every method of ArbRetryableTx a user calls.
pub const METHODS: &[Method] =
    &[REDEEM, GET_LIFETIME, GET_TIMEOUT, KEEPALIVE, GET_BENEFICIARY, CANCEL, GET_CURRENT_REDEEMER];

pub const TICKET_CREATED: Event = Event::new("TicketCreated(bytes32)", "bytes32 indexed ticketId");
pub const LIFETIME_EXTENDED: Event =
    Event::new("LifetimeExtended(bytes32,uint256)", "bytes32 indexed ticketId, uint256 newTimeout");
/// A redeem scheduled: its transaction's hash, and the gas it was given.
pub const REDEEM_SCHEDULED: Event = Event::new(
    "RedeemScheduled(bytes32,bytes32,uint64,uint64,address,uint256,uint256)",
    "bytes32 indexed ticketId, bytes32 indexed retryTxHash, uint64 indexed sequenceNum, uint64 donatedGas, \
     address gasDonor, uint256 maxRefund, uint256 submissionFeeRefund",
);
pub const CANCELED: Event = Event::new("Canceled(bytes32)", "bytes32 indexed ticketId");

/// The custom error a missing ticket reverts with.
pub const NO_TICKET_WITH_ID: &str = "NoTicketWithID()";

/// How long a new ticket lives, in seconds: a week on Elysium.
#[must_use]
pub fn lifetime() -> Read<U256> {
    Read::new(ARB_RETRYABLE_TX, &GET_LIFETIME, &[], |answer| one(&GET_LIFETIME, answer, Token::as_u256))
}

/// When a live ticket expires, in Unix seconds.
#[must_use]
pub fn timeout(ticket: B256) -> Read<U256> {
    Read::new(ARB_RETRYABLE_TX, &GET_TIMEOUT, &[Arg::Word(ticket)], |answer| one(&GET_TIMEOUT, answer, Token::as_u256))
}

/// Who may cancel a live ticket, and is paid its call value if it never runs.
#[must_use]
pub fn beneficiary(ticket: B256) -> Read<Address> {
    Read::new(ARB_RETRYABLE_TX, &GET_BENEFICIARY, &[Arg::Word(ticket)], |answer| {
        one(&GET_BENEFICIARY, answer, Token::as_address)
    })
}

/// The redeem running now, inside one; zero otherwise.
#[must_use]
pub fn current_redeemer() -> Read<Address> {
    Read::new(ARB_RETRYABLE_TX, &GET_CURRENT_REDEEMER, &[], |answer| {
        one(&GET_CURRENT_REDEEMER, answer, Token::as_address)
    })
}

/// Runs a ticket again with this transaction's gas; it answers the redeem's hash.
#[must_use]
pub fn redeem(ticket: B256) -> Call {
    Call { to: Some(ARB_RETRYABLE_TX), value: U256::ZERO, data: REDEEM.calldata(&[Arg::Word(ticket)]) }
}

/// Extends a ticket one lifetime past its timeout, paying for the storage.
#[must_use]
pub fn keepalive(ticket: B256) -> Call {
    Call { to: Some(ARB_RETRYABLE_TX), value: U256::ZERO, data: KEEPALIVE.calldata(&[Arg::Word(ticket)]) }
}

/// Cancels a ticket and refunds its call value to its beneficiary, who alone may send it.
#[must_use]
pub fn cancel(ticket: B256) -> Call {
    Call { to: Some(ARB_RETRYABLE_TX), value: U256::ZERO, data: CANCEL.calldata(&[Arg::Word(ticket)]) }
}

/// `NoTicketWithID()`'s selector.
#[must_use]
pub fn no_ticket_selector() -> [u8; 4] {
    let hash = keccak256(NO_TICKET_WITH_ID.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

/// Whether a read failed because the ticket is gone: redeemed, cancelled or expired.
#[must_use]
pub fn no_ticket(error: &ReadError) -> bool {
    let ReadError::Rpc(RpcError::Node { error, .. }) = error else { return false };
    let data = error.data.as_deref().and_then(|data| crate::utils::hex::decode(data).ok());
    data.as_deref().and_then(|data| data.get(..4)) == Some(&no_ticket_selector()[..])
}

/// What a [`redeem`] answers, simulated with `eth_call`: the hash its redeem will run under.
#[must_use]
pub fn redeem_hash(answer: &[u8]) -> Option<B256> {
    one_word(&REDEEM, answer).ok()
}

#[cfg(test)]
#[path = "tests/retryable.rs"]
mod tests;
