//! ArbInfo's typed reads against Elysium testnet's answers (`elysium_testnet.json`, cast's
//! decoding beside them).

use crate::nitro::support::{decoded, number, recorded, sent, word};
use crate::test_support::text;
use crate::transfer::multicall::MULTICALL3;
use crate::utils::hex;

use super::*;

#[test]
fn a_balance_and_code_read_as_a_precompile_reads_them() {
    let owner = word(&recorded("ArbInfo", "getBalance", 0), 0).to_address().unwrap();
    assert_eq!(sent(&balance(owner), "ArbInfo", "getBalance", 0), number(&decoded("ArbInfo", "getBalance", 0)[0]));
    let at = word(&recorded("ArbInfo", "getCode", 0), 0).to_address().unwrap();
    assert_eq!(at, MULTICALL3);
    let code = sent(&code(at), "ArbInfo", "getCode", 0);
    assert_eq!(hex::encode_prefixed(&code), text(&decoded("ArbInfo", "getCode", 0)[0]).to_lowercase());
    assert!(code.starts_with(&[0x60, 0x80, 0x60, 0x40]), "Multicall3's runtime code");
}
