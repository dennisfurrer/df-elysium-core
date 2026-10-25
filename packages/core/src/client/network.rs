//! Elysium's networks. Testnet only: Kinetiq has not published mainnet's chain id, RPC or
//! contracts, so nothing here guesses them. Everything that differs between networks is a method
//! on [`Network`], so mainnet is one more arm when it is published.

use std::{fmt, str::FromStr};

use crate::B256;
use crate::bridge::Contracts;
use crate::utils::Address;

/// Elysium testnet's chain id.
pub const TESTNET_CHAIN_ID: u64 = 99_801;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Network {
    #[default]
    Testnet,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an Elysium network; expected testnet")]
pub struct UnknownNetwork(pub String);

impl Network {
    pub const ALL: [Network; 1] = [Network::Testnet];

    /// The chain every transaction is signed under.
    #[must_use]
    pub const fn chain_id(self) -> u64 {
        match self {
            Network::Testnet => TESTNET_CHAIN_ID,
        }
    }

    /// Kinetiq's public JSON-RPC: HTTPS only, no websocket, no mempool, and `eth_getLogs` over at
    /// most [`Network::log_span`] blocks unless the filter names an address or a topic. Now and then
    /// a backend behind it lags a block and refuses a read ([`crate::client::rpc::lagging`]).
    #[must_use]
    pub const fn rpc_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://testnet-rpc.elysium.kinetiq.xyz",
        }
    }

    /// Conduit's RPC for the same chain, the one its explorer hands wallets.
    #[must_use]
    pub const fn conduit_rpc_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://rpc-elysium-testnet.t.conduit.xyz",
        }
    }

    /// The most blocks one unfiltered `eth_getLogs` may span on [`Network::rpc_url`]:
    /// `toBlock - fromBlock`, refused past it with `-32602` and HTTP 400.
    #[must_use]
    pub const fn log_span(self) -> u64 {
        match self {
            Network::Testnet => 2_000,
        }
    }

    /// Conduit's explorer for the chain.
    #[must_use]
    pub const fn explorer_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://test-explorer.elysium.kinetiq.xyz",
        }
    }

    /// A transaction on the explorer, for a link a person follows.
    #[must_use]
    pub fn tx_url(self, hash: B256) -> String {
        format!("{}/tx/{hash}", self.explorer_url())
    }

    /// An account or a contract on the explorer.
    #[must_use]
    pub fn address_url(self, address: Address) -> String {
        format!("{}/address/{address}", self.explorer_url())
    }

    /// A block on the explorer.
    #[must_use]
    pub fn block_url(self, number: u64) -> String {
        format!("{}/block/{number}", self.explorer_url())
    }

    /// Conduit's Sourcify, where Elysium's contracts are verified ([`crate::deploy::verify`]).
    #[must_use]
    pub const fn verifier_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://contracts.conduit.xyz",
        }
    }

    /// Kinetiq's faucet page, for testnet HYPE.
    #[must_use]
    pub const fn faucet_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://elysium.kinetiq.xyz/testnet-faucet",
        }
    }

    /// Kinetiq's bridge page, between HyperEVM and Elysium.
    #[must_use]
    pub const fn bridge_url(self) -> &'static str {
        match self {
            Network::Testnet => "https://elysium.kinetiq.xyz/testnet-bridge",
        }
    }

    /// The Hyperliquid network whose HyperEVM Elysium settles to: its batches, assertions and
    /// bridge live there ([`df_hyperliquid_core::Network::evm_rpc_url`]).
    #[must_use]
    pub const fn hyperliquid(self) -> df_hyperliquid_core::Network {
        match self {
            Network::Testnet => df_hyperliquid_core::Network::Testnet,
        }
    }

    /// The parent chain's id: HyperEVM's.
    #[must_use]
    pub const fn parent_chain_id(self) -> u64 {
        self.hyperliquid().evm_chain_id()
    }

    /// The rollup's contracts on the parent chain.
    #[must_use]
    pub const fn contracts(self) -> Contracts {
        match self {
            Network::Testnet => crate::bridge::contracts::TESTNET,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Network::Testnet => "testnet",
        }
    }

    #[must_use]
    pub const fn is_mainnet(self) -> bool {
        match self {
            Network::Testnet => false,
        }
    }

    /// The network whose chain id this is.
    #[must_use]
    pub const fn of_chain(chain_id: u64) -> Option<Network> {
        match chain_id {
            TESTNET_CHAIN_ID => Some(Network::Testnet),
            _ => None,
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Network {
    type Err = UnknownNetwork;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "testnet" | "test" => Ok(Network::Testnet),
            _ => Err(UnknownNetwork(text.trim().to_owned())),
        }
    }
}

#[cfg(test)]
#[path = "tests/network.rs"]
mod tests;
