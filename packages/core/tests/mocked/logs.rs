//! Log scans through [`Client::logs`] against the fake node: an unfiltered scan goes out in windows
//! inside the public node's 2,000-block cap, a filtered one whole, and the node's own refusal of an
//! oversized query (recorded verbatim, HTTP 400) reads as its JSON-RPC error.

use df_elysium_core::client::rpc::ErrorKind;
use df_elysium_core::client::{BlockTag, LogFilter, RpcError};
use df_elysium_core::{Client, Network};
use serde_json::Value;

use crate::support::node::{Chain, Node};
use crate::support::{address, answer, call};

/// The head when the recordings were made.
const HEAD: u64 = 0xee_7a79;
/// 5,000 blocks back: the refused query's start.
const FROM: u64 = HEAD - 4_999;

fn block(value: &Value) -> u64 {
    u64::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

/// The windows the node was asked for, as `(fromBlock, toBlock)`.
fn windows(node: &Node) -> Vec<(u64, u64)> {
    node.calls(Chain::Elysium)
        .iter()
        .filter(|call| call["method"] == "eth_getLogs")
        .map(|call| (block(&call["params"][0]["fromBlock"]), block(&call["params"][0]["toBlock"])))
        .collect()
}

/// A node whose every `eth_getLogs` answers the recorded ArbRetryableTx logs.
fn node() -> Node {
    Node::testnet()
        .answer(Chain::Elysium, "eth_getLogs", answer(&call("eth_getLogs", 0)))
        .recorded(Chain::Elysium, &call("eth_blockNumber", 0))
}

#[tokio::test]
async fn an_unfiltered_scan_goes_out_in_windows_inside_the_cap_and_misses_no_block() {
    let client = Client::reader(node(), Network::Testnet);
    let filter = LogFilter {
        from_block: Some(BlockTag::Number(FROM)),
        to_block: Some(BlockTag::Number(HEAD)),
        ..LogFilter::default()
    };
    let logs = client.logs(&filter).await.unwrap();
    let asked = windows(client.http());
    assert_eq!(asked, [(FROM, FROM + 2_000), (FROM + 2_001, FROM + 4_001), (FROM + 4_002, HEAD)]);
    assert!(asked.iter().all(|(from, to)| to - from <= Network::Testnet.log_span()), "{asked:?}");
    let recorded = crate::support::result(&call("eth_getLogs", 0)).as_array().unwrap().len();
    assert_eq!(logs.len(), 3 * recorded, "every window's logs, in order");
    assert!(!client.http().methods(Chain::Elysium).contains(&"eth_blockNumber".to_owned()), "both ends were numbers");

    // A tag reads the head once: the scan runs to it.
    let client = Client::reader(node(), Network::Testnet);
    let to_head = LogFilter { from_block: Some(BlockTag::Number(FROM)), to_block: Some(BlockTag::Latest), ..filter };
    client.logs(&to_head).await.unwrap();
    assert_eq!(windows(client.http()).last(), Some(&(FROM + 4_002, HEAD)));
    assert_eq!(client.http().methods(Chain::Elysium).iter().filter(|method| *method == "eth_blockNumber").count(), 1);
}

#[tokio::test]
async fn a_filtered_scan_goes_out_whole_over_any_range() {
    let client = Client::reader(node(), Network::Testnet);
    let filter = LogFilter {
        from_block: Some(BlockTag::Number(0)),
        to_block: Some(BlockTag::Number(HEAD)),
        address: vec![address("0x000000000000000000000000000000000000006e")],
        ..LogFilter::default()
    };
    client.logs(&filter).await.unwrap();
    assert_eq!(windows(client.http()), [(0, HEAD)], "an address lifts the cap");
}

#[tokio::test]
async fn the_node_s_refusal_of_an_oversized_query_reads_as_its_error_whatever_the_status() {
    let refusal = call("eth_getLogs", 1);
    assert_eq!(refusal["status"], 400);
    let client = Client::reader(Node::testnet().recorded(Chain::Elysium, &refusal), Network::Testnet);
    let filter = LogFilter {
        from_block: Some(BlockTag::Number(FROM)),
        to_block: Some(BlockTag::Number(HEAD)),
        ..LogFilter::default()
    };
    // Straight through df-hyperliquid-core's Rpc, unpaged, as the node was asked.
    let refused = client.rpc().logs(&filter).await;
    let Err(RpcError::Node { method: "eth_getLogs", error }) = refused else { panic!("{refused:?}") };
    assert_eq!((error.code, error.kind()), (-32602, ErrorKind::InvalidParams));
    assert!(error.message.contains("exceeds maximum of 2000"), "{}", error.message);
}
