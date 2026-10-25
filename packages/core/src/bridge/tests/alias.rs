//! Aliases against @arbitrum/sdk 4.1.0's `Address.applyAlias` and its undo (`arbitrum_sdk.json`),
//! the overflow either way included, and a live deposit whose `from` is its depositor's alias.

use crate::test_support::{self, address, result, text};

use super::*;

#[test]
fn every_alias_is_the_sdk_s_and_undoes_to_its_sender() {
    let file = test_support::arbitrum_sdk();
    let cases = file["aliases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let (l1, l2) = (address(text(&case["l1"])), address(text(&case["l2"])));
        assert_eq!(apply_alias(l1), l2, "{l1}");
        assert_eq!(undo_alias(l2), l1, "{l2}");
    }
    assert_eq!(apply_alias(Address::ZERO), ALIAS_OFFSET);
    let top = address("0xffffffffffffffffffffffffffffffffffffffff");
    assert_eq!(apply_alias(top), address("0x1111000000000000000000000000000000001110"), "wraps past 2^160");
    assert_eq!(undo_alias(Address::ZERO), address("0xeeeeffffffffffffffffffffffffffffffffeeef"), "and back under zero");
}

/// HYPE deposited by an account: it arrives at the account, sent from the account's alias.
#[test]
fn a_live_deposit_comes_from_its_depositor_s_alias() {
    let deposit = result(&test_support::step("deposit", "transaction"));
    let (from, to) = (address(text(&deposit["from"])), address(text(&deposit["to"])));
    assert_eq!(apply_alias(to), from);
    assert_eq!(undo_alias(from), to);
}
