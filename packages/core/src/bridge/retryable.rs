//! Retryable tickets: a call on Elysium sent from HyperEVM, paid for there.
//! `Inbox.createRetryableTicket` carries the submission cost, the call's value and its gas at the
//! fee cap; Elysium makes a ticket (type `0x69`, its hash the ticket id) and redeems it on its
//! own (type `0x68`). A redeem that runs out of gas leaves the ticket for anyone to redeem
//! ([`crate::nitro::retryable`]) within its lifetime, a week.
//!
//! The submission cost is `(1400 + 6 * data length)` times the parent chain's base fee; whatever
//! is not used is refunded on Elysium, the excess to one address and the call's value, if it never
//! runs, to another.

use df_hyperliquid_core::client::evm::abi::Token;
use df_hyperliquid_core::client::evm::rlp;

use crate::bridge::message::{Message, MessageKind};
use crate::client::read::one;
use crate::client::{Arg, Call, Method, Network, Read};
use crate::utils::{Address, keccak256};
use crate::{B256, U256};

pub const CREATE_RETRYABLE_TICKET: Method =
    Method::new("createRetryableTicket(address,uint256,uint256,address,address,uint256,uint256,bytes)", "uint256");
/// The same without the inbox's checks: refund addresses are not aliased and the value is not
/// checked to cover the ticket. Easy to lose funds with.
pub const UNSAFE_CREATE_RETRYABLE_TICKET: Method = Method::new(
    "unsafeCreateRetryableTicket(address,uint256,uint256,address,address,uint256,uint256,bytes)",
    "uint256",
);
pub const CALCULATE_RETRYABLE_SUBMISSION_FEE: Method =
    Method::new("calculateRetryableSubmissionFee(uint256,uint256)", "uint256");

/// The type byte of a ticket on Elysium.
pub const SUBMIT_RETRYABLE_TX: u8 = 0x69;

/// What a ticket's submission costs per byte of data, and on top, in parent-chain gas.
const SUBMISSION_PER_BYTE: u64 = 6;
const SUBMISSION_BASE: u64 = 1_400;

/// `(1400 + 6 * data_length) * base_fee`: the least a ticket's `max_submission_cost` may be, at
/// the parent chain's base fee.
#[must_use]
pub fn submission_fee(data_length: usize, base_fee: U256) -> U256 {
    let units = (data_length as u64).saturating_mul(SUBMISSION_PER_BYTE).saturating_add(SUBMISSION_BASE);
    base_fee.saturating_mul(U256::from_u64(units))
}

/// The inbox's own figure, read on the parent chain. A `base_fee` of zero stands for the block's,
/// which is zero inside an `eth_call`: name it.
#[must_use]
pub fn submission_fee_read(network: Network, data_length: usize, base_fee: U256) -> Read<U256> {
    let args = [Arg::u64(data_length as u64), Arg::Uint(base_fee)];
    Read::new(network.contracts().inbox, &CALCULATE_RETRYABLE_SUBMISSION_FEE, &args, |answer| {
        one(&CALCULATE_RETRYABLE_SUBMISSION_FEE, answer, Token::as_u256)
    })
}

/// A ticket as the inbox takes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Retryable {
    /// The call's target on Elysium; zero for none.
    pub to: Address,
    /// HYPE the call carries, in wei.
    pub l2_call_value: U256,
    /// The most the submission may cost: at least [`submission_fee`].
    pub max_submission_cost: U256,
    /// Who is refunded the unused gas and submission cost on Elysium.
    pub excess_fee_refund_address: Address,
    /// Who may cancel the ticket and is paid its call value if it never runs.
    pub call_value_refund_address: Address,
    /// The redeem's gas on Elysium.
    pub gas_limit: U256,
    /// The redeem's fee cap, in wei per gas on Elysium.
    pub max_fee_per_gas: U256,
    pub data: Vec<u8>,
}

impl Retryable {
    /// What the parent transaction must carry: the submission cost, the call's value, and all the
    /// gas at the fee cap. `None` past 256 bits.
    #[must_use]
    pub fn deposit(&self) -> Option<U256> {
        self.gas_limit
            .checked_mul(self.max_fee_per_gas)?
            .checked_add(self.max_submission_cost)?
            .checked_add(self.l2_call_value)
    }

    fn args(&self) -> [Arg<'_>; 8] {
        [
            Arg::Address(self.to),
            Arg::Uint(self.l2_call_value),
            Arg::Uint(self.max_submission_cost),
            Arg::Address(self.excess_fee_refund_address),
            Arg::Address(self.call_value_refund_address),
            Arg::Uint(self.gas_limit),
            Arg::Uint(self.max_fee_per_gas),
            Arg::Bytes(&self.data),
        ]
    }

    /// `createRetryableTicket` on the network's inbox, carrying [`Retryable::deposit`].
    #[must_use]
    pub fn call(&self, network: Network) -> Option<Call> {
        Some(Call {
            to: Some(network.contracts().inbox),
            value: self.deposit()?,
            data: CREATE_RETRYABLE_TICKET.calldata(&self.args()),
        })
    }
}

/// A ticket as the parent chain delivered it: what its id is computed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// The inbox's message number: the ticket's `requestId`.
    pub message_number: U256,
    /// The sender's alias.
    pub from: Address,
    /// The parent chain's base fee when it was delivered.
    pub l1_base_fee: U256,
    /// What the parent transaction carried.
    pub deposit: U256,
    pub retryable: Retryable,
}

impl Ticket {
    /// The ticket a delivered message makes; `None` for any other kind. The inbox packs it as
    /// nine words (the target, call value, deposit, submission cost, the two refund addresses,
    /// gas limit, fee cap and the data's length), then the data.
    #[must_use]
    pub fn from_message(message: &Message) -> Option<Ticket> {
        if message.kind != MessageKind::SubmitRetryable {
            return None;
        }
        let data = message.data.as_deref()?;
        let word = |at: usize| data.get(at * 32..at * 32 + 32).and_then(U256::from_be_slice);
        let address = |at: usize| word(at).map(B256::from).and_then(|word| word.to_address());
        let length = usize::try_from(word(8)?.to_u64()?).ok()?;
        let call = data.get(9 * 32..)?;
        if call.len() != length {
            return None;
        }
        Some(Ticket {
            message_number: message.number,
            from: message.sender,
            l1_base_fee: message.base_fee_l1,
            deposit: word(2)?,
            retryable: Retryable {
                to: address(0)?,
                l2_call_value: word(1)?,
                max_submission_cost: word(3)?,
                excess_fee_refund_address: address(4)?,
                call_value_refund_address: address(5)?,
                gas_limit: word(6)?,
                max_fee_per_gas: word(7)?,
                data: call.to_vec(),
            },
        })
    }

    /// The ticket's id on `chain_id`, which is its submission's transaction hash:
    /// `keccak256(0x69 || rlp([chainId, requestId, from, l1BaseFee, deposit, maxFeePerGas,
    /// gasLimit, to or empty, callValue, callValueRefund, maxSubmissionCost, excessFeeRefund,
    /// data]))`.
    #[must_use]
    pub fn id(&self, chain_id: u64) -> B256 {
        let ticket = &self.retryable;
        let to = match ticket.to.is_zero() {
            true => rlp::bytes(&[]),
            false => rlp::bytes(ticket.to.bytes()),
        };
        let fields = [
            rlp::uint(u128::from(chain_id)),
            rlp::bytes(&self.message_number.to_be_bytes()),
            rlp::bytes(self.from.bytes()),
            rlp::u256(self.l1_base_fee),
            rlp::u256(self.deposit),
            rlp::u256(ticket.max_fee_per_gas),
            rlp::u256(ticket.gas_limit),
            to,
            rlp::u256(ticket.l2_call_value),
            rlp::bytes(ticket.call_value_refund_address.bytes()),
            rlp::u256(ticket.max_submission_cost),
            rlp::bytes(ticket.excess_fee_refund_address.bytes()),
            rlp::bytes(&ticket.data),
        ];
        let mut typed = vec![SUBMIT_RETRYABLE_TX];
        typed.extend(rlp::list(&fields));
        B256(keccak256(&typed))
    }
}

#[cfg(test)]
#[path = "tests/retryable.rs"]
mod tests;
