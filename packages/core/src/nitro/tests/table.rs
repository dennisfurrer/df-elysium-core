//! The table against the Nitro interfaces, and every precompile read Elysium testnet answered
//! (`elysium_testnet.json`): each is a method of its precompile, called by the selector the node
//! took, declared to answer what the interface declares, and read back as cast 1.8.3 read it.

use df_hyperliquid_core::client::evm::abi::{self, Function, Kind, Token};
use serde_json::Value;

use crate::client::Method;
use crate::client::records::Block;
use crate::nitro::{address_table, aggregator, gas_info, info, node, owner, retryable, statistics, sys, wasm};
use crate::test_support::{self, text};
use crate::utils::hex;

use super::*;

#[test]
fn each_precompile_sits_where_the_nitro_interfaces_put_it() {
    let at: Vec<(String, &str)> = ALL.iter().map(|one| (one.address.to_hex(), one.name)).collect();
    let expected = [
        ("0x0000000000000000000000000000000000000064", "ArbSys"),
        ("0x0000000000000000000000000000000000000065", "ArbInfo"),
        ("0x0000000000000000000000000000000000000066", "ArbAddressTable"),
        ("0x000000000000000000000000000000000000006b", "ArbOwnerPublic"),
        ("0x000000000000000000000000000000000000006c", "ArbGasInfo"),
        ("0x000000000000000000000000000000000000006d", "ArbAggregator"),
        ("0x000000000000000000000000000000000000006e", "ArbRetryableTx"),
        ("0x000000000000000000000000000000000000006f", "ArbStatistics"),
        ("0x0000000000000000000000000000000000000070", "ArbOwner"),
        ("0x0000000000000000000000000000000000000071", "ArbWasm"),
        ("0x0000000000000000000000000000000000000072", "ArbWasmCache"),
        ("0x00000000000000000000000000000000000000c8", "NodeInterface"),
    ];
    assert_eq!(at, expected.map(|(address, name)| (address.to_owned(), name)));
    for one in &ALL {
        assert_eq!(precompile(one.address), Some(one));
    }
    assert_eq!(precompile(Address::ZERO), None);
    assert_eq!(precompile(ARBOS), None, "ArbOS's sender is not a precompile");
    assert_eq!(ARBOS.to_hex(), "0x00000000000000000000000000000000000a4b05");
}

/// The block's mix hash carries ArbOS's version; ArbSys answers it plus 55.
#[test]
fn arbos_is_what_arbsys_reports_less_fifty_five_as_a_block_carries_it() {
    let reported = support_number("ArbSys", "arbOSVersion");
    assert_eq!(reported, 116);
    assert_eq!((ARBOS_VERSION_OFFSET, sys::arbos(reported)), (55, 61));
    let block: Block =
        serde_json::from_value(test_support::result(&test_support::answer("eth_getBlockByNumber", 0))).unwrap();
    assert_eq!(block.arbos_version(), sys::arbos(reported), "the recorded block's mix hash");
    assert_eq!(sys::arbos(3), 0, "never below zero");
}

fn support_number(precompile: &str, name: &str) -> u64 {
    crate::nitro::support::number(&crate::nitro::support::decoded(precompile, name, 0)[0]).to_u64().unwrap()
}

/// Every method this crate lists for a precompile, by the name the file gives it.
fn methods(precompile: &str) -> &'static [Method] {
    match precompile {
        "ArbSys" => sys::METHODS,
        "ArbInfo" => info::METHODS,
        "ArbAddressTable" => address_table::METHODS,
        "ArbOwnerPublic" => owner::METHODS,
        "ArbGasInfo" => gas_info::METHODS,
        "ArbAggregator" => aggregator::METHODS,
        "ArbRetryableTx" => retryable::METHODS,
        "ArbStatistics" => statistics::METHODS,
        "ArbWasm" | "ArbWasmCache" => wasm::METHODS,
        "NodeInterface" => node::METHODS,
        other => panic!("no precompile {other}"),
    }
}

/// cast's decoding, for comparison: every number as decimal text, every hex string lowercase.
fn normal(value: &Value) -> Value {
    match value {
        Value::Number(number) => Value::String(number.to_string()),
        Value::String(text) => Value::String(text.to_lowercase()),
        Value::Array(items) => Value::Array(items.iter().map(normal).collect()),
        other => other.clone(),
    }
}

/// A token as cast writes its value, normalised the same way.
fn shown(token: &Token, kind: &Kind) -> Value {
    match (token, kind) {
        (Token::Array(items), Kind::Array(inner) | Kind::Sized(inner, _)) => {
            Value::Array(items.iter().map(|item| shown(item, inner)).collect())
        }
        (Token::Tuple(items), Kind::Tuple(params)) => {
            Value::Array(items.iter().zip(params).map(|(item, param)| shown(item, &param.kind)).collect())
        }
        (Token::Bool(value), _) => Value::Bool(*value),
        (Token::Bytes(bytes) | Token::Fixed(bytes), _) => Value::String(hex::encode_prefixed(bytes)),
        (token, kind) => Value::String(token.show(kind).to_lowercase()),
    }
}

/// Two of the recorded reads are of methods this crate does not list: one whose struct the
/// interfaces name by a type this reads no further, and one this ArbOS reverts.
const UNLISTED: [&str; 2] = ["isCalldataPriceIncrease7976And7981Enabled", "getMultiGasPricingConstraints"];

#[test]
fn every_recorded_read_is_a_listed_method_and_reads_as_cast_read_it() {
    let file = test_support::elysium();
    let entries = file["nitro"].as_array().unwrap();
    assert_eq!(entries.len(), 105);
    let mut unlisted = Vec::new();
    let mut read = 0;
    for entry in entries {
        let (precompile, name) = (text(&entry["precompile"]), text(&entry["name"]));
        let Some(method) = methods(precompile).iter().find(|method| method.signature.starts_with(&format!("{name}(")))
        else {
            unlisted.push(name);
            continue;
        };
        let data = test_support::bytes(&entry["params"][0]["data"]);
        assert_eq!(method.selector(), data[..4], "{precompile}.{name}: selector");
        // The interface's own declaration, read independently of the crate's list.
        let declared = Function::parse(text(&entry["signature"])).unwrap();
        assert_eq!(declared.signature(), method.signature, "{precompile}.{name}: signature");
        let returns = abi::params(method.returns).unwrap();
        assert_eq!(abi::tuple_type(&declared.outputs), abi::tuple_type(&returns), "{precompile}.{name}: returns");
        let address = precompile_address(precompile);
        assert_eq!(text(&entry["params"][0]["to"]), address, "{precompile}.{name}: address");
        if entry.get("reverted").is_some() {
            assert!(test_support::result(text(&entry["answer"])).is_null(), "{name}: a revert has no result");
            continue;
        }
        let tokens = method.answer(&crate::nitro::support::answered(entry)).unwrap();
        let ours: Vec<Value> = tokens.iter().zip(&returns).map(|(token, param)| shown(token, &param.kind)).collect();
        let theirs: Vec<Value> = entry["decoded"].as_array().unwrap().iter().map(normal).collect();
        assert_eq!(ours, theirs, "{precompile}.{name}");
        read += 1;
    }
    assert_eq!(unlisted, UNLISTED);
    assert_eq!(read, 97, "the six others reverted");
}

fn precompile_address(name: &str) -> String {
    ALL.iter().find(|one| one.name == name).map(|one| one.address.to_hex()).unwrap()
}

/// No two methods a precompile lists share a selector, and each is named once.
#[test]
fn no_two_listed_methods_share_a_selector() {
    for (name, list) in [
        ("ArbSys", sys::METHODS),
        ("ArbInfo", info::METHODS),
        ("ArbAddressTable", address_table::METHODS),
        ("ArbOwnerPublic", owner::METHODS),
        ("ArbGasInfo", gas_info::METHODS),
        ("ArbAggregator", aggregator::METHODS),
        ("ArbRetryableTx", retryable::METHODS),
        ("ArbStatistics", statistics::METHODS),
        ("ArbWasm", wasm::METHODS),
        ("NodeInterface", node::METHODS),
    ] {
        let mut selectors: Vec<[u8; 4]> = list.iter().map(Method::selector).collect();
        selectors.sort_unstable();
        selectors.dedup();
        assert_eq!(selectors.len(), list.len(), "{name}");
        for method in list {
            assert!(abi::params(method.returns).is_ok(), "{name}: {}", method.signature);
            assert!(abi::selector(method.signature).is_ok(), "{name}: {}", method.signature);
            assert_eq!(abi::selector(method.signature).unwrap(), method.selector(), "{name}: {}", method.signature);
        }
    }
}
