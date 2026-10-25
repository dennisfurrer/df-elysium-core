//! ArbSys at `0x64`: the chain's own facts (its height, id and ArbOS version, block hashes, how
//! the caller was aliased) and the way out to the parent chain. `withdrawEth` and `sendTxToL1`
//! add a leaf to the outbox's tree and log it as [`L2_TO_L1_TX`]; once an assertion past it is
//! confirmed on HyperEVM, the leaf is executed there ([`crate::bridge::claim()`]).
//!
//! Inside a contract `block.number` is the parent chain's; [`arb_block_number`] is Elysium's.

use df_hyperliquid_core::client::evm::abi::{AbiError, Token};

use crate::client::read::{Fields, one, one_word};
use crate::client::{Arg, Call, Event, Method, Read};
use crate::nitro::table::{ARB_SYS, ARBOS_VERSION_OFFSET};
use crate::utils::Address;
use crate::{B256, U256};

pub const ARB_BLOCK_NUMBER: Method = Method::new("arbBlockNumber()", "uint256");
pub const ARB_BLOCK_HASH: Method = Method::new("arbBlockHash(uint256)", "bytes32");
pub const ARB_CHAIN_ID: Method = Method::new("arbChainID()", "uint256");
pub const ARB_OS_VERSION: Method = Method::new("arbOSVersion()", "uint256");
pub const GET_STORAGE_GAS_AVAILABLE: Method = Method::new("getStorageGasAvailable()", "uint256");
pub const IS_TOP_LEVEL_CALL: Method = Method::new("isTopLevelCall()", "bool");
pub const MAP_L1_SENDER_CONTRACT_ADDRESS_TO_L2_ALIAS: Method =
    Method::new("mapL1SenderContractAddressToL2Alias(address,address)", "address");
pub const WAS_MY_CALLERS_ADDRESS_ALIASED: Method = Method::new("wasMyCallersAddressAliased()", "bool");
pub const MY_CALLERS_ADDRESS_WITHOUT_ALIASING: Method = Method::new("myCallersAddressWithoutAliasing()", "address");
pub const SEND_TX_TO_L1: Method = Method::new("sendTxToL1(address,bytes)", "uint256");
pub const SEND_MERKLE_TREE_STATE: Method =
    Method::new("sendMerkleTreeState()", "uint256 size, bytes32 root, bytes32[] partials");
pub const WITHDRAW_ETH: Method = Method::new("withdrawEth(address)", "uint256");

/// Every method of ArbSys this crate calls.
pub const METHODS: &[Method] = &[
    ARB_BLOCK_NUMBER,
    ARB_BLOCK_HASH,
    ARB_CHAIN_ID,
    ARB_OS_VERSION,
    GET_STORAGE_GAS_AVAILABLE,
    IS_TOP_LEVEL_CALL,
    MAP_L1_SENDER_CONTRACT_ADDRESS_TO_L2_ALIAS,
    WAS_MY_CALLERS_ADDRESS_ALIASED,
    MY_CALLERS_ADDRESS_WITHOUT_ALIASING,
    SEND_TX_TO_L1,
    SEND_MERKLE_TREE_STATE,
    WITHDRAW_ETH,
];

/// One send to the parent chain, a leaf of the outbox's tree at `position`.
pub const L2_TO_L1_TX: Event = Event::new(
    "L2ToL1Tx(address,address,uint256,uint256,uint256,uint256,uint256,uint256,bytes)",
    "address caller, address indexed destination, uint256 indexed hash, uint256 indexed position, \
     uint256 arbBlockNum, uint256 ethBlockNum, uint256 timestamp, uint256 callvalue, bytes data",
);

/// The outbox's tree after a send: one per level it touched.
pub const SEND_MERKLE_UPDATE: Event = Event::new(
    "SendMerkleUpdate(uint256,bytes32,uint256)",
    "uint256 indexed reserved, bytes32 indexed hash, uint256 indexed position",
);

fn small(method: &Method, answer: &[u8]) -> Result<u64, AbiError> {
    one(method, answer, |token| token.as_u64())
}

/// Elysium's block height: inside a contract, `block.number` is HyperEVM's.
#[must_use]
pub fn arb_block_number() -> Read<u64> {
    Read::new(ARB_SYS, &ARB_BLOCK_NUMBER, &[], |answer| small(&ARB_BLOCK_NUMBER, answer))
}

/// The hash of one of the last 256 blocks.
#[must_use]
pub fn arb_block_hash(number: u64) -> Read<B256> {
    Read::new(ARB_SYS, &ARB_BLOCK_HASH, &[Arg::u64(number)], |answer| one_word(&ARB_BLOCK_HASH, answer))
}

#[must_use]
pub fn arb_chain_id() -> Read<u64> {
    Read::new(ARB_SYS, &ARB_CHAIN_ID, &[], |answer| small(&ARB_CHAIN_ID, answer))
}

/// ArbOS's version plus [`ARBOS_VERSION_OFFSET`]: 116 for ArbOS 61.
#[must_use]
pub fn arb_os_version() -> Read<u64> {
    Read::new(ARB_SYS, &ARB_OS_VERSION, &[], |answer| small(&ARB_OS_VERSION, answer))
}

/// ArbOS's version from what [`arb_os_version`] answers.
#[must_use]
pub const fn arbos(reported: u64) -> u64 {
    reported.saturating_sub(ARBOS_VERSION_OFFSET)
}

/// Storage gas left in the block: always zero on Nitro, which prices storage as execution.
#[must_use]
pub fn storage_gas_available() -> Read<U256> {
    Read::new(ARB_SYS, &GET_STORAGE_GAS_AVAILABLE, &[], |answer| {
        one(&GET_STORAGE_GAS_AVAILABLE, answer, Token::as_u256)
    })
}

#[must_use]
pub fn is_top_level_call() -> Read<bool> {
    Read::new(ARB_SYS, &IS_TOP_LEVEL_CALL, &[], |answer| one(&IS_TOP_LEVEL_CALL, answer, Token::as_bool))
}

/// The address a contract on the parent chain acts as here ([`crate::bridge::alias`]), as ArbOS
/// computes it.
#[must_use]
pub fn alias_of(sender: Address) -> Read<Address> {
    let method = &MAP_L1_SENDER_CONTRACT_ADDRESS_TO_L2_ALIAS;
    Read::new(ARB_SYS, method, &[Arg::Address(sender), Arg::Address(Address::ZERO)], |answer| {
        one(&MAP_L1_SENDER_CONTRACT_ADDRESS_TO_L2_ALIAS, answer, Token::as_address)
    })
}

#[must_use]
pub fn was_my_callers_address_aliased() -> Read<bool> {
    Read::new(ARB_SYS, &WAS_MY_CALLERS_ADDRESS_ALIASED, &[], |answer| {
        one(&WAS_MY_CALLERS_ADDRESS_ALIASED, answer, Token::as_bool)
    })
}

#[must_use]
pub fn my_callers_address_without_aliasing() -> Read<Address> {
    Read::new(ARB_SYS, &MY_CALLERS_ADDRESS_WITHOUT_ALIASING, &[], |answer| {
        one(&MY_CALLERS_ADDRESS_WITHOUT_ALIASING, answer, Token::as_address)
    })
}

/// The outbox's tree: how many leaves, its root, and the partial roots it is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleTreeState {
    pub size: U256,
    pub root: B256,
    pub partials: Vec<B256>,
}

/// The outbox's tree as ArbOS keeps it. ArbSys answers it only to the zero address, so the read
/// is sent from it.
#[must_use]
pub fn send_merkle_tree_state() -> Read<MerkleTreeState> {
    Read::new(ARB_SYS, &SEND_MERKLE_TREE_STATE, &[], |answer| {
        let tokens = SEND_MERKLE_TREE_STATE.answer(answer)?;
        let mut fields = Fields::of("sendMerkleTreeState", &tokens);
        Ok(MerkleTreeState { size: fields.uint()?, root: fields.word()?, partials: fields.words()? })
    })
    .from(Address::ZERO)
}

/// Sends `value` wei of HYPE to `destination` on the parent chain: burnt here, claimable there
/// once confirmed ([`mod@crate::bridge::withdraw`]).
#[must_use]
pub fn withdraw_eth(destination: Address, value: U256) -> Call {
    Call { to: Some(ARB_SYS), value, data: WITHDRAW_ETH.calldata(&[Arg::Address(destination)]) }
}

/// A call of `data` on `destination` on the parent chain, carrying `value` wei, run there by
/// whoever executes it from the outbox.
#[must_use]
pub fn send_tx_to_l1(destination: Address, data: &[u8], value: U256) -> Call {
    Call { to: Some(ARB_SYS), value, data: SEND_TX_TO_L1.calldata(&[Arg::Address(destination), Arg::Bytes(data)]) }
}

#[cfg(test)]
#[path = "tests/sys.rs"]
mod tests;
