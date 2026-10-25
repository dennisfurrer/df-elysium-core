//! Test helpers: each precompile read as Elysium testnet answered it (`elysium_testnet.json`,
//! each with cast 1.8.3's decoding of the same bytes), what @arbitrum/sdk 4.1.0 and viem 2.57.4
//! encode for the same calls (`arbitrum_sdk.json`), and a read held to the call that was sent.

use serde_json::Value;

use crate::client::Read;
use crate::test_support::{self, text};
use crate::utils::hex;
use crate::{B256, U256};

/// The `nth` recorded read of `name` on `precompile`, as the file has it.
pub(crate) fn recorded(precompile: &str, name: &str, nth: usize) -> Value {
    let file = test_support::elysium();
    let found = file["nitro"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["precompile"] == precompile && entry["name"] == name)
        .nth(nth);
    found.unwrap_or_else(|| panic!("no {precompile}.{name} #{nth}")).clone()
}

/// The bytes a recorded read answered.
pub(crate) fn answered(entry: &Value) -> Vec<u8> {
    test_support::bytes(&test_support::result(text(&entry["answer"])))
}

/// The `at`-th argument word of a recorded read's calldata, after its selector.
pub(crate) fn arg(entry: &Value, at: usize) -> U256 {
    let data = test_support::bytes(&entry["params"][0]["data"]);
    U256::from_be_slice(&data[4 + at * 32..4 + at * 32 + 32]).unwrap()
}

/// The same argument as a word.
pub(crate) fn word(entry: &Value, at: usize) -> B256 {
    B256::from(arg(entry, at))
}

/// The read went out as recorded (its address, calldata, sender and value), and its answer
/// decodes.
pub(crate) fn sent<T>(read: &Read<T>, precompile: &str, name: &str, nth: usize) -> T {
    let entry = recorded(precompile, name, nth);
    let params = &entry["params"][0];
    assert_eq!(read.call.to.map(|to| to.to_hex()), Some(text(&params["to"]).to_lowercase()), "{name}: address");
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&params["data"]), "{name}: calldata");
    assert_eq!(
        read.call.from.map(|from| from.to_hex()),
        params.get("from").map(|from| text(from).to_lowercase()),
        "{name}: sender"
    );
    assert_eq!(
        read.call.value.map(U256::to_quantity),
        params.get("value").map(|value| text(value).to_owned()),
        "{name}: value"
    );
    read.decode(&answered(&entry)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// cast's decoding of a recorded read: its values in order.
pub(crate) fn decoded(precompile: &str, name: &str, nth: usize) -> Vec<Value> {
    recorded(precompile, name, nth)["decoded"].as_array().unwrap().clone()
}

/// A decoded value as a number: cast writes the wide ones as text and the narrow ones bare.
pub(crate) fn number(value: &Value) -> U256 {
    match value {
        Value::Number(number) => number.to_string().parse().unwrap(),
        other => text(other).parse().unwrap(),
    }
}

/// Every `arbitrum_sdk.json` calldata entry for `contract` and the function named `name`.
pub(crate) fn sdk_calldata(contract: &str, name: &str) -> Vec<Value> {
    let file = test_support::arbitrum_sdk();
    let prefix = format!("function {name}(");
    file["calldata"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["contract"] == contract && text(&entry["signature"]).starts_with(&prefix))
        .cloned()
        .collect()
}

/// The one `arbitrum_sdk.json` calldata entry for `contract` and `name`.
pub(crate) fn sdk_one(contract: &str, name: &str) -> Value {
    let found = sdk_calldata(contract, name);
    assert_eq!(found.len(), 1, "{contract}.{name}");
    found[0].clone()
}

/// An entry's argument as text.
pub(crate) fn sdk_arg(entry: &Value, at: usize) -> &str {
    text(&entry["args"][at])
}

/// An entry's calldata.
pub(crate) fn sdk_data(entry: &Value) -> &str {
    text(&entry["calldata"])
}

/// The `arbitrum_sdk.json` answer for `contract` and `name`, the `nth` of them.
pub(crate) fn sdk_answer(contract: &str, name: &str, nth: usize) -> Value {
    let file = test_support::arbitrum_sdk();
    let prefix = format!("function {name}(");
    let found = file["answers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["contract"] == contract && text(&entry["signature"]).starts_with(&prefix))
        .nth(nth);
    found.unwrap_or_else(|| panic!("no answer {contract}.{name} #{nth}")).clone()
}

/// The `arbitrum_sdk.json` event logged by `contract` under `name`.
pub(crate) fn sdk_event(contract: &str, name: &str) -> Value {
    let file = test_support::arbitrum_sdk();
    let prefix = format!("event {name}(");
    let found = file["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["contract"] == contract && text(&entry["signature"]).starts_with(&prefix));
    found.unwrap_or_else(|| panic!("no event {contract}.{name}")).clone()
}
