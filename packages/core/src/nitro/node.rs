//! The NodeInterface at `0xc8`: no code, answered by the node itself, through `eth_call` (and
//! `eth_estimateGas` for [`estimate_retryable_ticket`]). It splits a gas estimate between
//! Elysium's execution and HyperEVM's data, builds the Merkle proof a withdrawal is executed with
//! on the parent chain, and maps blocks to the batches and parent blocks that carry them.

use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::client::read::{Fields, one};
use crate::client::{Arg, Call, Method, Read};
use crate::gas::Estimate;
use crate::nitro::table::NODE_INTERFACE;
use crate::utils::Address;
use crate::{B256, U256};

pub const GAS_ESTIMATE_COMPONENTS: Method = Method::new(
    "gasEstimateComponents(address,bool,bytes)",
    "uint64 gasEstimate, uint64 gasEstimateForL1, uint256 baseFee, uint256 l1BaseFeeEstimate",
);
pub const GAS_ESTIMATE_L1_COMPONENT: Method = Method::new(
    "gasEstimateL1Component(address,bool,bytes)",
    "uint64 gasEstimateForL1, uint256 baseFee, uint256 l1BaseFeeEstimate",
);
pub const CONSTRUCT_OUTBOX_PROOF: Method =
    Method::new("constructOutboxProof(uint64,uint64)", "bytes32 send, bytes32 root, bytes32[] proof");
pub const FIND_BATCH_CONTAINING_BLOCK: Method = Method::new("findBatchContainingBlock(uint64)", "uint64 batch");
pub const GET_L1_CONFIRMATIONS: Method = Method::new("getL1Confirmations(bytes32)", "uint64 confirmations");
pub const ESTIMATE_RETRYABLE_TICKET: Method =
    Method::new("estimateRetryableTicket(address,uint256,address,uint256,address,address,bytes)", "");
pub const NITRO_GENESIS_BLOCK: Method = Method::new("nitroGenesisBlock()", "uint256 number");
pub const BLOCK_L1_NUM: Method = Method::new("blockL1Num(uint64)", "uint64 l1BlockNum");
pub const L2_BLOCK_RANGE_FOR_L1: Method =
    Method::new("l2BlockRangeForL1(uint64)", "uint64 firstBlock, uint64 lastBlock");

/// Every method of the NodeInterface this crate calls.
pub const METHODS: &[Method] = &[
    GAS_ESTIMATE_COMPONENTS,
    GAS_ESTIMATE_L1_COMPONENT,
    CONSTRUCT_OUTBOX_PROOF,
    FIND_BATCH_CONTAINING_BLOCK,
    GET_L1_CONFIRMATIONS,
    ESTIMATE_RETRYABLE_TICKET,
    NITRO_GENESIS_BLOCK,
    BLOCK_L1_NUM,
    L2_BLOCK_RANGE_FOR_L1,
];

/// The gas `call` takes when `from` sends it: the whole estimate, its part for HyperEVM's data,
/// and the two base fees it was priced at.
#[must_use]
pub fn gas_estimate_components(from: Option<Address>, call: &Call) -> Read<Estimate> {
    let to = call.to.unwrap_or(Address::ZERO);
    let args = [Arg::Address(to), Arg::Bool(call.to.is_none()), Arg::Bytes(&call.data)];
    let read = Read::new(NODE_INTERFACE, &GAS_ESTIMATE_COMPONENTS, &args, |answer| {
        let tokens = GAS_ESTIMATE_COMPONENTS.answer(answer)?;
        let mut fields = Fields::of("gasEstimateComponents", &tokens);
        Ok(Estimate {
            gas: fields.u64()?,
            gas_for_l1: fields.u64()?,
            base_fee: fields.uint()?,
            l1_base_fee: fields.uint()?,
        })
    })
    .with_value(call.value);
    match from {
        Some(from) => read.from(from),
        None => read,
    }
}

/// Just the part of [`gas_estimate_components`] for HyperEVM's data, which does not run the call.
#[must_use]
pub fn gas_estimate_l1_component(call: &Call) -> Read<(u64, U256, U256)> {
    let to = call.to.unwrap_or(Address::ZERO);
    let args = [Arg::Address(to), Arg::Bool(call.to.is_none()), Arg::Bytes(&call.data)];
    Read::new(NODE_INTERFACE, &GAS_ESTIMATE_L1_COMPONENT, &args, |answer| {
        let tokens = GAS_ESTIMATE_L1_COMPONENT.answer(answer)?;
        let mut fields = Fields::of("gasEstimateL1Component", &tokens);
        Ok((fields.u64()?, fields.uint()?, fields.uint()?))
    })
}

/// The proof a leaf of the outbox's tree is executed with on the parent chain: the leaf's hash,
/// the root of the tree at `size` leaves, and the path between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxProof {
    pub send: B256,
    pub root: B256,
    pub proof: Vec<B256>,
}

/// The proof of leaf `leaf` in the tree of `size` leaves: `size` is a confirmed block's send
/// count ([`crate::bridge::claim()`]), and the root must be the one the outbox knows.
#[must_use]
pub fn construct_outbox_proof(size: u64, leaf: u64) -> Read<OutboxProof> {
    Read::new(NODE_INTERFACE, &CONSTRUCT_OUTBOX_PROOF, &[Arg::u64(size), Arg::u64(leaf)], |answer| {
        let tokens = CONSTRUCT_OUTBOX_PROOF.answer(answer)?;
        let mut fields = Fields::of("constructOutboxProof", &tokens);
        Ok(OutboxProof { send: fields.word()?, root: fields.word()?, proof: fields.words()? })
    })
}

/// The batch that carries block `number` to HyperEVM; reverts for one not yet posted.
#[must_use]
pub fn find_batch_containing_block(number: u64) -> Read<u64> {
    Read::new(NODE_INTERFACE, &FIND_BATCH_CONTAINING_BLOCK, &[Arg::u64(number)], |answer| {
        one(&FIND_BATCH_CONTAINING_BLOCK, answer, Token::as_u64)
    })
}

/// How many HyperEVM blocks have been built on the one holding the batch with this block: zero
/// until the batch is posted.
#[must_use]
pub fn l1_confirmations(block_hash: B256) -> Read<u64> {
    Read::new(NODE_INTERFACE, &GET_L1_CONFIRMATIONS, &[Arg::Word(block_hash)], |answer| {
        one(&GET_L1_CONFIRMATIONS, answer, Token::as_u64)
    })
}

/// The first block of Nitro's history: zero on Elysium.
#[must_use]
pub fn nitro_genesis_block() -> Read<U256> {
    Read::new(NODE_INTERFACE, &NITRO_GENESIS_BLOCK, &[], |answer| one(&NITRO_GENESIS_BLOCK, answer, Token::as_u256))
}

/// The parent block a block was sequenced at.
#[must_use]
pub fn block_l1_num(number: u64) -> Read<u64> {
    Read::new(NODE_INTERFACE, &BLOCK_L1_NUM, &[Arg::u64(number)], |answer| one(&BLOCK_L1_NUM, answer, Token::as_u64))
}

/// The first and last of Elysium's blocks sequenced at parent block `l1_block`.
#[must_use]
pub fn l2_block_range_for_l1(l1_block: u64) -> Read<(u64, u64)> {
    Read::new(NODE_INTERFACE, &L2_BLOCK_RANGE_FOR_L1, &[Arg::u64(l1_block)], |answer| {
        let tokens = L2_BLOCK_RANGE_FOR_L1.answer(answer)?;
        let mut fields = Fields::of("l2BlockRangeForL1", &tokens);
        Ok((fields.u64()?, fields.u64()?))
    })
}

/// A retryable's call, to be gas-estimated here with `eth_estimateGas` as if its ticket had
/// arrived: from `sender` on HyperEVM, unaliased (the node aliases it), with `deposit` wei to spend
/// (a HYPE over the call's value covers any gas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryableEstimate {
    pub sender: Address,
    pub deposit: U256,
    pub to: Address,
    pub l2_call_value: U256,
    pub excess_fee_refund_address: Address,
    pub call_value_refund_address: Address,
    pub data: Vec<u8>,
}

/// The call `eth_estimateGas` takes to estimate a retryable's gas limit on Elysium.
#[must_use]
pub fn estimate_retryable_ticket(ticket: &RetryableEstimate) -> Call {
    let data = ESTIMATE_RETRYABLE_TICKET.calldata(&[
        Arg::Address(ticket.sender),
        Arg::Uint(ticket.deposit),
        Arg::Address(ticket.to),
        Arg::Uint(ticket.l2_call_value),
        Arg::Address(ticket.excess_fee_refund_address),
        Arg::Address(ticket.call_value_refund_address),
        Arg::Bytes(&ticket.data),
    ]);
    Call { to: Some(NODE_INTERFACE), value: U256::ZERO, data }
}

/// The fields of an [`OutboxProof`] answer, for a caller that has the bytes already.
pub fn read_outbox_proof(answer: &[u8]) -> Result<OutboxProof, AbiError> {
    construct_outbox_proof(0, 0).decode(answer)
}

#[cfg(test)]
#[path = "tests/node.rs"]
mod tests;
