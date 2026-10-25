//! A HYPE send, exact or refused, and Permit2 where Elysium testnet holds its code
//! (`elysium_testnet.json`).

use crate::test_support::{answer, result, text};

use super::*;

#[test]
fn a_hype_send_carries_its_amount_in_wei_or_is_refused() {
    let to = Address::from_bytes([0x11; 20]);
    let call = hype(to, Dec::parse("0.02").unwrap()).unwrap();
    assert_eq!((call.to, call.value, call.data.len()), (Some(to), U256::from_u64(20_000_000_000_000_000), 0));
    assert_eq!(hype(to, Dec::ZERO), Err(BadAmount::NotPositive(Dec::ZERO)));
    assert!(hype(to, Dec::parse("-1").unwrap()).is_err());
    assert_eq!(hype_wei(to, U256::ONE).value, U256::ONE);
}

#[test]
fn permit2_is_predeployed_at_its_canonical_address() {
    let code = result(&answer("eth_getCode", 1));
    assert!(text(&code).len() > 2, "Elysium holds its code");
    assert_eq!(PERMIT2.to_hex(), "0x000000000022d473030f116ddee9f6b43ac78ba3");
}
