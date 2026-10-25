//! Where a creation lands: CREATE2 against viem's `getContractAddress` (`arbitrum_sdk.json`), and
//! the deterministic deployer's calldata as it takes it.

use crate::test_support::{arbitrum_sdk, bytes, text};

use super::*;

#[test]
fn create2_lands_where_viem_says() {
    let file = arbitrum_sdk();
    let cases = file["create2"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let deployer: Address = text(&case["deployer"]).parse().unwrap();
        let salt: B256 = text(&case["salt"]).parse().unwrap();
        let hash: B256 = text(&case["initCodeHash"]).parse().unwrap();
        if let Some(code) = case.get("initCode") {
            assert_eq!(init_code_hash(&bytes(code)), hash);
        }
        assert_eq!(create2_address(deployer, salt, hash).to_hex(), text(&case["address"]).to_lowercase());
    }
}

#[test]
fn a_creation_carries_its_code_and_arguments_and_create2_goes_through_the_deployer() {
    let code = [0x60, 0x0a, 0x60, 0x0c];
    let args = U256::from_u64(5).to_be_bytes();
    let call = create(&code, &args);
    assert_eq!((call.to, call.value), (None, U256::ZERO), "a creation names no recipient");
    assert_eq!(call.data, [&code[..], &args[..]].concat());
    let salt = B256([0x01; 32]);
    let call = create2(salt, &code);
    assert_eq!(call.to, Some(CREATE2_DEPLOYER));
    assert_eq!(call.data, [&salt.0[..], &code[..]].concat(), "the deployer takes the salt, then the code");
    assert_eq!(CREATE2_DEPLOYER.to_hex(), "0x4e59b44847b379578588920ca78fbf26c0b4956c");
}
