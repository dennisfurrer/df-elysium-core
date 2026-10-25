//! [`LogCursor`]: a log filter followed from a start block, a window at a time, `confirmations`
//! blocks behind the head so a reorganised block is never read. The window a poll asks for and
//! the block it moves on to are its only state, so a program can keep it across restarts.

use crate::client::rpc::{LogFilter, window};

/// Where a followed filter has read to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogCursor {
    filter: LogFilter,
    next: u64,
    span: u64,
    confirmations: u64,
}

impl LogCursor {
    /// `filter` read from block `from` on, windows of at most `span` blocks past their first
    /// ([`crate::client::Network::log_span`] for an unfiltered one), `confirmations` behind the
    /// head.
    #[must_use]
    pub fn new(filter: LogFilter, from: u64, span: u64, confirmations: u64) -> LogCursor {
        LogCursor { filter, next: from, span, confirmations }
    }

    /// The first block not yet read.
    #[must_use]
    pub const fn next(&self) -> u64 {
        self.next
    }

    /// The window to read now that the head is `head`; `None` while nothing confirmed is unread.
    #[must_use]
    pub fn window(&self, head: u64) -> Option<LogFilter> {
        let confirmed = head.checked_sub(self.confirmations)?;
        if self.next > confirmed {
            return None;
        }
        let last = self.next.saturating_add(self.span).min(confirmed);
        Some(window(&self.filter, self.next, last))
    }

    /// The window `window` answered was read: the next starts after it.
    pub fn read(&mut self, window: &LogFilter) {
        if let Some(crate::client::BlockTag::Number(last)) = window.to_block {
            self.next = self.next.max(last.saturating_add(1));
        }
    }
}

#[cfg(test)]
#[path = "tests/cursor.rs"]
mod tests;
