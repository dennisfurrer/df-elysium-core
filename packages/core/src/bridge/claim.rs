//! Claiming a withdrawal on HyperEVM. The rollup (BoLD) confirms assertions about Elysium's state;
//! each names an Elysium block, and that block's send count and send root are the outbox's tree up
//! to it. A withdrawal at a position under the latest confirmed send count is executed on the
//! outbox with a proof against that root, built by the NodeInterface on Elysium.
//!
//! [`claim`] does the whole read: the latest confirmed assertion, the block it names, the proof,
//! and the call that executes the withdrawal, or why it cannot yet. Anyone may send that call,
//! from any address with HYPE on the parent chain for its gas; the HYPE goes to the withdrawal's
//! destination, and a leaf is executed once.

use df_hyperliquid_core::client::Http;
use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::bridge::Contracts;
use crate::bridge::withdraw::Withdrawal;
use crate::client::read::{Fields, one, one_word};
use crate::client::rpc::{BlockTag, LogFilter, Rpc, RpcError};
use crate::client::{Arg, BadLog, Call, Client, Event, Log, Method, Read, ReadError};
use crate::nitro::node::construct_outbox_proof;
use crate::{B256, U256};

pub const LATEST_CONFIRMED: Method = Method::new("latestConfirmed()", "bytes32");
pub const GET_ASSERTION: Method = Method::new(
    "getAssertion(bytes32)",
    "(uint64 firstChildBlock, uint64 secondChildBlock, uint64 createdAtBlock, bool isFirstChild, uint8 status, \
     bytes32 configHash)",
);
pub const CONFIRM_PERIOD_BLOCKS: Method = Method::new("confirmPeriodBlocks()", "uint64");

pub const EXECUTE_TRANSACTION: Method =
    Method::new("executeTransaction(bytes32[],uint256,address,address,uint256,uint256,uint256,uint256,bytes)", "");
pub const IS_SPENT: Method = Method::new("isSpent(uint256)", "bool");
pub const ROOTS: Method = Method::new("roots(bytes32)", "bytes32");

/// An assertion made: its state after, whose global state names an Elysium block and send root.
pub const ASSERTION_CREATED: Event = Event::new(
    "AssertionCreated(bytes32,bytes32,((bytes32,bytes32,(bytes32,uint256,address,uint64,uint64)),\
     ((bytes32[2],uint64[2]),uint8,bytes32),((bytes32[2],uint64[2]),uint8,bytes32)),bytes32,uint256,bytes32,\
     uint256,address,uint64)",
    "bytes32 indexed assertionHash, bytes32 indexed parentAssertionHash, \
     ((bytes32 prevPrevAssertionHash, bytes32 sequencerBatchAcc, (bytes32 wasmModuleRoot, uint256 requiredStake, \
     address challengeManager, uint64 confirmPeriodBlocks, uint64 nextInboxPosition) configData) beforeStateData, \
     ((bytes32[2] bytes32Vals, uint64[2] u64Vals) globalState, uint8 machineStatus, bytes32 endHistoryRoot) \
     beforeState, ((bytes32[2] bytes32Vals, uint64[2] u64Vals) globalState, uint8 machineStatus, \
     bytes32 endHistoryRoot) afterState) assertion, bytes32 afterInboxBatchAcc, uint256 inboxMaxCount, \
     bytes32 wasmModuleRoot, uint256 requiredStake, address challengeManager, uint64 confirmPeriodBlocks",
);
pub const ASSERTION_CONFIRMED: Event = Event::new(
    "AssertionConfirmed(bytes32,bytes32,bytes32)",
    "bytes32 indexed assertionHash, bytes32 blockHash, bytes32 sendRoot",
);
pub const OUT_BOX_TRANSACTION_EXECUTED: Event = Event::new(
    "OutBoxTransactionExecuted(address,address,uint256,uint256)",
    "address indexed to, address indexed l2Sender, uint256 indexed zero, uint256 transactionIndex",
);
pub const SEND_ROOT_UPDATED: Event =
    Event::new("SendRootUpdated(bytes32,bytes32)", "bytes32 indexed outputRoot, bytes32 indexed l2BlockHash");

/// The latest assertion the rollup has confirmed.
#[must_use]
pub fn latest_confirmed(contracts: &Contracts) -> Read<B256> {
    Read::new(contracts.rollup, &LATEST_CONFIRMED, &[], |answer| one_word(&LATEST_CONFIRMED, answer))
}

/// Where an assertion stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssertionStatus {
    NoAssertion,
    Pending,
    Confirmed,
    Other(u8),
}

/// An assertion as the rollup keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assertion {
    pub first_child_block: u64,
    pub second_child_block: u64,
    /// The parent block it was created in: where its `AssertionCreated` log is.
    pub created_at_block: u64,
    pub is_first_child: bool,
    pub status: AssertionStatus,
    pub config_hash: B256,
}

#[must_use]
pub fn assertion(contracts: &Contracts, hash: B256) -> Read<Assertion> {
    Read::new(contracts.rollup, &GET_ASSERTION, &[Arg::Word(hash)], |answer| {
        let tokens = GET_ASSERTION.answer(answer)?;
        let mut fields = Fields::tuple("getAssertion", &tokens)?;
        Ok(Assertion {
            first_child_block: fields.u64()?,
            second_child_block: fields.u64()?,
            created_at_block: fields.u64()?,
            is_first_child: fields.bool()?,
            status: match fields.u8()? {
                0 => AssertionStatus::NoAssertion,
                1 => AssertionStatus::Pending,
                2 => AssertionStatus::Confirmed,
                other => AssertionStatus::Other(other),
            },
            config_hash: fields.word()?,
        })
    })
}

/// Parent blocks an assertion must wait, unchallenged, to be confirmed.
#[must_use]
pub fn confirm_period_blocks(contracts: &Contracts) -> Read<u64> {
    Read::new(contracts.rollup, &CONFIRM_PERIOD_BLOCKS, &[], |answer| {
        one(&CONFIRM_PERIOD_BLOCKS, answer, Token::as_u64)
    })
}

/// Whether the outbox has executed the leaf at `position`.
#[must_use]
pub fn is_spent(contracts: &Contracts, position: u64) -> Read<bool> {
    Read::new(contracts.outbox, &IS_SPENT, &[Arg::u64(position)], |answer| one(&IS_SPENT, answer, Token::as_bool))
}

/// The Elysium block a confirmed send root is from; zero for a root the outbox does not know.
#[must_use]
pub fn root(contracts: &Contracts, send_root: B256) -> Read<B256> {
    Read::new(contracts.outbox, &ROOTS, &[Arg::Word(send_root)], |answer| one_word(&ROOTS, answer))
}

/// Executes `withdrawal` on the outbox with `proof` ([`construct_outbox_proof`]).
#[must_use]
pub fn execute(contracts: &Contracts, withdrawal: &Withdrawal, proof: &[B256]) -> Call {
    let data = EXECUTE_TRANSACTION.calldata(&[
        Arg::Words(proof),
        Arg::u64(withdrawal.position),
        Arg::Address(withdrawal.caller),
        Arg::Address(withdrawal.destination),
        Arg::u64(withdrawal.arb_block),
        Arg::u64(withdrawal.eth_block),
        Arg::u64(withdrawal.timestamp),
        Arg::Uint(withdrawal.value),
        Arg::Bytes(&withdrawal.data),
    ]);
    Call { to: Some(contracts.outbox), value: U256::ZERO, data }
}

/// What a confirmed assertion says of Elysium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmed {
    pub assertion: B256,
    /// The Elysium block it confirms up to.
    pub block_hash: B256,
    /// The outbox's root at that block.
    pub send_root: B256,
}

/// The filter that finds an assertion's `AssertionCreated` log: the rollup's, at the block it was
/// created in. One block, inside HyperEVM's 50-block window.
#[must_use]
pub fn assertion_created_filter(contracts: &Contracts, hash: B256, created_at_block: u64) -> LogFilter {
    LogFilter {
        from_block: Some(BlockTag::Number(created_at_block)),
        to_block: Some(BlockTag::Number(created_at_block)),
        block_hash: None,
        address: vec![contracts.rollup],
        topics: vec![Some(vec![ASSERTION_CREATED.topic()]), Some(vec![hash])],
    }
}

/// The Elysium block and send root an `AssertionCreated` log names in its state after.
pub fn read_assertion_created(log: &Log) -> Result<Confirmed, BadLog> {
    let tokens = ASSERTION_CREATED.decode(log)?;
    let misfit =
        || BadLog::Abi { event: "AssertionCreated", error: AbiError::Type("not the declared state".to_owned()) };
    let word = |token: &Token| token.as_bytes().and_then(|bytes| <[u8; 32]>::try_from(bytes).ok()).map(B256);
    let assertion = tokens.first().and_then(word).ok_or_else(misfit)?;
    let words = tokens
        .get(2)
        .and_then(Token::as_items)
        .and_then(|inputs| inputs.get(2))
        .and_then(Token::as_items)
        .and_then(|after| after.first())
        .and_then(Token::as_items)
        .and_then(|global| global.first())
        .and_then(Token::as_items)
        .ok_or_else(misfit)?;
    match words {
        [block_hash, send_root] => Ok(Confirmed {
            assertion,
            block_hash: word(block_hash).ok_or_else(misfit)?,
            send_root: word(send_root).ok_or_else(misfit)?,
        }),
        _ => Err(misfit()),
    }
}

/// Why a claim could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClaimError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Log(#[from] BadLog),
    #[error("no AssertionCreated log for {assertion} at parent block {block}")]
    NoAssertionLog { assertion: B256, block: u64 },
    #[error("Elysium has no block {0}, which a confirmed assertion names")]
    UnknownBlock(B256),
    #[error("the {what} disagree: {left} and {right}")]
    Mismatch { what: &'static str, left: B256, right: B256 },
}

/// Where a withdrawal stands on the parent chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// Executed already: the HYPE was paid.
    Spent,
    /// Not under a confirmed assertion yet: the latest confirms the first `confirmed` leaves.
    Waiting { confirmed: u64 },
    /// Ready: send this call on the parent chain.
    Ready { call: Call, confirmed: Confirmed },
}

/// Where `withdrawal` stands, read from Elysium through `elysium` and from HyperEVM through
/// `parent`, and the call that claims it once it can be.
pub async fn claim<H: Http, S, P: Http>(
    elysium: &Client<H, S>,
    parent: &Rpc<P>,
    withdrawal: &Withdrawal,
) -> Result<Claim, ClaimError> {
    let contracts = elysium.network().contracts();
    if is_spent(&contracts, withdrawal.position).on(parent, BlockTag::Latest).await? {
        return Ok(Claim::Spent);
    }
    let confirmed = latest(&contracts, parent).await?;
    let block = elysium.block_by_hash(confirmed.block_hash, false).await?;
    let block = block.ok_or(ClaimError::UnknownBlock(confirmed.block_hash))?;
    if block.send_root != confirmed.send_root {
        return Err(ClaimError::Mismatch {
            what: "assertion's and block's send roots",
            left: confirmed.send_root,
            right: block.send_root,
        });
    }
    if withdrawal.position >= block.send_count {
        return Ok(Claim::Waiting { confirmed: block.send_count });
    }
    let proof = elysium.read(&construct_outbox_proof(block.send_count, withdrawal.position)).await?;
    if proof.root != confirmed.send_root {
        return Err(ClaimError::Mismatch {
            what: "proof's and assertion's roots",
            left: proof.root,
            right: confirmed.send_root,
        });
    }
    Ok(Claim::Ready { call: execute(&contracts, withdrawal, &proof.proof), confirmed })
}

/// The latest confirmed assertion and what it says, read from the parent chain.
pub async fn latest<P: Http>(contracts: &Contracts, parent: &Rpc<P>) -> Result<Confirmed, ClaimError> {
    let hash = latest_confirmed(contracts).on(parent, BlockTag::Latest).await?;
    let created = assertion(contracts, hash).on(parent, BlockTag::Latest).await?.created_at_block;
    let logs = parent.logs(&assertion_created_filter(contracts, hash, created)).await?;
    let log = logs.iter().find(|log| ASSERTION_CREATED.matches(log) && log.topics.get(1) == Some(&hash));
    Ok(read_assertion_created(log.ok_or(ClaimError::NoAssertionLog { assertion: hash, block: created })?)?)
}

#[cfg(test)]
#[path = "tests/claim.rs"]
mod tests;
