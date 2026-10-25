//! The client against a wire that answers as Elysium testnet did (`elysium_testnet.json`): each
//! read goes out once as the node took it, an unfiltered log scan is sent in windows the node
//! takes, and a send keeps its nonce until one fails.

use serde_json::{Value, json};

use crate::client::rpc::LogFilter;
use crate::test_support::{Wire, answer, block_on, ok, result};

use super::*;

fn reader(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Client<Wire> {
    Client::reader(Wire::answering(answers), Network::Testnet)
}

fn params(client: &Client<Wire>, nth: usize) -> Value {
    client.http().bodies()[nth]["params"].clone()
}

#[test]
fn each_read_is_one_call_to_the_public_node() {
    let client = reader([
        ok(&answer("eth_chainId", 0)),
        ok(&answer("eth_getBalance", 0)),
        ok(&answer("eth_getTransactionCount", 0)),
        ok(&answer("eth_getCode", 2)),
        ok(&answer("eth_gasPrice", 0)),
        ok(&answer("eth_getTransactionReceipt", 3)),
    ]);
    let operator: Address = "0xA16A365aE8f34EDaDB8157849d4ACe274bBa9164".parse().unwrap();
    assert_eq!(block_on(client.chain_id()), Ok(99_801));
    assert_eq!(block_on(client.hype(operator)), Ok(Dec::parse("1.010307903950076094").unwrap()));
    assert_eq!(block_on(client.nonce(operator)), Ok(0x5f));
    assert_eq!(params(&client, 2), json!([operator.to_hex(), "pending"]), "counting what the sequencer took");
    assert_eq!(block_on(client.code(crate::nitro::ARB_SYS)), Ok(vec![0xfe]), "a precompile's code");
    assert_eq!(block_on(client.gas_price()), Ok(U256::from_u64(0x5f71980)));
    let deposit = block_on(client.receipt(B256::ZERO)).unwrap().unwrap();
    assert_eq!(deposit.tx_kind(), Some(TxKind::Deposit));
    let sent = client.http().sent();
    assert!(sent.iter().all(|request| request.url == "https://testnet-rpc.elysium.kinetiq.xyz"));
    assert_eq!(sent.len(), 6, "each once");
    assert_eq!(
        format!("{client:?}"),
        r#"Client { network: Testnet, chain: 99801, url: "https://testnet-rpc.elysium.kinetiq.xyz", .. }"#
    );
    let parent = Client::parent(Wire::answering([]), Network::Testnet, ());
    assert_eq!(
        (parent.chain(), parent.rpc().url()),
        (998, "https://rpc.hyperliquid-testnet.xyz/evm"),
        "HyperEVM testnet"
    );
}

#[test]
fn an_unfiltered_scan_goes_out_in_windows_and_a_filtered_one_whole() {
    let empty = r#"{"jsonrpc":"2.0","id":1,"result":[]}"#;
    let client = reader([ok(empty), ok(empty), ok(empty), ok(&answer("eth_getLogs", 0))]);
    let wide = LogFilter {
        from_block: Some(BlockTag::Number(0xee66f2)),
        to_block: Some(BlockTag::Number(0xee7a79)),
        ..LogFilter::default()
    };
    assert_eq!(block_on(client.logs(&wide)), Ok(Vec::new()));
    let windows: Vec<(u64, u64)> = (0..3)
        .map(|nth| {
            let filter = &params(&client, nth)[0];
            let block =
                |key: &str| u64::from_str_radix(filter[key].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
            (block("fromBlock"), block("toBlock"))
        })
        .collect();
    assert!(windows.iter().all(|(from, to)| to - from <= 2_000), "{windows:?}");
    assert_eq!((windows[0].0, windows[2].1), (0xee66f2, 0xee7a79));
    let recorded = &result(&answer("eth_getLogs", 0));
    let narrow = LogFilter { address: vec![crate::nitro::ARB_RETRYABLE_TX], ..wide };
    let logs = block_on(client.logs(&narrow)).unwrap();
    assert_eq!(logs.len(), recorded.as_array().unwrap().len());
    assert_eq!(params(&client, 3)[0]["fromBlock"], "0xee66f2", "sent whole");
}

#[test]
fn a_scan_to_the_head_reads_the_head_once() {
    let empty = r#"{"jsonrpc":"2.0","id":1,"result":[]}"#;
    let head = r#"{"jsonrpc":"2.0","id":1,"result":"0xbb8"}"#;
    let client = reader([ok(head), ok(empty), ok(empty)]);
    let filter =
        LogFilter { from_block: Some(BlockTag::Number(0)), to_block: Some(BlockTag::Latest), ..LogFilter::default() };
    assert_eq!(block_on(client.logs(&filter)), Ok(Vec::new()));
    let bodies = client.http().bodies();
    assert_eq!(bodies[0]["method"], "eth_blockNumber");
    assert_eq!(
        (bodies[1]["params"][0]["toBlock"].clone(), bodies[2]["params"][0]["toBlock"].clone()),
        (json!("0x7d0"), json!("0xbb8"))
    );
}

#[cfg(feature = "sign")]
mod sending {
    use crate::account::LocalKey;
    use crate::test_support::status;

    use super::*;

    /// Hardhat's account #0, a published test key. Only the fake wire sees what it signs.
    fn key() -> LocalKey {
        LocalKey::from_hex("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80").unwrap()
    }

    fn hash(raw: &str) -> String {
        let bytes = crate::utils::hex::decode(raw).unwrap();
        format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{}"}}"#, tx_hash(&bytes))
    }

    #[test]
    fn a_send_is_estimated_capped_signed_and_its_nonce_kept() {
        let estimate = r#"{"jsonrpc":"2.0","id":1,"result":"0x52b6"}"#;
        let price = r#"{"jsonrpc":"2.0","id":1,"result":"0x5f5e100"}"#;
        let count = r#"{"jsonrpc":"2.0","id":1,"result":"0x7"}"#;
        let refused = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"nonce too low"}}"#;
        let me = key().address();
        let call = Call { to: Some(me), value: U256::ZERO, data: Vec::new() };
        let client = Client::new(Wire::answering([ok(estimate), ok(price), ok(count)]), Network::Testnet, key());
        let tx = block_on(client.prepare(call.clone())).unwrap();
        assert_eq!((tx.chain_id, tx.nonce, tx.gas), (99_801, 7, 0x52b6 * 13 / 10), "the estimate with 30% over");
        assert_eq!(
            (tx.max_priority_fee, tx.max_fee),
            (U256::ZERO, U256::from_u64(200_000_000)),
            "no tip, twice the base"
        );
        assert_eq!(client.nonces().kept(), Some(7));
        let raw = block_on(client.sign(&tx)).unwrap();
        assert_eq!(tx.signer(&key().sign_digest(&tx.signing_hash().0).unwrap()), Ok(me));
        assert_eq!(tx_hash(&raw), tx_hash(&tx.sign(&key()).unwrap()), "the bytes a local key signs");

        // Sent: the next prepare takes the next nonce without asking.
        let client = Client::new(
            Wire::answering([
                ok(estimate),
                ok(price),
                ok(count),
                ok(&hash(&crate::utils::hex::encode_prefixed(&raw))),
                ok(estimate),
                ok(price),
                status(200, refused),
                ok(estimate),
                ok(price),
                ok(count),
            ]),
            Network::Testnet,
            key(),
        );
        let sent = block_on(client.send(call.clone())).unwrap();
        assert_eq!(sent, tx_hash(&raw));
        assert_eq!(client.nonces().kept(), Some(8));
        let refusal = block_on(client.send(call.clone())).unwrap_err();
        assert!(matches!(refusal, SendError::Rpc(RpcError::Node { .. })), "{refusal:?}");
        assert_eq!(client.nonces().kept(), None, "a refused send forgets the nonce");
        assert_eq!(block_on(client.prepare(call)).unwrap().nonce, 7, "read from the node again");
        let methods: Vec<String> =
            client.http().bodies().iter().map(|body| body["method"].as_str().unwrap().to_owned()).collect();
        assert_eq!(
            methods,
            [
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_getTransactionCount",
                "eth_sendRawTransaction",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_sendRawTransaction",
                "eth_estimateGas",
                "eth_gasPrice",
                "eth_getTransactionCount",
            ]
        );
    }

    #[test]
    fn a_sync_send_answers_its_receipt_and_a_lost_answer_forgets_the_nonce() {
        let estimate = r#"{"jsonrpc":"2.0","id":1,"result":"0x52b6"}"#;
        let price = r#"{"jsonrpc":"2.0","id":1,"result":"0x5f5e100"}"#;
        let count = r#"{"jsonrpc":"2.0","id":1,"result":"0x0"}"#;
        let receipt = answer("eth_getTransactionReceipt", 2);
        let call = Call { to: Some(key().address()), value: U256::ZERO, data: Vec::new() };
        let client = Client::new(
            Wire::answering([ok(estimate), ok(price), ok(count), ok(&receipt), ok(estimate), ok(price)]),
            Network::Testnet,
            key(),
        );
        let landed = block_on(client.send_sync(call.clone(), Some(2_000))).unwrap();
        assert!(landed.succeeded() && landed.gas_used_for_l1 > 0);
        assert_eq!(client.http().bodies()[3]["method"], "eth_sendRawTransactionSync");
        assert_eq!(client.http().bodies()[3]["params"][1], 2000);
        let lost = block_on(client.send_sync(call, None)).unwrap_err();
        assert!(matches!(lost, SendError::Rpc(RpcError::Transport(TransportError::Unreachable(_)))), "{lost:?}");
        assert_eq!(client.nonces().kept(), None, "it may have landed: the node will say");
    }
}
