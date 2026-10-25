//! The rollup's contracts on HyperEVM, as Elysium's own rollup names them: read from
//! `Rollup.inbox()`, `outbox()`, `bridge()` and the rest on HyperEVM testnet, the rollup found
//! through the batches its poster delivers to the sequencer inbox.

use crate::utils::Address;

const fn address(text: &str) -> Address {
    match Address::parse_const(text) {
        Some(address) => address,
        None => Address::ZERO,
    }
}

/// One Elysium network's contracts on its parent chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Contracts {
    /// The rollup (BoLD): assertions, their confirmation, and the addresses below.
    pub rollup: Address,
    /// Holds what was bridged, and records every message both ways.
    pub bridge: Address,
    /// Where deposits and retryable tickets go in.
    pub inbox: Address,
    /// Where withdrawals come out, once an assertion past them is confirmed.
    pub outbox: Address,
    /// Where the batch poster delivers Elysium's batches.
    pub sequencer_inbox: Address,
    pub rollup_event_inbox: Address,
    pub challenge_manager: Address,
}

/// Elysium testnet's, on HyperEVM testnet (998).
pub const TESTNET: Contracts = Contracts {
    rollup: address("0xEbf08e34941fd93a0Fc1cD89dbBF7447267039Df"),
    bridge: address("0x36dAbc3CEE2E1084B7d04c8015Ef68Cac78f4f37"),
    inbox: address("0x11178Df40CBf87a4343f4B3F8B09B366ceFA8Ec0"),
    outbox: address("0x87391D602aaffB3Fe8051EcCDb75d6EEe8C31060"),
    sequencer_inbox: address("0xb5B1a0AebfFeF819Ec3F8c39a17b5C4E70A001E3"),
    rollup_event_inbox: address("0x5ad7539049Ba2184Be320977B866a6341f4A5F1C"),
    challenge_manager: address("0x0754429325CF02835CA17FcFBa9fc059cb2835B6"),
};
