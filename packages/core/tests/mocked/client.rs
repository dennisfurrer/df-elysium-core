//! Every [`Client`] read against the fake node answering as Elysium testnet did: each goes out with
//! the params the recording was made with (the node answers nothing else) and reads the node's
//! own answer into its type, Nitro's fields included. Failures of the wire stay apart from the
//! node's refusals, and nothing is retried.

use df_elysium_core::client::rpc::{ErrorKind, nitro as queries};
use df_elysium_core::client::{BlockTag, BlockTransactions, Call, RpcError, TransportError, TxKind};
use df_elysium_core::nitro::{aggregator, gas_info, node, owner, retryable, sys, wasm};
use df_elysium_core::{Client, Dec, Network, U256};
use serde_json::Value;

use crate::support::node::{Chain, Node};
use crate::support::{address, call, elysium, nitro, result, word};

const OPERATOR: &str = "0xa16a365ae8f34edadb8157849d4ace274bba9164";
const GUARDIAN: &str = "0x8499e50474b6e8c988b232f52583439abf37a8a1";
/// The block holding an internal, a legacy and an EIP-1559 transaction.
const BLOCK: u64 = 0xee_63c6;

/// A node that answers every recorded Elysium call and precompile read.
fn recorded() -> Client<Node> {
    let file = elysium();
    let node = Node::testnet()
        .all_recorded(Chain::Elysium, file["calls"].as_array().unwrap())
        .all_recorded(Chain::Elysium, file["nitro"].as_array().unwrap());
    Client::reader(node, Network::Testnet)
}

fn quantity(value: &Value) -> u64 {
    u64::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

#[tokio::test]
async fn the_chain_its_accounts_and_its_gas_read_as_testnet_answered() {
    let client = recorded();
    assert_eq!(client.chain_id().await, Ok(Network::Testnet.chain_id()));
    assert_eq!(client.block_number().await, Ok(0xee_7a79));
    let operator = address(OPERATOR);
    assert_eq!(client.balance(operator).await, Ok(U256::from(1_010_307_903_950_076_094u64)));
    assert_eq!(client.hype(operator).await, Ok(Dec::parse("1.010307903950076094").unwrap()), "HYPE, exactly");
    assert_eq!(client.nonce(operator).await, Ok(0x5f), "counting what the sequencer took");
    assert_eq!(client.gas_price().await, Ok(U256::from(0x5f7_1980u64)));
    assert_eq!(client.code(df_elysium_core::nitro::ARB_SYS).await, Ok(vec![0xfe]), "a precompile's code is 0xfe");
    let multicall = client.code(df_elysium_core::transfer::multicall::MULTICALL3).await.unwrap();
    assert!(multicall.len() > 1_000, "Multicall3 is predeployed");
    assert_eq!(
        client.code(address("0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed")).await,
        Ok(Vec::new()),
        "no CreateX when this was recorded"
    );
    assert_eq!(
        client.run(queries::client_version()).await.unwrap(),
        "nitro/vlocal+unknown-unknown/linux-amd64/go1.25.12"
    );
    assert_eq!(client.run(queries::net_version()).await.unwrap(), "99801");

    // A HYPE transfer, split by the NodeInterface: a little over 21,000, nine of it HyperEVM's data.
    let transfer = Call { to: Some(address(GUARDIAN)), value: U256::ONE, data: Vec::new() };
    let estimate = client.estimate(Some(operator), &transfer).await.unwrap();
    assert_eq!((estimate.gas, estimate.gas_for_l1, estimate.gas_for_l2()), (21_174, 9, 21_165));
    assert_eq!((estimate.base_fee, estimate.l1_base_fee), (U256::from(100_030_000u64), U256::from(376_087u64)));
    assert_eq!(estimate.cost(), U256::from(21_174u64 * 100_030_000));
    assert_eq!(estimate.hype(), Ok(Dec::parse("0.00000211803522").unwrap()));
}

#[tokio::test]
async fn receipts_and_transactions_of_every_type_read_with_nitro_s_fields() {
    let client = recorded();
    for entry in
        elysium()["calls"].as_array().unwrap().iter().filter(|call| call["method"] == "eth_getTransactionReceipt")
    {
        let said = result(entry);
        let hash = word(entry["params"][0].as_str().unwrap());
        let receipt = client.receipt(hash).await.unwrap();
        let Some(receipt) = receipt else {
            assert!(said.is_null(), "not in a block: {hash}");
            continue;
        };
        assert_eq!(receipt.transaction_hash, hash);
        assert_eq!(receipt.gas_used, quantity(&said["gasUsed"]));
        assert_eq!(receipt.gas_used_for_l1, quantity(&said["gasUsedForL1"]));
        assert_eq!(receipt.l1_block_number, Some(quantity(&said["l1BlockNumber"])));
        assert_eq!(receipt.tx_kind().map(TxKind::byte), Some(u8::try_from(quantity(&said["type"])).unwrap()));
        assert_eq!(receipt.logs.len(), said["logs"].as_array().unwrap().len());
        assert!(receipt.succeeded());
        let price = said["effectiveGasPrice"].as_str().unwrap().parse::<U256>().unwrap();
        assert_eq!(receipt.fee(), price.checked_mul(U256::from(receipt.gas_used)), "every unit at the price paid");
    }
    // The EIP-1559 one: eight of its units paid for HyperEVM's data.
    let paid = client.receipt(word("0x739155450a85e6248c875b205fe178f562095371e8625ca14e7edc5f62d3b060")).await;
    let paid = paid.unwrap().unwrap();
    assert_eq!((paid.gas_used, paid.gas_used_for_l1, paid.gas_used_for_l2()), (0x11edd, 8, 0x11edd - 8));

    let kinds = [
        ("0xf9aa32c040241a4fd2ba16c2f93ba25c1477ecd9245a43f7316af3ba91a91060", TxKind::Internal),
        ("0xaa1b1066ed13bc2d6b36a29f22df0b8b59bd8fb856cfb781e75b71e6a27a7d29", TxKind::Legacy),
        ("0x739155450a85e6248c875b205fe178f562095371e8625ca14e7edc5f62d3b060", TxKind::DynamicFee),
        ("0xc10215c38b8bacde3e39632d81569f3e230ee6940b126ce99f4891fdb5fd72a0", TxKind::Deposit),
        ("0xb6242bd2b0816e0bfb40140a0454cc6181a8c652c8163ed8625df43d6682e5d3", TxKind::SubmitRetryable),
        ("0x9bbe17298d6b381c6084fef47a9479b2bd6dc42f9254e82683a2e75bdd282b4e", TxKind::Retry),
    ];
    for (hash, kind) in kinds {
        let transaction = client.transaction(word(hash)).await.unwrap().unwrap();
        assert_eq!((transaction.hash, transaction.tx_kind()), (word(hash), kind), "{hash}");
        assert_eq!(transaction.tx_kind().is_nitro(), !matches!(kind, TxKind::Legacy | TxKind::DynamicFee));
    }
    let internal = client.transaction(word(kinds[0].0)).await.unwrap().unwrap();
    assert_eq!((internal.from, internal.to), (df_elysium_core::nitro::ARBOS, Some(df_elysium_core::nitro::ARBOS)));
    let deposit = client.transaction(word(kinds[3].0)).await.unwrap().unwrap();
    assert_eq!(deposit.message_number(), Some(U256::from(1_493_518u64)), "the parent inbox's message");
    let submit = client.transaction(word(kinds[4].0)).await.unwrap().unwrap();
    assert_eq!(submit.beneficiary, Some(address("0xad32ee12973276936c27c7c4abec01c80828767b")));
    assert_eq!(submit.deposit_value, Some(U256::from(0x1_8715_2d40_e000u64)));
    let retry = client.transaction(word(kinds[5].0)).await.unwrap().unwrap();
    assert_eq!(retry.ticket_id, Some(word(kinds[4].0)), "a redeem names the ticket it runs: the submission's hash");
    let eip1559 = client.transaction(word(kinds[2].0)).await.unwrap().unwrap();
    let rebuilt = eip1559.to_eip1559().unwrap();
    let signature = eip1559.signature().unwrap();
    assert_eq!(df_elysium_core::client::tx_hash(&rebuilt.signed(&signature).unwrap()), eip1559.hash);
    assert_eq!(rebuilt.signer(&signature), Ok(eip1559.from), "rebuilt, it hashes and recovers as the node says");

    let unknown = word("0x1111111111111111111111111111111111111111111111111111111111111111");
    assert_eq!(client.receipt(unknown).await, Ok(None));
    assert_eq!(client.transaction(unknown).await, Ok(None));
}

#[tokio::test]
async fn a_block_names_its_parent_block_send_root_and_arbos_and_reads_by_number_or_hash() {
    let client = recorded();
    let said = result(&call("eth_getBlockByNumber", 0));
    let block = client.block(BlockTag::Number(BLOCK), false).await.unwrap().unwrap();
    assert_eq!(block.number, BLOCK);
    assert_eq!(block.l1_block_number, 0x40b_4862, "sequenced at this HyperEVM block");
    assert_eq!(block.send_count, quantity(&said["sendCount"]));
    assert_eq!(block.send_root.to_hex(), said["sendRoot"].as_str().unwrap());
    assert_eq!(block.arbos_version(), 61, "the mix hash's third eight bytes");
    assert_eq!(block.miner, df_elysium_core::nitro::aggregator::SEQUENCER);
    assert_eq!(block.gas_limit, 0x4_0000_0000_0000, "the header's, not the real limit");
    let BlockTransactions::Hashes(hashes) = &block.transactions else { panic!("hashes were asked for") };
    assert_eq!(hashes.len(), 4);

    let full = client.block(BlockTag::Number(BLOCK), true).await.unwrap().unwrap();
    let BlockTransactions::Full(transactions) = &full.transactions else { panic!("whole transactions") };
    let kinds: Vec<TxKind> = transactions.iter().map(|transaction| transaction.tx_kind()).collect();
    assert_eq!(kinds, [TxKind::Internal, TxKind::Legacy, TxKind::DynamicFee, TxKind::Legacy], "ArbOS opens the block");

    let by_hash = client.block_by_hash(block.hash, false).await.unwrap().unwrap();
    assert_eq!(by_hash, block);
}

#[tokio::test]
async fn precompiles_read_through_the_client_as_testnet_answered() {
    let client = recorded();
    let reported = client.read(&sys::arb_os_version()).await.unwrap();
    assert_eq!((reported, sys::arbos(reported)), (116, 61));
    assert_eq!(client.read(&sys::arb_chain_id()).await, Ok(99_801));
    assert_eq!(
        client.read(&aggregator::batch_posters()).await.unwrap(),
        [aggregator::SEQUENCER, address("0xb42125c7D54E666F4e76AaBBEc8Fe5fDD7281028")]
    );
    assert_eq!(
        client.read(&owner::chain_owners()).await.unwrap(),
        [address("0xFa78c1a82455a6ce83aA82CF464D74e88b384600")]
    );
    let prices = client.read(&gas_info::prices_in_wei()).await.unwrap();
    assert_eq!(prices.per_arb_gas_total, U256::from(100_000_000u64), "the base fee, at its floor");
    assert_eq!(client.read(&gas_info::minimum_gas_price()).await, Ok(U256::from(100_000_000u64)));
    assert_eq!(client.read(&gas_info::max_tx_gas_limit()).await, Ok(U256::from(32_000_000u64)));
    assert_eq!(client.read(&retryable::lifetime()).await, Ok(U256::from(604_800u64)), "a week");
    assert_eq!(client.read(&wasm::stylus_version()).await, Ok(3));
    assert_eq!(client.read(&node::nitro_genesis_block()).await, Ok(U256::ZERO));
    let decoded = &nitro("getL1BaseFeeEstimate", 0)["decoded"][0];
    assert_eq!(client.read(&gas_info::l1_base_fee_estimate()).await.unwrap().to_string(), decoded.as_str().unwrap());

    // A ticket that is gone reverts with NoTicketWithID().
    let gone = client
        .read(&retryable::timeout(word("0x0000000000000000000000000000000000000000000000000000000000000001")))
        .await;
    let error = gone.unwrap_err();
    assert!(retryable::no_ticket(&error), "{error}");
}

#[tokio::test]
async fn a_wire_failure_stays_apart_from_the_node_s_refusal_and_nothing_is_retried() {
    let invalid = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params"}}"#;
    let node = Node::testnet()
        .retry_after(Chain::Elysium, 3)
        .status(Chain::Elysium, None, 502, "bad gateway")
        .status(Chain::Elysium, None, 400, invalid)
        .unreachable(Chain::Elysium, None)
        .recorded(Chain::Elysium, &call("eth_blockNumber", 0));
    let client = Client::reader(node, Network::Testnet);
    let limited = TransportError::RateLimited { retry_after_ms: Some(3_000) };
    assert_eq!(client.block_number().await, Err(RpcError::Transport(limited)));
    let gateway = TransportError::Status { status: 502, body: "bad gateway".to_owned() };
    assert_eq!(client.block_number().await, Err(RpcError::Transport(gateway)));
    let refused = client.block_number().await;
    let Err(RpcError::Node { method: "eth_blockNumber", error }) = refused else { panic!("{refused:?}") };
    assert_eq!((error.code, error.kind()), (-32602, ErrorKind::InvalidParams), "a refusal, whatever the status");
    assert!(matches!(client.block_number().await, Err(RpcError::Transport(TransportError::Unreachable(_)))));
    assert_eq!(client.block_number().await, Ok(0xee_7a79));
    assert_eq!(client.http().methods(Chain::Elysium).len(), 5, "each sent once");
}
