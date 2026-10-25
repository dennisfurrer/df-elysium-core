//! What the mocked, conformance, live and send suites share: the recordings on disk, a published
//! test key, and [`node::Node`], a fake Elysium and HyperEVM that answers from recordings.

// Each suite uses its own part of this.
#![allow(dead_code)]

pub mod node;

use std::path::PathBuf;

use df_elysium_core::account::LocalKey;
use df_elysium_core::utils::{Address, hex};
use df_elysium_core::{B256, U256};
use serde_json::Value;

/// Hardhat's account #0, a published test key. Only the fake node ever sees what it signs.
pub const KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

pub fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

/// A file under `fixtures/`: what a service answered, recorded.
pub fn fixture(path: &str) -> String {
    read(&root().join("fixtures").join(path))
}

/// A file under `vectors/`: what the chain answered, or what other implementations produce.
pub fn vector(path: &str) -> String {
    read(&root().join("vectors").join(path))
}

/// Elysium testnet's answers, verbatim (`elysium_testnet.json`).
pub fn elysium() -> Value {
    serde_json::from_str(&vector("elysium_testnet.json")).unwrap()
}

/// HyperEVM testnet's answers to the rollup's contracts, verbatim (`hyperevm_testnet.json`).
pub fn hyperevm() -> Value {
    serde_json::from_str(&vector("hyperevm_testnet.json")).unwrap()
}

/// What @arbitrum/sdk 4.1.0 and viem 2.57.4 compute (`arbitrum_sdk.json`).
pub fn arbitrum_sdk() -> Value {
    serde_json::from_str(&vector("arbitrum_sdk.json")).unwrap()
}

/// The `nth` recorded call of `method` to Elysium, counted from zero: its method, params, status
/// and answer.
pub fn call(method: &str, nth: usize) -> Value {
    let file = elysium();
    let found = file["calls"].as_array().unwrap().iter().filter(|call| call["method"] == method).nth(nth);
    found.unwrap_or_else(|| panic!("no {method} #{nth}")).clone()
}

/// The `nth` recorded read of the precompile method `name`.
pub fn nitro(name: &str, nth: usize) -> Value {
    let file = elysium();
    let found = file["nitro"].as_array().unwrap().iter().filter(|call| call["name"] == name).nth(nth);
    found.unwrap_or_else(|| panic!("no nitro {name} #{nth}")).clone()
}

/// One step of a recorded journey across both chains.
pub fn step(journey: &str, step: &str) -> Value {
    let file = elysium();
    let found = &file["journeys"][journey][step];
    assert!(found.is_object(), "no {journey}.{step}");
    found.clone()
}

/// The recorded read of the parent's contract `name`.
pub fn parent(name: &str) -> Value {
    let file = hyperevm();
    let found = file["calls"].as_array().unwrap().iter().find(|call| call["name"] == name);
    found.unwrap_or_else(|| panic!("no parent {name}")).clone()
}

/// An entry's answer text.
pub fn answer(entry: &Value) -> &str {
    text(&entry["answer"])
}

/// An answer's `result`.
pub fn result(entry: &Value) -> Value {
    serde_json::from_str::<Value>(answer(entry)).unwrap()["result"].clone()
}

pub fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_else(|| panic!("{value} is not text"))
}

pub fn bytes(value: &Value) -> Vec<u8> {
    hex::decode(text(value)).unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn word(text: &str) -> B256 {
    text.parse().unwrap()
}

/// A number as a vector writes it: decimal, or `0x` hex.
pub fn number(value: &Value) -> U256 {
    text(value).parse().unwrap()
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
