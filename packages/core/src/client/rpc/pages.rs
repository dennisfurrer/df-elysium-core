//! A log scan in pages. Elysium's public node answers an `eth_getLogs` that names no address and
//! no topic over at most [`Network::log_span`](crate::client::Network::log_span) blocks; one that
//! names either may span any range, and is sent whole.

use crate::client::rpc::{BlockTag, LogFilter};

/// The windows `[from, to]` is scanned in: inclusive, in order, each spanning at most `span`
/// blocks past its first (`toBlock - fromBlock <= span`). Empty when `from > to`.
#[must_use]
pub fn pages(from: u64, to: u64, span: u64) -> Pages {
    Pages { next: Some(from).filter(|from| *from <= to), to, span }
}

/// The windows of a scan, from [`pages`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    next: Option<u64>,
    to: u64,
    span: u64,
}

impl Iterator for Pages {
    type Item = (u64, u64);

    fn next(&mut self) -> Option<(u64, u64)> {
        let from = self.next?;
        let last = from.saturating_add(self.span).min(self.to);
        self.next = last.checked_add(1).filter(|next| *next <= self.to);
        Some((from, last))
    }
}

/// Whether the node takes the filter over any range: it names an address or a topic.
#[must_use]
pub fn is_narrow(filter: &LogFilter) -> bool {
    !filter.address.is_empty() || filter.topics.iter().any(Option::is_some)
}

/// The filter for one window: the same addresses and topics, between these blocks.
#[must_use]
pub fn window(filter: &LogFilter, from: u64, to: u64) -> LogFilter {
    LogFilter {
        from_block: Some(BlockTag::Number(from)),
        to_block: Some(BlockTag::Number(to)),
        block_hash: None,
        ..filter.clone()
    }
}

#[cfg(test)]
#[path = "tests/pages.rs"]
mod tests;
