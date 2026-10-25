//! Polling against the fake node: [`Heads`] fed the heights it answers in turn, and a
//! [`LogCursor`] following ArbRetryableTx's logs a confirmed window at a time, its first window
//! the one recorded on testnet.

use df_elysium_core::client::{BlockTag, LogFilter};
use df_elysium_core::live::{Heads, LogCursor};
use df_elysium_core::nitro::ARB_RETRYABLE_TX;
use df_elysium_core::{Client, Network};

use crate::support::node::{Chain, Node};
use crate::support::{call, result};

fn height(number: u64) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{number:#x}"}}"#)
}

#[tokio::test]
async fn heads_answer_each_new_block_once_and_ignore_a_lagging_node() {
    let heights: Vec<String> = [0x10, 0x10, 0x13, 0x12, 0x14].iter().map(|number| height(*number)).collect();
    let answers: Vec<&str> = heights.iter().map(String::as_str).collect();
    let client = Client::reader(Node::testnet().answers(Chain::Elysium, "eth_blockNumber", &answers), Network::Testnet);
    let mut heads = Heads::new();
    let mut seen = Vec::new();
    for _ in 0..answers.len() {
        seen.push(heads.seen(client.block_number().await.unwrap()));
    }
    assert_eq!(seen, [Some(0x10..=0x10), None, Some(0x11..=0x13), None, Some(0x14..=0x14)]);
    assert_eq!(heads.last(), Some(0x14));
}

#[tokio::test]
async fn a_cursor_reads_each_confirmed_window_once_behind_the_head() {
    let recorded = call("eth_getLogs", 0);
    let first = 0xee_7154;
    let heads = [height(first - 1), height(0xee_7160), height(0xee_7160), height(0xee_7170)];
    let answers: Vec<&str> = heads.iter().map(String::as_str).collect();
    let node = Node::testnet()
        .answers(Chain::Elysium, "eth_blockNumber", &answers)
        .recorded(Chain::Elysium, &recorded)
        .answer(Chain::Elysium, "eth_getLogs", r#"{"jsonrpc":"2.0","id":1,"result":[]}"#);
    let client = Client::reader(node, Network::Testnet);
    let filter = LogFilter { address: vec![ARB_RETRYABLE_TX], ..LogFilter::default() };
    let mut cursor = LogCursor::new(filter, first, Network::Testnet.log_span(), 2);

    let mut read = Vec::new();
    for _ in 0..heads.len() {
        let head = client.block_number().await.unwrap();
        let Some(window) = cursor.window(head) else {
            read.push(None);
            continue;
        };
        let logs = client.logs(&window).await.unwrap();
        cursor.read(&window);
        read.push(Some((window.from_block, window.to_block, logs.len())));
    }
    let recorded_logs = result(&recorded).as_array().unwrap().len();
    assert_eq!(
        read,
        [
            None,
            Some((Some(BlockTag::Number(first)), Some(BlockTag::Number(0xee_715e)), recorded_logs)),
            None,
            Some((Some(BlockTag::Number(0xee_715f)), Some(BlockTag::Number(0xee_716e)), 0)),
        ],
        "two blocks behind the head, never a block twice"
    );
    assert_eq!(cursor.next(), 0xee_716f);
}
