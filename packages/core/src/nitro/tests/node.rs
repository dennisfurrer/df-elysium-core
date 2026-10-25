//! The NodeInterface against Elysium testnet's answers (`elysium_testnet.json`, cast's decoding
//! beside them): gas split between Elysium and HyperEVM for a HYPE transfer and a Multicall3 call,
//! a withdrawal's outbox proof, and the map from blocks to batches and parent blocks. Calldata
//! against viem's `encodeFunctionData` and answers against `decodeFunctionResult`
//! (`arbitrum_sdk.json`).

use crate::client::Call;
use crate::nitro::support::{
    arg, decoded, number, recorded, sdk_answer, sdk_arg, sdk_calldata, sdk_data, sdk_one, sent, word,
};
use crate::test_support::{bytes, text};
use crate::utils::hex;

use super::*;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// The recorded estimates: a HYPE transfer of one wei, and Multicall3's `getBlockNumber()`.
#[test]
fn a_gas_estimate_splits_elysium_s_execution_from_hyperevm_s_data() {
    let sender = address("0xA16A365aE8f34EDaDB8157849d4ACe274bBa9164");
    let transfer =
        Call { to: Some(address("0x8499e50474b6e8c988b232f52583439abf37a8a1")), value: U256::ONE, data: Vec::new() };
    let estimate = sent(&gas_estimate_components(Some(sender), &transfer), "NodeInterface", "gasEstimateComponents", 0);
    let said = decoded("NodeInterface", "gasEstimateComponents", 0);
    assert_eq!(
        [U256::from(estimate.gas), U256::from(estimate.gas_for_l1), estimate.base_fee, estimate.l1_base_fee].to_vec(),
        said.iter().map(number).collect::<Vec<_>>()
    );
    assert!(estimate.gas > 21_000, "more than a plain transfer's intrinsic gas: {}", estimate.gas);
    assert_eq!(estimate.gas_for_l2(), 21_165);
    let (l1, base, l1_base) = sent(
        &gas_estimate_l1_component(&transfer).from(sender).with_value(U256::ONE),
        "NodeInterface",
        "gasEstimateL1Component",
        0,
    );
    assert_eq!((l1, l1_base), (estimate.gas_for_l1, estimate.l1_base_fee));
    assert_eq!(base, number(&decoded("NodeInterface", "gasEstimateL1Component", 0)[1]));
    let call = recorded("NodeInterface", "gasEstimateComponents", 1);
    let target = word(&call, 0).to_address().unwrap();
    let data = bytes(&call["params"][0]["data"])[4 + 4 * 32..4 + 4 * 32 + 4].to_vec();
    let multicall = Call { to: Some(target), value: U256::ZERO, data };
    let estimate =
        sent(&gas_estimate_components(Some(sender), &multicall), "NodeInterface", "gasEstimateComponents", 1);
    assert_eq!(estimate.gas, 21_508);
}

#[test]
fn a_creation_is_estimated_with_no_target_and_the_flag_set() {
    let creations: Vec<_> = sdk_calldata("NodeInterface", "gasEstimateComponents");
    let creation = creations.iter().find(|entry| sdk_arg(entry, 1) == "true").unwrap();
    let call = Call { to: None, value: U256::ZERO, data: hex::decode(sdk_arg(creation, 2)).unwrap() };
    let read = gas_estimate_components(None, &call);
    assert_eq!(hex::encode_prefixed(&read.call.data), sdk_data(creation));
    assert_eq!((read.call.from, read.call.value), (None, None), "no sender, no value");
    let transfer = creations.iter().find(|entry| sdk_arg(entry, 1) == "false").unwrap();
    let call = Call { to: Some(address(sdk_arg(transfer, 0))), value: U256::ZERO, data: Vec::new() };
    assert_eq!(hex::encode_prefixed(&gas_estimate_components(None, &call).call.data), sdk_data(transfer));
    let l1 = sdk_one("NodeInterface", "gasEstimateL1Component");
    let call =
        Call { to: Some(address(sdk_arg(&l1, 0))), value: U256::ZERO, data: hex::decode(sdk_arg(&l1, 2)).unwrap() };
    assert_eq!(hex::encode_prefixed(&gas_estimate_l1_component(&call).call.data), sdk_data(&l1));
}

/// The proof of the recorded withdrawal: its leaf and the root of the tree it is confirmed in.
#[test]
fn an_outbox_proof_reads_its_leaf_root_and_path() {
    let asked = recorded("NodeInterface", "constructOutboxProof", 0);
    let (size, leaf) = (arg(&asked, 0).to_u64().unwrap(), arg(&asked, 1).to_u64().unwrap());
    assert_eq!((size, leaf), (746_936, 746_935), "the last leaf of the confirmed tree");
    let proof = sent(&construct_outbox_proof(size, leaf), "NodeInterface", "constructOutboxProof", 0);
    let said = decoded("NodeInterface", "constructOutboxProof", 0);
    assert_eq!(proof.send.to_hex(), text(&said[0]));
    assert_eq!(proof.root.to_hex(), text(&said[1]));
    let path: Vec<&str> = said[2].as_array().unwrap().iter().map(text).collect();
    assert_eq!(proof.proof.iter().map(B256::to_hex).collect::<Vec<_>>(), path);
    assert_eq!(read_outbox_proof(&crate::nitro::support::answered(&asked)), Ok(proof));
    for (nth, entry) in sdk_calldata("NodeInterface", "constructOutboxProof").iter().enumerate() {
        let (size, leaf) = (sdk_arg(entry, 0).parse().unwrap(), sdk_arg(entry, 1).parse().unwrap());
        assert_eq!(hex::encode_prefixed(&construct_outbox_proof(size, leaf).call.data), sdk_data(entry), "#{nth}");
    }
    for nth in 0..2 {
        let answer = sdk_answer("NodeInterface", "constructOutboxProof", nth);
        let proof = read_outbox_proof(&bytes(&answer["data"])).unwrap();
        let said = answer["decoded"].as_array().unwrap();
        assert_eq!(proof.send.to_hex(), text(&said[0]), "#{nth}");
        assert_eq!(proof.root.to_hex(), text(&said[1]), "#{nth}");
        assert_eq!(proof.proof.len(), said[2].as_array().unwrap().len(), "#{nth}");
    }
}

#[test]
fn blocks_map_to_parent_blocks_batches_and_confirmations() {
    assert_eq!(sent(&nitro_genesis_block(), "NodeInterface", "nitroGenesisBlock", 0), U256::ZERO);
    let asked = arg(&recorded("NodeInterface", "blockL1Num", 0), 0).to_u64().unwrap();
    let parent = sent(&block_l1_num(asked), "NodeInterface", "blockL1Num", 0);
    assert_eq!(U256::from(parent), number(&decoded("NodeInterface", "blockL1Num", 0)[0]));
    let range = sent(&l2_block_range_for_l1(parent), "NodeInterface", "l2BlockRangeForL1", 0);
    assert!(range.0 <= asked && asked <= range.1, "{asked} is in {range:?}");
    let posted = arg(&recorded("NodeInterface", "findBatchContainingBlock", 1), 0).to_u64().unwrap();
    assert_eq!(sent(&find_batch_containing_block(posted), "NodeInterface", "findBatchContainingBlock", 1), 3_427);
    let unposted = recorded("NodeInterface", "findBatchContainingBlock", 0);
    let read = find_batch_containing_block(arg(&unposted, 0).to_u64().unwrap());
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&unposted["params"][0]["data"]));
    assert!(text(&unposted["answer"]).contains("execution reverted"), "not yet in a batch");
    let posted_hash = word(&recorded("NodeInterface", "getL1Confirmations", 0), 0);
    assert_eq!(sent(&l1_confirmations(posted_hash), "NodeInterface", "getL1Confirmations", 0), 12_446);
    let fresh = word(&recorded("NodeInterface", "getL1Confirmations", 1), 0);
    assert_eq!(sent(&l1_confirmations(fresh), "NodeInterface", "getL1Confirmations", 1), 0, "not yet posted");
    for (name, data) in [
        ("findBatchContainingBlock", find_batch_containing_block(15_620_000).call.data),
        ("getL1Confirmations", l1_confirmations(B256([0xab; 32])).call.data),
        ("nitroGenesisBlock", nitro_genesis_block().call.data),
        ("blockL1Num", block_l1_num(15_620_000).call.data),
        ("l2BlockRangeForL1", l2_block_range_for_l1(67_847_915).call.data),
    ] {
        assert_eq!(hex::encode_prefixed(&data), sdk_data(&sdk_one("NodeInterface", name)), "{name}");
    }
}

#[test]
fn a_retryable_is_estimated_through_its_own_call() {
    let entry = sdk_one("NodeInterface", "estimateRetryableTicket");
    let ticket = RetryableEstimate {
        sender: address(sdk_arg(&entry, 0)),
        deposit: sdk_arg(&entry, 1).parse().unwrap(),
        to: address(sdk_arg(&entry, 2)),
        l2_call_value: sdk_arg(&entry, 3).parse().unwrap(),
        excess_fee_refund_address: address(sdk_arg(&entry, 4)),
        call_value_refund_address: address(sdk_arg(&entry, 5)),
        data: hex::decode(sdk_arg(&entry, 6)).unwrap(),
    };
    let call = estimate_retryable_ticket(&ticket);
    assert_eq!((call.to, call.value), (Some(NODE_INTERFACE), U256::ZERO));
    assert_eq!(hex::encode_prefixed(&call.data), sdk_data(&entry));
}

/// The live estimate viem decoded.
#[test]
fn a_live_estimate_reads_as_viem_read_it() {
    let answer = sdk_answer("NodeInterface", "gasEstimateComponents", 0);
    let call = Call::default();
    let estimate = gas_estimate_components(None, &call).decode(&bytes(&answer["data"])).unwrap();
    let said: Vec<&str> = answer["decoded"].as_array().unwrap().iter().map(text).collect();
    assert_eq!(
        [
            estimate.gas.to_string(),
            estimate.gas_for_l1.to_string(),
            estimate.base_fee.to_string(),
            estimate.l1_base_fee.to_string()
        ]
        .to_vec(),
        said
    );
}
