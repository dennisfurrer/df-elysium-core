//! Multicall3 against viem: `aggregate3`'s calldata (`encodeFunctionData`, checked against cast
//! by the generator) and a live answer holding one failure, decoded by viem (`arbitrum_sdk.json`).

use crate::test_support::{arbitrum_sdk, bytes, text};

use super::*;

#[test]
fn a_batch_writes_viems_calldata_and_reads_each_outcome() {
    let file = arbitrum_sdk();
    let case = file["calldata"].as_array().unwrap().iter().find(|case| case["contract"] == "Multicall3").unwrap();
    let calls: Vec<Call3> = case["args"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| Call3 {
            target: text(&call[0]).parse().unwrap(),
            allow_failure: text(&call[1]) == "true",
            data: bytes(&call[2]),
        })
        .collect();
    assert_eq!(calls.len(), 2);
    let read = aggregate3(&calls);
    assert_eq!(read.call.to, Some(MULTICALL3));
    assert_eq!(crate::utils::hex::encode_prefixed(&read.call.data), text(&case["calldata"]));

    let answer = file["answers"].as_array().unwrap().iter().find(|one| one["contract"] == "Multicall3").unwrap();
    let outcomes = read.decode(&bytes(&answer["data"])).unwrap();
    let decoded = answer["decoded"].as_array().unwrap();
    let decoded = decoded.first().and_then(serde_json::Value::as_array).map_or(decoded.as_slice(), Vec::as_slice);
    assert_eq!(outcomes.len(), decoded.len());
    for (outcome, theirs) in outcomes.iter().zip(decoded) {
        let success = theirs.get("success").or_else(|| theirs.get(0)).unwrap();
        let data = theirs.get("returnData").or_else(|| theirs.get(1)).unwrap();
        assert_eq!(outcome.success, success.as_bool().unwrap_or_else(|| text(success) == "true"));
        assert_eq!(crate::utils::hex::encode_prefixed(&outcome.data), text(data));
    }
    assert!(outcomes.iter().any(|outcome| !outcome.success), "one call failed and the batch went on");
}

#[test]
fn a_typed_read_goes_into_a_batch_as_its_call() {
    let read = eth_balance(Address::from_bytes([0x11; 20]));
    let call = Call3::of(&read, true);
    assert_eq!((call.target, call.allow_failure, call.data.clone()), (MULTICALL3, true, read.call.data.clone()));
    assert_eq!(aggregate3(&[]).call.data.len(), 4 + 64, "an empty batch: the offset and a length of zero");
    assert_eq!(block_number().call.data, GET_BLOCK_NUMBER.selector());
}
