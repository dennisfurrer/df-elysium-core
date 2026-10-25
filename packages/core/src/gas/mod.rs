//! What a transaction costs on Elysium. One gas figure pays for two things: Elysium's execution at
//! its base fee, and posting the transaction's bytes to HyperEVM, which ArbOS folds into the same
//! gas at the parent chain's data price. A plain HYPE transfer therefore takes a little over
//! 21,000 (a hard-coded 21,000 is refused as intrinsic gas too low).
//!
//! There is no priority auction: the tip is zero and ordering is first come, first served. A fee
//! cap of twice the base fee covers a block or two of rising fees; what is not used is not paid.

use df_hyperliquid_core::client::evm::BadAmount;

use crate::client::{Call, Fees, Read};
use crate::utils::{Address, Dec, hype};
use crate::{U256, nitro};

/// The fee cap over the current base fee.
const BASE_FEE_TIMES: u64 = 2;

/// The fees a transaction names: no tip, and a cap of twice `base`, saturating rather than
/// wrapping.
#[must_use]
pub fn fees(base: U256) -> Fees {
    Fees { max_priority_fee: U256::ZERO, max_fee: base.saturating_mul(U256::from_u64(BASE_FEE_TIMES)) }
}

/// A gas estimate split as the NodeInterface splits it ([`nitro::node::gas_estimate_components`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    /// All the gas the transaction takes, the parent chain's data included.
    pub gas: u64,
    /// Of [`Estimate::gas`], the part that pays for HyperEVM's data.
    pub gas_for_l1: u64,
    /// Elysium's base fee, in wei per gas.
    pub base_fee: U256,
    /// The parent chain's data price ArbOS charges against, in wei per unit.
    pub l1_base_fee: U256,
}

impl Estimate {
    /// The gas Elysium's execution takes.
    #[must_use]
    pub const fn gas_for_l2(&self) -> u64 {
        self.gas.saturating_sub(self.gas_for_l1)
    }

    /// What the transaction costs at the base fee it was estimated at, in wei.
    #[must_use]
    pub fn cost(&self) -> U256 {
        self.base_fee.saturating_mul(U256::from_u64(self.gas))
    }

    /// Of [`Estimate::cost`], what HyperEVM's data costs, in wei.
    #[must_use]
    pub fn cost_for_l1(&self) -> U256 {
        self.base_fee.saturating_mul(U256::from_u64(self.gas_for_l1))
    }

    /// [`Estimate::cost`] in HYPE, exactly.
    pub fn hype(&self) -> Result<Dec, BadAmount> {
        hype(self.cost())
    }
}

/// The estimate of `call` sent by `from`, as a read.
#[must_use]
pub fn components(from: Option<Address>, call: &Call) -> Read<Estimate> {
    nitro::node::gas_estimate_components(from, call)
}

/// What a transaction costs at most, in HYPE: every unit of its gas at its fee cap, and its value.
pub fn most_spent(gas: u64, fees: Fees, value: U256) -> Result<Dec, BadAmount> {
    let gas = fees.max_fee.checked_mul(U256::from_u64(gas)).ok_or(BadAmount::TooLarge)?;
    hype(gas.checked_add(value).ok_or(BadAmount::TooLarge)?)
}

#[cfg(test)]
#[path = "tests/gas.rs"]
mod tests;
