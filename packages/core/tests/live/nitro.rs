//! Elysium testnet, read only: a sweep of ArbOS's precompiles and the NodeInterface through the
//! crate's typed reads, each read into its type and its answer held against the recording of the
//! same read (`nitro` in `vectors/elysium_testnet.json`).

use std::fmt::Debug;

use df_elysium_core::client::{BlockTag, Read, ReadError, RpcError};
use df_elysium_core::nitro::{aggregator, gas_info, info, node, owner, retryable, statistics, sys, wasm};
use df_elysium_core::transfer::multicall::MULTICALL3;
use df_elysium_core::{Client, Network, U256};

use crate::support::{address, nitro};
use crate::tap::{Tap, reader, same_shape};

/// Reads `read` and holds its answer against the recording of `name`. The public RPC balances
/// over nodes, and one a block behind answers `header not found` for `latest`: a read, so it is
/// asked once more.
async fn check<T: Debug>(client: &Client<Tap>, name: &str, read: Read<T>) -> T {
    let behind = |error: &ReadError| matches!(error, ReadError::Rpc(RpcError::Node { error, .. }) if error.message == "header not found");
    let value = match client.read(&read).await {
        Err(error) if behind(&error) => {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            client.read(&read).await
        }
        answer => answer,
    };
    let value = value.unwrap_or_else(|error| panic!("{name}: {error}"));
    same_shape(client.http(), &nitro(name, 0));
    value
}

#[tokio::test]
#[ignore = "testnet"]
async fn arbsys_and_arbinfo_answer_the_chain_s_own_facts() {
    let client = reader();
    let head = client.block_number().await.unwrap();
    let height = check(&client, "arbBlockNumber", sys::arb_block_number()).await;
    assert!(height >= head, "Elysium's height, not HyperEVM's");
    assert_eq!(check(&client, "arbChainID", sys::arb_chain_id()).await, Network::Testnet.chain_id());
    assert_eq!(sys::arbos(check(&client, "arbOSVersion", sys::arb_os_version()).await), 61);
    let hash = check(&client, "arbBlockHash", sys::arb_block_hash(height - 1)).await;
    assert_eq!(client.block(BlockTag::Number(height - 1), false).await.unwrap().unwrap().hash, hash);
    assert_eq!(check(&client, "getStorageGasAvailable", sys::storage_gas_available()).await, U256::ZERO);
    assert!(check(&client, "isTopLevelCall", sys::is_top_level_call()).await);
    assert!(!check(&client, "wasMyCallersAddressAliased", sys::was_my_callers_address_aliased()).await);
    check(&client, "myCallersAddressWithoutAliasing", sys::my_callers_address_without_aliasing()).await;
    let alias = check(&client, "mapL1SenderContractAddressToL2Alias", sys::alias_of(MULTICALL3)).await;
    assert_eq!(alias, df_elysium_core::bridge::apply_alias(MULTICALL3), "ArbOS aliases as the crate does");
    let tree = check(&client, "sendMerkleTreeState", sys::send_merkle_tree_state()).await;
    assert!(tree.size > U256::from(747_000u64) && !tree.partials.is_empty(), "{tree:?}");
    let operator = address("0xa16a365ae8f34edadb8157849d4ace274bba9164");
    assert_eq!(check(&client, "getBalance", info::balance(operator)).await, client.balance(operator).await.unwrap());
    assert!(check(&client, "getCode", info::code(MULTICALL3)).await.len() > 1_000);
}

#[tokio::test]
#[ignore = "testnet"]
async fn the_gas_model_and_who_runs_the_chain_read_from_their_precompiles() {
    let client = reader();
    let prices = check(&client, "getPricesInWei", gas_info::prices_in_wei()).await;
    let floor = check(&client, "getMinimumGasPrice", gas_info::minimum_gas_price()).await;
    assert!(prices.per_arb_gas_total >= floor && floor == U256::from(100_000_000u64), "{prices:?}");
    check(&client, "getPricesInArbGas", gas_info::prices_in_arb_gas()).await;
    check(&client, "getGasAccountingParams", gas_info::gas_accounting_params()).await;
    assert!(check(&client, "getL1BaseFeeEstimate", gas_info::l1_base_fee_estimate()).await > U256::ZERO);
    check(&client, "getL1BaseFeeEstimateInertia", gas_info::l1_base_fee_estimate_inertia()).await;
    check(&client, "getL1RewardRate", gas_info::l1_reward_rate()).await;
    check(&client, "getL1RewardRecipient", gas_info::l1_reward_recipient()).await;
    check(&client, "getL1GasPriceEstimate", gas_info::l1_gas_price_estimate()).await;
    check(&client, "getCurrentTxL1GasFees", gas_info::current_tx_l1_gas_fees()).await;
    check(&client, "getGasBacklog", gas_info::gas_backlog()).await;
    check(&client, "getPricingInertia", gas_info::pricing_inertia()).await;
    check(&client, "getGasBacklogTolerance", gas_info::gas_backlog_tolerance()).await;
    check(&client, "getL1PricingSurplus", gas_info::l1_pricing_surplus()).await;
    check(&client, "getPerBatchGasCharge", gas_info::per_batch_gas_charge()).await;
    check(&client, "getAmortizedCostCapBips", gas_info::amortized_cost_cap_bips()).await;
    check(&client, "getL1FeesAvailable", gas_info::l1_fees_available()).await;
    assert_eq!(check(&client, "getMaxTxGasLimit", gas_info::max_tx_gas_limit()).await, U256::from(32_000_000u64));
    assert_eq!(check(&client, "getMaxBlockGasLimit", gas_info::max_block_gas_limit()).await, 32_000_000);
    assert!(!check(&client, "getGasPricingConstraints", gas_info::gas_pricing_constraints()).await.is_empty());
    assert!(!check(&client, "getMultiGasBaseFee", gas_info::multi_gas_base_fee()).await.is_empty());
    check(&client, "getLastL1PricingUpdateTime", gas_info::last_l1_pricing_update_time()).await;

    let posters = check(&client, "getBatchPosters", aggregator::batch_posters()).await;
    assert!(posters.contains(&aggregator::SEQUENCER), "{posters:?}");
    check(&client, "getDefaultAggregator", aggregator::default_aggregator()).await;
    check(&client, "getPreferredAggregator", aggregator::preferred_aggregator(posters[0])).await;
    check(&client, "getFeeCollector", aggregator::fee_collector(posters[0])).await;
    check(&client, "getTxBaseFee", aggregator::tx_base_fee(posters[0])).await;

    let owners = check(&client, "getAllChainOwners", owner::chain_owners()).await;
    assert!(check(&client, "isChainOwner", owner::is_chain_owner(owners[0])).await);
    check(&client, "getNetworkFeeAccount", owner::network_fee_account()).await;
    check(&client, "getInfraFeeAccount", owner::infra_fee_account()).await;
    check(&client, "getBrotliCompressionLevel", owner::brotli_compression_level()).await;
    check(&client, "getScheduledUpgrade", owner::scheduled_upgrade()).await;
    check(&client, "getCollectTips", owner::collect_tips()).await;
    check(&client, "getMaxStylusContractFragments", owner::max_stylus_contract_fragments()).await;
    check(&client, "isCalldataPriceIncreaseEnabled", owner::is_calldata_price_increase_enabled()).await;
    check(&client, "getAllNativeTokenOwners", owner::native_token_owners()).await;
    check(&client, "getAllTransactionFilterers", owner::transaction_filterers()).await;
    assert_eq!(check(&client, "getStats", statistics::stats()).await.classic_accounts, U256::ZERO, "born on Nitro");
}

#[tokio::test]
#[ignore = "testnet"]
async fn retryables_stylus_and_the_node_interface_answer_as_recorded() {
    let client = reader();
    assert_eq!(check(&client, "getLifetime", retryable::lifetime()).await, U256::from(604_800u64));
    check(&client, "getCurrentRedeemer", retryable::current_redeemer()).await;
    let gone = client.read(&retryable::timeout(df_elysium_core::B256::ZERO)).await.unwrap_err();
    assert!(retryable::no_ticket(&gone), "{gone}");

    assert_eq!(check(&client, "stylusVersion", wasm::stylus_version()).await, 3);
    check(&client, "inkPrice", wasm::ink_price()).await;
    check(&client, "maxStackDepth", wasm::max_stack_depth()).await;
    check(&client, "freePages", wasm::free_pages()).await;
    check(&client, "pageGas", wasm::page_gas()).await;
    check(&client, "pageLimit", wasm::page_limit()).await;
    check(&client, "expiryDays", wasm::expiry_days()).await;
    check(&client, "keepaliveDays", wasm::keepalive_days()).await;
    check(&client, "blockCacheSize", wasm::block_cache_size()).await;
    check(&client, "allCacheManagers", wasm::cache_managers()).await;

    assert_eq!(check(&client, "nitroGenesisBlock", node::nitro_genesis_block()).await, U256::ZERO);
    let head = client.block_number().await.unwrap();
    let parent = check(&client, "blockL1Num", node::block_l1_num(head - 10)).await;
    assert_eq!(parent, client.block(BlockTag::Number(head - 10), false).await.unwrap().unwrap().l1_block_number);
    let (first, last) = check(&client, "l2BlockRangeForL1", node::l2_block_range_for_l1(parent)).await;
    assert!(first <= head - 10 && head - 10 <= last, "{first}..={last}");
    // Batches lag thousands of blocks: a block 100,000 back is posted and confirmed on HyperEVM.
    let old = client.block(BlockTag::Number(head - 100_000), false).await.unwrap().unwrap();
    assert!(client.read(&node::find_batch_containing_block(old.number)).await.unwrap() > 3_000);
    assert!(client.read(&node::l1_confirmations(old.hash)).await.unwrap() > 0, "posted to HyperEVM");
    let (gas_for_l1, base, l1_base) = client
        .read(&node::gas_estimate_l1_component(&df_elysium_core::transfer::hype_wei(MULTICALL3, U256::ONE)))
        .await
        .unwrap();
    same_shape(client.http(), &nitro("gasEstimateL1Component", 0));
    assert!(gas_for_l1 > 0 && base > U256::ZERO && l1_base > U256::ZERO);
}
