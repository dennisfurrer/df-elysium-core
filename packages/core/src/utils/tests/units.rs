//! Expected values are node's `BigInt` over the same digits, and a live balance read from
//! Elysium testnet.

use super::*;

#[test]
fn wei_reads_as_hype_and_hype_becomes_wei_exactly() {
    let dec = |text: &str| Dec::parse(text).unwrap();
    // The operator's balance on Elysium testnet, 2026-10-25.
    let held: U256 = "1010313171011928712".parse().unwrap();
    assert_eq!(hype(held), Ok(dec("1.010313171011928712")));
    assert_eq!(wei(dec("1.010313171011928712")), Ok(held));
    assert_eq!(wei(dec("0.02")), Ok(U256::from(20_000_000_000_000_000u64)));
    assert_eq!(hype(U256::ONE), Ok(dec("0.000000000000000001")));
    assert!(wei(dec("-1")).is_err(), "a negative amount");
    assert_eq!(HYPE_DECIMALS, 18);
}
