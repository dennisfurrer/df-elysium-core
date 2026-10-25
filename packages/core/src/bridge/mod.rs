//! HYPE and messages between HyperEVM and Elysium, both ways, through the rollup's own contracts on
//! the parent chain ([`Contracts`]).
//!
//! - [`mod@deposit`]: HYPE in, through the inbox, and the deposit's hash on Elysium before it lands.
//! - [`retryable`]: a call on Elysium paid for on HyperEVM, and its ticket id.
//! - [`mod@withdraw`]: HYPE or a call out, through ArbSys, and the sends a receipt logged.
//! - [`mod@claim`]: a withdrawal executed on the parent chain once an assertion past it is confirmed.
//! - [`message`]: what the parent chain logged for each message in.
//! - [`alias`]: how a parent-chain sender is named on Elysium.
//!
//! Parent-chain reads go through df-hyperliquid-core's [`Rpc`](crate::client::Rpc) to HyperEVM
//! ([`Network::hyperliquid`](crate::client::Network::hyperliquid)), and calls there are sent on
//! [`Network::parent_chain_id`](crate::client::Network::parent_chain_id). HyperEVM answers
//! `eth_getLogs` over at most 50 blocks.

pub mod alias;
pub mod claim;
pub mod contracts;
pub mod deposit;
pub mod message;
pub mod retryable;
pub mod withdraw;

pub use alias::{ALIAS_OFFSET, apply_alias, undo_alias};
pub use claim::{Claim, ClaimError, Confirmed, claim};
pub use contracts::Contracts;
pub use deposit::{Deposit, deposit, deposit_wei};
pub use message::{Message, MessageKind};
pub use retryable::{Retryable, Ticket, submission_fee};
pub use withdraw::{Withdrawal, withdraw};
