//! Elysium testnet, read only: every [`Client`](df_elysium_core::Client) read, each answer read into
//! its type and its JSON-RPC envelope held against the recording the mocked suite replays
//! (`vectors/elysium_testnet.json`). The recorded transactions and blocks are read again: they are
//! history, and must read the same.

use df_elysium_core::client::rpc::nitro as queries;
use df_elysium_core::client::{BlockTag, BlockTransactions, Call, LogFilter, TxKind};
use df_elysium_core::nitro::{ARB_RETRYABLE_TX, ARB_SYS};
use df_elysium_core::transfer::multicall::MULTICALL3;
use df_elysium_core::{Network, U256};

use crate::support::{address, call, nitro, result, word};
use crate::tap::{reader, same_shape};

const OPERATOR: &str = "0xa16a365ae8f34edadb8157849d4ace274bba9164";
const GUARDIAN: &str = "0x8499e50474b6e8c988b232f52583439abf37a8a1";

#[tokio::test]
#[ignore = "testnet"]
async fn the_chain_its_accounts_and_its_gas_read_as_recorded() {
    let client = reader();
    let http = client.http();
    assert_eq!(client.chain_id().await, Ok(Network::Testnet.chain_id()));
    same_shape(http, &call("eth_chainId", 0));
    assert!(client.block_number().await.unwrap() > 0xee_7a79, "past the recorded height");
    same_shape(http, &call("eth_blockNumber", 0));
    assert_eq!(client.run(queries::net_version()).await.unwrap(), "99801");
    assert!(client.run(queries::client_version()).await.unwrap().starts_with("nitro/"));

    let operator = address(OPERATOR);
    let wei = client.balance(operator).await.unwrap();
    same_shape(http, &call("eth_getBalance", 0));
    assert_eq!(client.hype(operator).await.unwrap(), df_elysium_core::utils::hype(wei).unwrap());
    assert!(client.nonce(operator).await.unwrap() >= 0x5f, "it only goes up");
    same_shape(http, &call("eth_getTransactionCount", 0));
    let price = client.gas_price().await.unwrap();
    same_shape(http, &call("eth_gasPrice", 0));
    assert!(price >= U256::from(100_000_000u64), "at or over the floor: {price}");

    assert_eq!(client.code(ARB_SYS).await, Ok(vec![0xfe]));
    assert!(client.code(MULTICALL3).await.unwrap().len() > 1_000);
    assert!(client.code(df_elysium_core::transfer::PERMIT2).await.unwrap().len() > 1_000);
    assert!(client.code(df_elysium_core::deploy::CREATE2_DEPLOYER).await.unwrap().len() > 10);
    assert_eq!(client.code(address("0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed")).await, Ok(Vec::new()), "no CreateX");

    let transfer = Call { to: Some(address(GUARDIAN)), value: U256::ONE, data: Vec::new() };
    let estimate = client.estimate(Some(operator), &transfer).await.unwrap();
    same_shape(http, &nitro("gasEstimateComponents", 0));
    assert!(estimate.gas > 21_000 && estimate.gas < 22_000, "a little over 21,000: {estimate:?}");
    assert!(estimate.gas_for_l1 > 0, "HyperEVM's data is paid for: {estimate:?}");
}

#[tokio::test]
#[ignore = "testnet"]
async fn the_latest_block_and_the_recorded_history_read_with_nitro_s_fields() {
    let client = reader();
    let http = client.http();
    let latest = client.block(BlockTag::Latest, false).await.unwrap().unwrap();
    same_shape(http, &call("eth_getBlockByNumber", 0));
    assert_eq!(latest.arbos_version(), 61, "ArbOS 61; a new version shows here first");
    assert!(latest.send_count >= 0xb_663f && latest.l1_block_number > 0x40b_4862);
    // Whole transactions are not held against the recording: a block may hold a type it lacked.
    let full = client.block(BlockTag::Number(latest.number), true).await.unwrap().unwrap();
    let BlockTransactions::Full(transactions) = &full.transactions else { panic!("whole transactions") };
    assert_eq!(transactions.first().map(|one| one.tx_kind()), Some(TxKind::Internal), "ArbOS opens every block");
    assert_eq!(client.block_by_hash(latest.hash, false).await.unwrap().unwrap().hash, latest.hash);
    same_shape(http, &call("eth_getBlockByHash", 0));

    for (nth, entry) in [0, 1, 2, 3, 4, 5].map(|nth| (nth, call("eth_getTransactionReceipt", nth))) {
        let hash = word(entry["params"][0].as_str().unwrap());
        let receipt = client.receipt(hash).await.unwrap().unwrap();
        same_shape(http, &entry);
        let said = result(&entry);
        assert_eq!(receipt.transaction_hash, hash, "receipt #{nth}");
        assert_eq!(
            Some(receipt.gas_used_for_l1),
            said["gasUsedForL1"].as_str().map(|text| u64::from_str_radix(&text[2..], 16).unwrap())
        );
        let transaction = client.transaction(hash).await.unwrap().unwrap();
        same_shape(http, &call("eth_getTransactionByHash", nth));
        assert_eq!(transaction.hash, hash);
    }
}

#[tokio::test]
#[ignore = "testnet"]
async fn logs_scan_in_windows_inside_the_cap_and_a_filtered_scan_goes_whole() {
    let client = reader();
    let head = client.block_number().await.unwrap();
    let unfiltered = LogFilter {
        from_block: Some(BlockTag::Number(head - 4_999)),
        to_block: Some(BlockTag::Number(head)),
        ..LogFilter::default()
    };
    let logs = client.logs(&unfiltered).await.unwrap();
    assert!(!logs.is_empty(), "every block logs something");
    let numbers: Vec<u64> = logs.iter().filter_map(|log| log.block_number).collect();
    assert!(numbers.windows(2).all(|pair| pair[0] <= pair[1]), "in order across windows");
    let refused = client.rpc().logs(&unfiltered).await.unwrap_err();
    assert!(refused.to_string().contains("exceeds maximum of 2000"), "{refused}");

    let filtered = LogFilter {
        from_block: Some(BlockTag::Number(head - 50_000)),
        to_block: Some(BlockTag::Number(head)),
        address: vec![ARB_RETRYABLE_TX],
        ..LogFilter::default()
    };
    let tickets = client.logs(&filtered).await.unwrap();
    assert!(tickets.iter().all(|log| log.address == ARB_RETRYABLE_TX), "50,000 blocks in one call");
}
