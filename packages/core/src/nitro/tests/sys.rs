//! ArbSys's reads as Elysium testnet answered them (`elysium_testnet.json`, cast's decoding beside
//! them), its calls against viem's `encodeFunctionData` (`arbitrum_sdk.json`), and its two events
//! against live logs and viem's `decodeEventLog`.

use crate::bridge::alias::apply_alias;
use crate::client::Log;
use crate::nitro::support::{
    arg, decoded, number, recorded, sdk_arg, sdk_calldata, sdk_data, sdk_event, sdk_one, sent, word,
};
use crate::test_support::text;
use crate::utils::hex;

use super::*;

#[test]
fn the_chain_s_own_facts_read_as_the_node_answered_them() {
    let height = sent(&arb_block_number(), "ArbSys", "arbBlockNumber", 0);
    assert_eq!(U256::from(height), number(&decoded("ArbSys", "arbBlockNumber", 0)[0]));
    let asked = recorded("ArbSys", "arbBlockHash", 0);
    let hash = sent(&arb_block_hash(arg(&asked, 0).to_u64().unwrap()), "ArbSys", "arbBlockHash", 0);
    assert_eq!(hash.to_hex(), text(&decoded("ArbSys", "arbBlockHash", 0)[0]));
    assert_eq!(sent(&arb_chain_id(), "ArbSys", "arbChainID", 0), 99_801);
    assert_eq!(sent(&arb_os_version(), "ArbSys", "arbOSVersion", 0), 116);
    assert_eq!(sent(&storage_gas_available(), "ArbSys", "getStorageGasAvailable", 0), U256::ZERO, "always zero");
    assert!(sent(&is_top_level_call(), "ArbSys", "isTopLevelCall", 0));
    assert!(!sent(&was_my_callers_address_aliased(), "ArbSys", "wasMyCallersAddressAliased", 0));
    assert_eq!(
        sent(&my_callers_address_without_aliasing(), "ArbSys", "myCallersAddressWithoutAliasing", 0),
        Address::ZERO
    );
}

/// ArbOS's alias of a parent-chain sender, and this crate's, agree.
#[test]
fn arbos_aliases_a_sender_as_the_bridge_does() {
    for nth in 0..2 {
        let asked = recorded("ArbSys", "mapL1SenderContractAddressToL2Alias", nth);
        let sender = word(&asked, 0).to_address().unwrap();
        let alias = sent(&alias_of(sender), "ArbSys", "mapL1SenderContractAddressToL2Alias", nth);
        assert_eq!(
            alias.to_hex(),
            text(&decoded("ArbSys", "mapL1SenderContractAddressToL2Alias", nth)[0]).to_lowercase()
        );
        assert_eq!(alias, apply_alias(sender), "{sender}");
    }
}

/// The outbox's tree as ArbOS keeps it: asked from the zero address, which an `eth_call` with no
/// sender is sent from anyway.
#[test]
fn the_outbox_s_tree_reads_its_size_root_and_partials() {
    let read = send_merkle_tree_state();
    assert_eq!(read.call.from, Some(Address::ZERO));
    let entry = recorded("ArbSys", "sendMerkleTreeState", 0);
    assert_eq!(hex::encode_prefixed(&read.call.data), text(&entry["params"][0]["data"]));
    let state = read.decode(&crate::nitro::support::answered(&entry)).unwrap();
    let said = decoded("ArbSys", "sendMerkleTreeState", 0);
    assert_eq!(state.size, number(&said[0]));
    assert_eq!(state.root.to_hex(), text(&said[1]));
    let partials: Vec<&str> = said[2].as_array().unwrap().iter().map(text).collect();
    assert_eq!(state.partials.iter().map(B256::to_hex).collect::<Vec<_>>(), partials);
    assert_eq!(state.partials.len(), 20, "one per level of a tree of {} leaves", state.size);
}

#[test]
fn withdrawals_and_sends_to_the_parent_chain_are_viem_s_calldata() {
    let withdraw = sdk_one("ArbSys", "withdrawEth");
    let destination: Address = sdk_arg(&withdraw, 0).parse().unwrap();
    let call = withdraw_eth(destination, U256::from(5u8));
    assert_eq!((call.to, call.value), (Some(ARB_SYS), U256::from(5u8)), "to ArbSys, the HYPE as value");
    assert_eq!(hex::encode_prefixed(&call.data), sdk_data(&withdraw));
    let sends = sdk_calldata("ArbSys", "sendTxToL1");
    assert_eq!(sends.len(), 2, "with and without data");
    for send in &sends {
        let destination: Address = sdk_arg(send, 0).parse().unwrap();
        let data = hex::decode(sdk_arg(send, 1)).unwrap();
        let call = send_tx_to_l1(destination, &data, U256::ZERO);
        assert_eq!(hex::encode_prefixed(&call.data), sdk_data(send), "{} bytes of data", data.len());
    }
    let hash = sdk_one("ArbSys", "arbBlockHash");
    let number: u64 = sdk_arg(&hash, 0).parse().unwrap();
    assert_eq!(hex::encode_prefixed(&arb_block_hash(number).call.data), sdk_data(&hash));
    for (name, read) in [
        ("arbBlockNumber", arb_block_number().call),
        ("arbChainID", arb_chain_id().call),
        ("arbOSVersion", arb_os_version().call),
        ("isTopLevelCall", is_top_level_call().call),
        ("wasMyCallersAddressAliased", was_my_callers_address_aliased().call),
        ("myCallersAddressWithoutAliasing", my_callers_address_without_aliasing().call),
        ("sendMerkleTreeState", send_merkle_tree_state().call),
    ] {
        assert_eq!(hex::encode_prefixed(&read.data), sdk_data(&sdk_one("ArbSys", name)), "{name}");
        assert_eq!(read.to, Some(ARB_SYS), "{name}");
    }
}

fn log(event: &serde_json::Value) -> Log {
    serde_json::from_value(event["log"].clone()).unwrap()
}

/// A live `L2ToL1Tx` and a live `SendMerkleUpdate` from Elysium, as viem decoded them.
#[test]
fn a_send_logs_its_leaf_and_the_tree_s_update_under_their_own_topics() {
    let send = sdk_event("ArbSys", "L2ToL1Tx");
    assert_eq!(L2_TO_L1_TX.topic().to_hex(), text(&send["topic"]));
    assert_eq!(L2_TO_L1_TX.name(), "L2ToL1Tx");
    let tokens = L2_TO_L1_TX.decode(&log(&send)).unwrap();
    let said = &send["decoded"];
    assert_eq!(tokens[0].as_address().unwrap().to_hex(), text(&said["caller"]));
    assert_eq!(tokens[1].as_address().unwrap().to_hex(), text(&said["destination"]));
    assert_eq!(tokens[2].as_u256().unwrap().to_string(), text(&said["hash"]));
    assert_eq!(tokens[3].as_u256().unwrap().to_string(), text(&said["position"]));
    assert_eq!(tokens[7].as_u256().unwrap().to_string(), text(&said["callvalue"]));
    assert_eq!(hex::encode_prefixed(tokens[8].as_bytes().unwrap()), text(&said["data"]));

    let update = sdk_event("ArbSys", "SendMerkleUpdate");
    assert_eq!(SEND_MERKLE_UPDATE.topic().to_hex(), text(&update["topic"]));
    assert_eq!(
        SEND_MERKLE_UPDATE.topic().to_hex(),
        "0xe9e13da364699fb5b0496ff5a0fc70760ad5836e93ba96568a4e42b9914a8b95"
    );
    let tokens = SEND_MERKLE_UPDATE.decode(&log(&update)).unwrap();
    assert_eq!(tokens[0].as_u256(), Some(U256::ZERO), "reserved");
    assert_eq!(hex::encode_prefixed(tokens[1].as_bytes().unwrap()), text(&update["decoded"]["hash"]));
    assert_eq!(tokens[2].as_u256().unwrap().to_string(), text(&update["decoded"]["position"]));
    assert!(L2_TO_L1_TX.decode(&log(&update)).is_err(), "one event is not the other");
}
