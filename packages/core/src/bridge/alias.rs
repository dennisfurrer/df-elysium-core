//! Address aliasing. A message from the parent chain arrives from its sender's alias, the sender
//! plus `0x1111000000000000000000000000000000001111` modulo 2^160, so a contract on HyperEVM can
//! never pass for the account at the same address on Elysium. The inbox aliases every sender: a
//! deposit's `from` is the alias, its `to` the depositor itself when that is an account.

use crate::utils::Address;

/// What every sender from the parent chain is shifted by.
pub const ALIAS_OFFSET: Address = match Address::parse_const("0x1111000000000000000000000000000000001111") {
    Some(address) => address,
    None => Address::ZERO,
};

/// The address a parent-chain sender arrives as.
#[must_use]
pub fn apply_alias(l1: Address) -> Address {
    add(l1.bytes(), ALIAS_OFFSET.bytes(), false)
}

/// The parent-chain sender an alias stands for.
#[must_use]
pub fn undo_alias(l2: Address) -> Address {
    add(l2.bytes(), ALIAS_OFFSET.bytes(), true)
}

/// `a + b` or `a - b` over 160 bits, the overflow dropped.
fn add(a: &[u8; 20], b: &[u8; 20], subtract: bool) -> Address {
    let mut out = [0u8; 20];
    let mut carry = 0i16;
    for at in (0..20).rev() {
        let step = match subtract {
            true => i16::from(a[at]) - i16::from(b[at]) + carry,
            false => i16::from(a[at]) + i16::from(b[at]) + carry,
        };
        out[at] = step.rem_euclid(256) as u8;
        carry = step.div_euclid(256);
    }
    Address::from_bytes(out)
}

#[cfg(test)]
#[path = "tests/alias.rs"]
mod tests;
