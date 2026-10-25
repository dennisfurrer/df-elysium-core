//! Where each precompile sits, in address order, and ArbOS's own sender.

use crate::utils::Address;

/// `0x` and nineteen zero bytes, then `at`.
const fn at(low: u8) -> Address {
    let mut bytes = [0u8; 20];
    bytes[19] = low;
    Address::from_bytes(bytes)
}

/// The chain's own facts and the way out to the parent chain.
pub const ARB_SYS: Address = at(0x64);
/// Balances and code, as a precompile reads them.
pub const ARB_INFO: Address = at(0x65);
/// Addresses compressed to indexes, for calldata that costs less to post.
pub const ARB_ADDRESS_TABLE: Address = at(0x66);
/// Who runs the chain and what it has scheduled, for anyone to read.
pub const ARB_OWNER_PUBLIC: Address = at(0x6b);
/// The gas model: prices in wei and in gas, the parent chain's data price, the backlog.
pub const ARB_GAS_INFO: Address = at(0x6c);
/// The batch posters and their fee collectors.
pub const ARB_AGGREGATOR: Address = at(0x6d);
/// Retryable tickets from the parent chain: redeem, keep alive, cancel.
pub const ARB_RETRYABLE_TX: Address = at(0x6e);
/// A snapshot of the chain's counts.
pub const ARB_STATISTICS: Address = at(0x6f);
/// The chain owner's: nothing a user can call.
pub const ARB_OWNER: Address = at(0x70);
/// Stylus: activating WebAssembly programs and their parameters.
pub const ARB_WASM: Address = at(0x71);
/// Stylus's program cache.
pub const ARB_WASM_CACHE: Address = at(0x72);
/// RPC only, with no code: gas estimates split between Elysium and HyperEVM, outbox proofs,
/// batches and confirmations.
pub const NODE_INTERFACE: Address = at(0xc8);

/// ArbOS itself: the sender and recipient of the internal transaction that opens every block.
pub const ARBOS: Address = match Address::parse_const("0x00000000000000000000000000000000000a4b05") {
    Some(address) => address,
    None => Address::ZERO,
};

/// `ArbSys.arbOSVersion()` answers ArbOS's version plus this: Nitro counted on from Classic.
pub const ARBOS_VERSION_OFFSET: u64 = 55;

/// One precompile: where it sits and what the Nitro interfaces call it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Precompile {
    pub address: Address,
    pub name: &'static str,
}

/// Every precompile a user reaches, and the NodeInterface, in address order.
pub const ALL: [Precompile; 12] = [
    Precompile { address: ARB_SYS, name: "ArbSys" },
    Precompile { address: ARB_INFO, name: "ArbInfo" },
    Precompile { address: ARB_ADDRESS_TABLE, name: "ArbAddressTable" },
    Precompile { address: ARB_OWNER_PUBLIC, name: "ArbOwnerPublic" },
    Precompile { address: ARB_GAS_INFO, name: "ArbGasInfo" },
    Precompile { address: ARB_AGGREGATOR, name: "ArbAggregator" },
    Precompile { address: ARB_RETRYABLE_TX, name: "ArbRetryableTx" },
    Precompile { address: ARB_STATISTICS, name: "ArbStatistics" },
    Precompile { address: ARB_OWNER, name: "ArbOwner" },
    Precompile { address: ARB_WASM, name: "ArbWasm" },
    Precompile { address: ARB_WASM_CACHE, name: "ArbWasmCache" },
    Precompile { address: NODE_INTERFACE, name: "NodeInterface" },
];

/// The precompile at `address`, if one sits there.
#[must_use]
pub fn precompile(address: Address) -> Option<&'static Precompile> {
    ALL.iter().find(|one| one.address == address)
}

#[cfg(test)]
#[path = "tests/table.rs"]
mod tests;
