//! HYPE in, from HyperEVM: `Inbox.depositEth()` on the parent chain with the HYPE as its value.
//! HYPE is the parent chain's own coin, so it moves through the native bridge, 1:1, and arrives
//! as Elysium's gas a block or two after the parent transaction, as a deposit transaction
//! (type `0x64`) whose hash is known before it lands ([`Deposit::tx_hash`]).
//!
//! The deposit arrives at the depositor's own address when that is an account, and at its alias
//! when it is a contract; its `from` is always the alias.

use df_hyperliquid_core::client::evm::BadAmount;
use df_hyperliquid_core::client::evm::rlp;

use crate::bridge::message::{Message, MessageKind};
use crate::client::{Call, Method, Network};
use crate::utils::{Address, Dec, keccak256, wei};
use crate::{B256, U256};

pub const DEPOSIT_ETH: Method = Method::new("depositEth()", "uint256");

/// The type byte of a deposit on Elysium.
pub const DEPOSIT_TX: u8 = 0x64;

/// Deposits `amount` HYPE from the sender on the parent chain. Exact: a nineteenth place is
/// refused, never rounded.
pub fn deposit(network: Network, amount: Dec) -> Result<Call, BadAmount> {
    if !amount.is_positive() {
        return Err(BadAmount::NotPositive(amount));
    }
    Ok(deposit_wei(network, wei(amount)?))
}

/// Deposits `value` wei. The call goes to the parent chain's inbox, on
/// [`Network::parent_chain_id`].
#[must_use]
pub fn deposit_wei(network: Network, value: U256) -> Call {
    Call { to: Some(network.contracts().inbox), value, data: DEPOSIT_ETH.calldata(&[]) }
}

/// A deposit as Elysium runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deposit {
    /// The inbox's message number: the transaction's `requestId`.
    pub message_number: U256,
    /// The depositor's alias.
    pub from: Address,
    /// Who receives the HYPE.
    pub to: Address,
    pub value: U256,
}

impl Deposit {
    /// The deposit a delivered message makes; `None` for any other kind. Its data is the
    /// recipient's twenty bytes, then the value's thirty-two.
    #[must_use]
    pub fn from_message(message: &Message) -> Option<Deposit> {
        if message.kind != MessageKind::EthDeposit {
            return None;
        }
        let data = message.data.as_deref()?;
        let (to, value) = data.split_first_chunk::<20>()?;
        let value: &[u8; 32] = value.try_into().ok()?;
        Some(Deposit {
            message_number: message.number,
            from: message.sender,
            to: Address::from_bytes(*to),
            value: U256::from_be_bytes(*value),
        })
    }

    /// Its transaction's hash on `chain_id`:
    /// `keccak256(0x64 || rlp([chainId, requestId, from, to, value]))`, the request id as a word.
    #[must_use]
    pub fn tx_hash(&self, chain_id: u64) -> B256 {
        let fields = [
            rlp::uint(u128::from(chain_id)),
            rlp::bytes(&self.message_number.to_be_bytes()),
            rlp::bytes(self.from.bytes()),
            rlp::bytes(self.to.bytes()),
            rlp::u256(self.value),
        ];
        let mut typed = vec![DEPOSIT_TX];
        typed.extend(rlp::list(&fields));
        B256(keccak256(&typed))
    }
}

#[cfg(test)]
#[path = "tests/deposit.rs"]
mod tests;
