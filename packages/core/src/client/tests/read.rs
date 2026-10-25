//! A read's call and its answer: the `eth_call` it writes, sent once through any node, and an
//! answer that does not fit its declared shape refused. Answers are Elysium testnet's, verbatim
//! (`elysium_testnet.json`).

use serde_json::json;

use crate::client::rpc::{ErrorKind, RpcError};
use crate::nitro::{gas_info, sys};
use crate::test_support::{Wire, block_on, elysium, ok, status, text};

use super::*;

fn recorded(name: &str) -> String {
    let file = elysium();
    let found =
        file["nitro"].as_array().unwrap().iter().find(|call| call["name"] == name && call.get("reverted").is_none());
    text(&found.unwrap_or_else(|| panic!("no {name}"))["answer"]).to_owned()
}

#[test]
fn a_read_goes_out_as_one_eth_call_and_its_answer_reads() {
    let wire = Wire::answering([ok(&recorded("arbOSVersion")), ok(&recorded("getPricesInWei"))]);
    let rpc = Rpc::new(wire, "https://testnet-rpc.elysium.kinetiq.xyz");
    assert_eq!(block_on(sys::arb_os_version().on(&rpc, BlockTag::Latest)), Ok(116));
    let prices = block_on(gas_info::prices_in_wei().on(&rpc, BlockTag::Number(5))).unwrap();
    assert_eq!(prices.per_arb_gas_base, U256::from_u64(100_000_000), "the 0.1 gwei floor");
    let bodies = rpc.http().bodies();
    assert_eq!(
        bodies[0]["params"],
        json!([{"to": "0x0000000000000000000000000000000000000064", "data": "0x051038f2"}, "latest"])
    );
    assert_eq!(bodies[1]["params"][1], "0x5");
}

#[test]
fn a_read_carries_its_sender_and_value_when_given() {
    let me = Address::from_bytes([0xaa; 20]);
    let read = sys::is_top_level_call().from(me).with_value(U256::from_u64(7));
    assert_eq!((read.call.from, read.call.value), (Some(me), Some(U256::from_u64(7))));
    assert_eq!(sys::is_top_level_call().with_value(U256::ZERO).call.value, None, "no value written for zero");
    assert_eq!(
        sys::send_merkle_tree_state().call.from,
        Some(Address::ZERO),
        "ArbSys answers it to the zero address only"
    );
}

#[test]
fn a_refusal_or_an_answer_of_another_shape_is_an_error() {
    let refused = r#"{"jsonrpc":"2.0","id":1,"error":{"code":3,"message":"execution reverted","data":"0x80698456"}}"#;
    let rpc = Rpc::new(Wire::answering([status(200, refused), ok(r#"{"jsonrpc":"2.0","id":1,"result":"0x01"}"#)]), "x");
    let error = block_on(sys::arb_os_version().on(&rpc, BlockTag::Latest)).unwrap_err();
    let ReadError::Rpc(RpcError::Node { error: node, .. }) = &error else { panic!("{error:?}") };
    assert_eq!(node.kind(), ErrorKind::Reverted);
    assert!(crate::nitro::retryable::no_ticket(&error), "NoTicketWithID()");
    assert!(
        matches!(block_on(sys::arb_os_version().on(&rpc, BlockTag::Latest)), Err(ReadError::Abi(_))),
        "one byte, not a word"
    );
}
