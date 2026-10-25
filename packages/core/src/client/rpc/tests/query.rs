//! Queries as Elysium's node took them and its answers verbatim (`elysium_testnet.json`): Nitro's
//! queries write the bodies df-hyperliquid-core's do, a refusal is typed whatever the HTTP status,
//! and an answer that does not read says so.

use crate::client::rpc::{ErrorKind, eth, nitro, query};
use crate::client::{OFFLINE, Request, Response};
use crate::test_support::{Wire, answer, block_on, ok, status};
use serde_json::{Value, json};

use super::*;

#[test]
fn each_query_writes_the_body_the_node_takes() {
    let hash: B256 = "0xc10215c38b8bacde3e39632d81569f3e230ee6940b126ce99f4891fdb5fd72a0".parse().unwrap();
    let body = |text: String| serde_json::from_str::<Value>(&text).unwrap();
    assert_eq!(
        body(nitro::receipt(hash).body()),
        json!({"jsonrpc": "2.0", "id": 1, "method": "eth_getTransactionReceipt", "params": [hash.to_hex()]})
    );
    assert_eq!(body(nitro::block(BlockTag::Number(0xee63c6), true).body())["params"], json!(["0xee63c6", true]));
    assert_eq!(body(nitro::block_by_hash(hash, false).body())["params"], json!([hash.to_hex(), false]));
    assert_eq!(body(nitro::block_receipts(BlockTag::Latest).body())["params"], json!(["latest"]));
    assert_eq!(body(nitro::send_raw_transaction_sync(&[0x02, 0xc0], None).body())["params"], json!(["0x02c0"]));
    assert_eq!(
        body(nitro::send_raw_transaction_sync(&[0x02, 0xc0], Some(2_000)).body())["params"],
        json!(["0x02c0", 2000]),
        "the timeout in milliseconds, as a number: Nitro refuses a quantity"
    );
    assert_eq!(
        nitro::client_version().answer(&answer("web3_clientVersion", 0)).unwrap(),
        "nitro/vlocal+unknown-unknown/linux-amd64/go1.25.12"
    );
    assert_eq!(nitro::net_version().answer(&answer("net_version", 0)).unwrap(), "99801");
    assert_eq!(nitro::receipt(hash).answer(&answer("eth_getTransactionReceipt", 6)), Ok(None), "not in a block");
    assert_eq!(nitro::transaction(hash).answer(&answer("eth_getTransactionByHash", 6)), Ok(None), "not known");
}

#[test]
fn a_refusal_is_a_typed_node_error_whatever_the_status() {
    // The unfiltered 4999-block scan, refused with HTTP 400.
    let rpc =
        Rpc::new(Wire::answering([status(400, &answer("eth_getLogs", 1))]), "https://testnet-rpc.elysium.kinetiq.xyz");
    let refused = block_on(run(&rpc, nitro::block(BlockTag::Latest, false))).unwrap_err();
    let RpcError::Node { error, .. } = refused else { panic!("{refused:?}") };
    assert_eq!((error.code, error.kind()), (-32602, ErrorKind::InvalidParams));
    assert!(error.message.contains("exceeds maximum of 2000"), "{}", error.message);
    // A send the node could not decode.
    let refused =
        nitro::send_raw_transaction_sync(&[0x02, 0xc0], None).answer(&answer("eth_sendRawTransactionSync", 0));
    let Err(RpcError::Node { method, error }) = refused else { panic!("{refused:?}") };
    assert_eq!((method, error.code, error.kind()), ("eth_sendRawTransactionSync", -32000, ErrorKind::Server));
    // A custom error's revert keeps its data.
    let reverted = Query::new("eth_call", Vec::new(), query::data).answer(&answer("eth_call", 1)).unwrap_err();
    let RpcError::Node { error, .. } = reverted else { panic!() };
    assert_eq!((error.code, error.data.as_deref()), (3, Some("0x80698456")), "NoTicketWithID()");
}

#[test]
fn a_status_with_no_refusal_and_a_rate_limit_stay_transport_errors() {
    let wire = Wire::answering([
        status(502, "bad gateway"),
        Ok(Response { status: 429, headers: vec![("Retry-After".to_owned(), "2".to_owned())], body: Vec::new() }),
        ok(&answer("eth_getBlockByNumber", 0)),
    ]);
    let rpc = Rpc::new(wire, "https://testnet-rpc.elysium.kinetiq.xyz");
    let block = || block_on(run(&rpc, nitro::block(BlockTag::Number(0xee63c6), false)));
    assert_eq!(block(), Err(TransportError::Status { status: 502, body: "bad gateway".to_owned() }.into()));
    assert_eq!(block(), Err(TransportError::RateLimited { retry_after_ms: Some(2_000) }.into()));
    assert_eq!(block().unwrap().unwrap().number, 0xee63c6);
    assert_eq!(rpc.http().sent().len(), 3, "each sent once");
    assert!(matches!(
        nitro::client_version().answer(r#"{"jsonrpc":"2.0","id":1,"result":3}"#),
        Err(RpcError::Unreadable { method: "web3_clientVersion", .. })
    ));
    assert!(matches!(nitro::client_version().answer("<html>"), Err(RpcError::Unreadable { .. })));
    assert!(matches!(nitro::client_version().answer(r#"{"jsonrpc":"2.0","id":1}"#), Err(RpcError::Unreadable { .. })));
}

/// What makes an `offline` build refuse a send: the body names a raw transaction, synchronous or not.
#[test]
fn a_raw_transaction_is_what_an_offline_build_refuses() {
    for body in
        [nitro::send_raw_transaction_sync(&[0x02, 0xc0], None).body(), eth::send_raw_transaction(&[0x02, 0xc0]).body()]
    {
        let request = Request::post_json("https://testnet-rpc.elysium.kinetiq.xyz", body);
        assert!(request.is_signed());
        assert_eq!(request.refuse_if_offline().is_err(), OFFLINE);
    }
    let read = Request::post_json("https://testnet-rpc.elysium.kinetiq.xyz", nitro::client_version().body());
    assert_eq!(read.refuse_if_offline(), Ok(()), "a read goes out offline too");
}

/// The refusal a lagging backend answers, as the public node wrote it, and one that is not.
#[test]
fn a_lagging_backend_is_told_apart_from_a_refusal() {
    let lagging = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"header not found"}}"#;
    let error = nitro::client_version().answer(lagging).unwrap_err();
    assert!(super::super::lagging(&error));
    assert!(crate::client::ReadError::Rpc(error).is_lagging());
    let refused = nitro::client_version().answer(&answer("eth_sendRawTransaction", 0)).unwrap_err();
    assert!(!super::super::lagging(&refused));
}
