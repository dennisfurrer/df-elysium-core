//! New blocks and logs as they land, polled: Elysium's public node has no websocket, and a block
//! every 100 to 200 ms makes a poll every few hundred milliseconds as fresh as a subscription.
//!
//! - [`Heads`]: the blocks new since the last poll.
//! - [`LogCursor`]: a filter followed block by block, `confirmations` behind the head, never
//!   missing or repeating a window.
//!
//! Both are sans-IO: they say what to ask and take the answers, so a program polls on its own
//! timer (tokio, `setTimeout`, a Worker's alarm). [`crate::Client::logs`] sends a window.

pub mod cursor;
pub mod heads;

pub use cursor::LogCursor;
pub use heads::Heads;
