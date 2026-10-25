//! Stylus's parameters against Elysium testnet's answers (`elysium_testnet.json`, cast's decoding
//! beside them), and activation and the cache's calls against viem's `encodeFunctionData`
//! (`arbitrum_sdk.json`). No program is activated on Elysium testnet, so a program's reads revert.

use crate::nitro::support::{recorded, sdk_arg, sdk_data, sdk_one, sent, word};
use crate::test_support::text;
use crate::utils::hex;

use super::*;

#[test]
fn the_chain_s_stylus_parameters_read_as_answered() {
    assert_eq!(sent(&stylus_version(), "ArbWasm", "stylusVersion", 0), 3);
    assert_eq!(sent(&ink_price(), "ArbWasm", "inkPrice", 0), 10_000);
    assert_eq!(sent(&max_stack_depth(), "ArbWasm", "maxStackDepth", 0), 22_000);
    assert_eq!(sent(&free_pages(), "ArbWasm", "freePages", 0), 2);
    assert_eq!(sent(&page_gas(), "ArbWasm", "pageGas", 0), 1_000);
    assert_eq!(sent(&page_limit(), "ArbWasm", "pageLimit", 0), 128);
    assert_eq!(sent(&expiry_days(), "ArbWasm", "expiryDays", 0), 365);
    assert_eq!(sent(&keepalive_days(), "ArbWasm", "keepaliveDays", 0), 31);
    assert_eq!(sent(&block_cache_size(), "ArbWasm", "blockCacheSize", 0), 32);
    assert!(sent(&cache_managers(), "ArbWasmCache", "allCacheManagers", 0).is_empty());
    let asked = word(&recorded("ArbWasmCache", "isCacheManager", 0), 0).to_address().unwrap();
    assert!(!sent(&is_cache_manager(asked), "ArbWasmCache", "isCacheManager", 0));
    assert!(!sent(&codehash_is_cached(B256::ZERO), "ArbWasmCache", "codehashIsCached", 0));
}

/// `ProgramNotActivated()`, as the node refused a program that is not one.
#[test]
fn a_program_that_is_not_activated_is_refused_by_the_read_that_asks() {
    for name in ["programVersion", "codehashVersion"] {
        let refused = recorded("ArbWasm", name, 0);
        let read = match name {
            "programVersion" => program_version(word(&refused, 0).to_address().unwrap()).call,
            _ => codehash_version(word(&refused, 0)).call,
        };
        assert_eq!(hex::encode_prefixed(&read.data), text(&refused["params"][0]["data"]), "{name}");
        assert!(text(&refused["answer"]).contains("ProgramNotActivated()"), "{name}");
    }
    assert_eq!(crate::client::Method::new("ProgramNotActivated()", "").selector(), [0x6f, 0x80, 0x9c, 0x4e]);
}

#[test]
fn activation_and_the_cache_are_viem_s_calldata() {
    let activate = sdk_one("ArbWasm", "activateProgram");
    let program: Address = sdk_arg(&activate, 0).parse().unwrap();
    let fee = U256::from(1_000_000_000_000u64);
    let call = activate_program(program, fee);
    assert_eq!((call.to, call.value), (Some(ARB_WASM), fee), "the data fee as value");
    assert_eq!(hex::encode_prefixed(&call.data), sdk_data(&activate));
    let quote = activation_quote(program, fee);
    assert_eq!((&quote.call.data, quote.call.value), (&call.data, Some(fee)), "the same call, simulated");
    let mut answer = U256::from(2u8).to_be_bytes().to_vec();
    answer.extend_from_slice(&U256::from(7u8).to_be_bytes());
    assert_eq!(quote.decode(&answer), Ok(Activation { version: 2, data_fee: U256::from(7u8) }));
    for (name, data) in [
        ("programVersion", program_version(program).call.data),
        ("codehashVersion", codehash_version(B256([0xcd; 32])).call.data),
        ("stylusVersion", stylus_version().call.data),
    ] {
        assert_eq!(hex::encode_prefixed(&data), sdk_data(&sdk_one("ArbWasm", name)), "{name}");
    }
    let cache = sdk_one("ArbWasmCache", "cacheProgram");
    assert_eq!(hex::encode_prefixed(&cache_program(program).data), sdk_data(&cache));
    assert_eq!(cache_program(program).to, Some(ARB_WASM_CACHE));
    let codehash: B256 = sdk_arg(&sdk_one("ArbWasmCache", "evictCodehash"), 0).parse().unwrap();
    assert_eq!(
        hex::encode_prefixed(&evict_codehash(codehash).data),
        sdk_data(&sdk_one("ArbWasmCache", "evictCodehash"))
    );
    assert_eq!(
        hex::encode_prefixed(&codehash_is_cached(codehash).call.data),
        sdk_data(&sdk_one("ArbWasmCache", "codehashIsCached"))
    );
    let keepalive = codehash_keepalive(codehash, fee);
    assert_eq!(
        (keepalive.to, keepalive.value, &keepalive.data[..4]),
        (Some(ARB_WASM), fee, &CODEHASH_KEEPALIVE.selector()[..])
    );
}

#[test]
fn an_activation_logs_its_program_under_its_topic() {
    assert_eq!(
        PROGRAM_ACTIVATED.topic().0,
        crate::utils::keccak256(b"ProgramActivated(bytes32,bytes32,address,uint256,uint16)")
    );
    assert_eq!(PROGRAM_LIFETIME_EXTENDED.name(), "ProgramLifetimeExtended");
}
