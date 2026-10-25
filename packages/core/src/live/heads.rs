//! [`Heads`]: which blocks are new since the last poll of `eth_blockNumber`.

use std::ops::RangeInclusive;

/// The last block seen; each new head answers the blocks after it, once each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Heads {
    last: Option<u64>,
}

impl Heads {
    /// Nothing seen yet: the first head is new on its own.
    #[must_use]
    pub const fn new() -> Heads {
        Heads { last: None }
    }

    /// Everything after `last` is new.
    #[must_use]
    pub const fn after(last: u64) -> Heads {
        Heads { last: Some(last) }
    }

    #[must_use]
    pub const fn last(&self) -> Option<u64> {
        self.last
    }

    /// The blocks new at `head`, oldest first: none when the head has not moved (or a lagging node
    /// answered an older one), the head alone at first.
    pub fn seen(&mut self, head: u64) -> Option<RangeInclusive<u64>> {
        let first = match self.last {
            None => head,
            Some(last) if head > last => last + 1,
            Some(_) => return None,
        };
        self.last = Some(head);
        Some(first..=head)
    }
}

#[cfg(test)]
#[path = "tests/heads.rs"]
mod tests;
