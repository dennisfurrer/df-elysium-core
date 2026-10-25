//! Transactions on chain 99801 against viem's `serializeTransaction` and `signTransaction` with
//! Hardhat's account #0, each built from the crate's own call builders (a HYPE transfer, a
//! creation, `ArbSys.withdrawEth`) and its fee rule, and CREATE2 addresses against viem's
//! `getContractAddress`. The legacy EIP-155 vector is not rebuilt: the crate sends EIP-1559 only.

use df_elysium_core::client::{Call, tx_hash};
use df_elysium_core::utils::hex;
use df_elysium_core::{U256, deploy, gas, nitro, transfer};
use serde_json::Value;

use crate::support::{address, arbitrum_sdk, bytes, key, number, text, word};

fn small(value: &Value) -> u64 {
    number(value).to_u64().unwrap()
}

/// The call a vector's fields make, from the crate's builder for it.
fn call(name: &str, fields: &Value) -> Call {
    let value = number(&fields["value"]);
    match name {
        "HYPE transfer" => transfer::hype_wei(address(text(&fields["to"])), value),
        "contract creation" => deploy::create(&bytes(&fields["data"]), &[]),
        "ArbSys.withdrawEth" => {
            let data = bytes(&fields["data"]);
            let destination = df_elysium_core::utils::Address::from_bytes(data[16..36].try_into().unwrap());
            nitro::sys::withdraw_eth(destination, value)
        }
        other => panic!("no builder for {other}"),
    }
}

#[test]
fn every_eip1559_vector_is_rebuilt_from_the_crate_s_builders_byte_for_byte() {
    let file = arbitrum_sdk();
    let mut rebuilt = 0;
    for case in file["transactions"].as_array().unwrap() {
        let name = text(&case["name"]);
        let fields = &case["fields"];
        if fields["type"] == "legacy" {
            continue;
        }
        let built = call(name, fields);
        assert_eq!(built.data, bytes(&fields["data"]), "{name}: calldata");
        assert_eq!(built.to.map(|to| to.to_hex()), fields["to"].as_str().map(str::to_owned), "{name}: to");
        let max_fee = number(&fields["maxFeePerGas"]);
        let base = max_fee.div_rem_u64(2).unwrap().0;
        let fees = gas::fees(base);
        assert_eq!(
            (fees.max_fee, fees.max_priority_fee),
            (max_fee, number(&fields["maxPriorityFeePerGas"])),
            "{name}: fees"
        );
        let tx = built.transaction(small(&fields["chainId"]), small(&fields["nonce"]), small(&fields["gas"]), fees);
        assert_eq!(hex::encode_prefixed(&tx.unsigned()), text(&case["unsigned"]), "{name}: unsigned");
        assert_eq!(tx.signing_hash().to_hex(), text(&case["signingHash"]), "{name}: signing hash");
        let signed = tx.sign(&key()).unwrap();
        assert_eq!(hex::encode_prefixed(&signed), text(&case["signed"]), "{name}: signed");
        assert_eq!(tx_hash(&signed).to_hex(), text(&case["hash"]), "{name}: hash");
        rebuilt += 1;
    }
    assert_eq!(rebuilt, 3, "the transfer, the creation and the withdrawal");
}

#[test]
fn create2_addresses_match_viem() {
    let file = arbitrum_sdk();
    let cases = file["create2"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let init_code = bytes(&case["initCode"]);
        let hash = deploy::init_code_hash(&init_code);
        assert_eq!(hash, word(text(&case["initCodeHash"])));
        let deployer = address(text(&case["deployer"]));
        let at = deploy::create2_address(deployer, word(text(&case["salt"])), hash);
        assert_eq!(at, address(text(&case["address"])), "{case}");
        if deployer == deploy::CREATE2_DEPLOYER {
            let sent = deploy::create2(word(text(&case["salt"])), &init_code);
            assert_eq!((sent.to, sent.value), (Some(deploy::CREATE2_DEPLOYER), U256::ZERO));
            assert_eq!(
                sent.data,
                [word(text(&case["salt"])).0.as_slice(), &init_code].concat(),
                "salt, then init code"
            );
        }
    }
}
