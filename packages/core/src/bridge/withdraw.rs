//! HYPE out, to HyperEVM: `ArbSys.withdrawEth(destination)` on Elysium, the HYPE as its value. It
//! is burnt here at once and added to the outbox's tree as a leaf, logged as `L2ToL1Tx`; once the
//! rollup confirms an assertion past it on the parent chain, anyone may execute it there from the
//! outbox ([`crate::bridge::claim()`]), paying HyperEVM's gas, and the HYPE is paid to
//! `destination`. `sendTxToL1` sends any call the same way.

use df_hyperliquid_core::client::evm::BadAmount;
use df_hyperliquid_core::client::evm::abi::AbiError;

use crate::client::read::Fields;
use crate::client::{BadLog, Call, Log};
use crate::nitro::sys::{self, L2_TO_L1_TX};
use crate::nitro::table::ARB_SYS;
use crate::utils::{Address, Dec, wei};
use crate::{B256, U256};

/// Withdraws `amount` HYPE to `destination` on the parent chain. Exact: a nineteenth place is
/// refused, never rounded.
pub fn withdraw(destination: Address, amount: Dec) -> Result<Call, BadAmount> {
    if !amount.is_positive() {
        return Err(BadAmount::NotPositive(amount));
    }
    Ok(sys::withdraw_eth(destination, wei(amount)?))
}

/// One send to the parent chain, as `L2ToL1Tx` logs it: everything its execution on the outbox
/// needs but the proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawal {
    /// Who sent it on Elysium: the outbox's `l2Sender`.
    pub caller: Address,
    pub destination: Address,
    /// The leaf's hash in the outbox's tree.
    pub hash: U256,
    /// The leaf's index: what the outbox marks spent.
    pub position: u64,
    pub arb_block: u64,
    /// The parent chain's block when it was sent.
    pub eth_block: u64,
    /// Unix seconds.
    pub timestamp: u64,
    /// HYPE it carries, in wei.
    pub value: U256,
    /// The call it makes on `destination`; empty for a plain withdrawal.
    pub data: Vec<u8>,
}

impl Withdrawal {
    /// The send an `L2ToL1Tx` log records. Only ArbSys writes them.
    pub fn from_log(log: &Log) -> Result<Withdrawal, BadLog> {
        if log.address != ARB_SYS {
            return Err(BadLog::NotThisEvent(L2_TO_L1_TX.name()));
        }
        let tokens = L2_TO_L1_TX.decode(log)?;
        let abi = |error: AbiError| BadLog::Abi { event: "L2ToL1Tx", error };
        let mut fields = Fields::of("L2ToL1Tx", &tokens);
        Ok(Withdrawal {
            caller: fields.address().map_err(abi)?,
            destination: fields.address().map_err(abi)?,
            hash: fields.uint().map_err(abi)?,
            position: fields.u64().map_err(abi)?,
            arb_block: fields.u64().map_err(abi)?,
            eth_block: fields.u64().map_err(abi)?,
            timestamp: fields.u64().map_err(abi)?,
            value: fields.uint().map_err(abi)?,
            data: fields.bytes().map_err(abi)?,
        })
    }

    /// Every send among a receipt's logs.
    pub fn in_logs(logs: &[Log]) -> Result<Vec<Withdrawal>, BadLog> {
        logs.iter().filter(|log| log.address == ARB_SYS && L2_TO_L1_TX.matches(log)).map(Withdrawal::from_log).collect()
    }

    /// The leaf's hash as a word.
    #[must_use]
    pub fn leaf(&self) -> B256 {
        B256::from(self.hash)
    }
}

#[cfg(test)]
#[path = "tests/withdraw.rs"]
mod tests;
