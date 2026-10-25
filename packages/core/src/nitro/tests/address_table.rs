//! ArbAddressTable's typed reads against Elysium testnet's answers (`elysium_testnet.json`): the
//! table is empty, so a lookup reverts.

use crate::nitro::support::{recorded, sent, word};
use crate::test_support::{bytes, text};
use crate::utils::hex;

use super::*;

#[test]
fn an_empty_table_has_nothing_registered_and_its_calls_are_its_methods() {
    assert_eq!(sent(&size(), "ArbAddressTable", "size", 0), U256::ZERO);
    let asked = word(&recorded("ArbAddressTable", "addressExists", 0), 0).to_address().unwrap();
    assert!(!sent(&address_exists(asked), "ArbAddressTable", "addressExists", 0));
    let refused = recorded("ArbAddressTable", "lookup", 0);
    assert_eq!(hex::encode_prefixed(&lookup(asked).call.data), text(&refused["params"][0]["data"]));
    assert!(refused["answer"].as_str().unwrap().contains("execution reverted"), "nothing to look up");
    let register = register(asked);
    assert_eq!(register.to, Some(ARB_ADDRESS_TABLE));
    assert_eq!(&register.data[..4], &REGISTER.selector());
    assert_eq!(&register.data[4..], &bytes(&refused["params"][0]["data"])[4..], "the address as a word");
    assert_eq!(&lookup_index(7).call.data[4..], &U256::from(7u8).to_be_bytes());
    let decompress = decompress(&[0x01, 0x02], 1);
    assert_eq!(decompress.call.data.len(), 4 + 32 * 4, "the offset head, the offset, the length, the padded bytes");
}
