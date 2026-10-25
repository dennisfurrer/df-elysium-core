//! [`Nonces`]: one signer's next nonce, kept between sends so a program quoting several times a
//! second does not read it from the node before each. Elysium has no mempool: a transaction the
//! sequencer takes is in the next block, so the node's pending count is the chain's.

use std::sync::{Mutex, PoisonError};

/// The next nonce, once known. A send that is refused, or whose answer never came, forgets it, so
/// the next send reads the node again: a lost answer may still have landed, and the count says.
#[derive(Debug, Default)]
pub struct Nonces {
    next: Mutex<Option<u64>>,
}

impl Nonces {
    #[must_use]
    pub const fn new() -> Nonces {
        Nonces { next: Mutex::new(None) }
    }

    /// The kept next nonce, if one is kept.
    #[must_use]
    pub fn kept(&self) -> Option<u64> {
        *self.next.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `count` (the node's pending count) unless a later one is already kept, and answers
    /// the nonce to use.
    pub fn read(&self, count: u64) -> u64 {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let nonce = next.map_or(count, |kept| kept.max(count));
        *next = Some(nonce);
        nonce
    }

    /// A transaction at `nonce` was taken: the next is one past it.
    pub fn sent(&self, nonce: u64) {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let after = nonce.saturating_add(1);
        *next = Some(next.map_or(after, |kept| kept.max(after)));
    }

    /// Forgets the kept nonce: the next send reads the node.
    pub fn forget(&self) {
        *self.next.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

#[cfg(test)]
#[path = "tests/nonce.rs"]
mod tests;
