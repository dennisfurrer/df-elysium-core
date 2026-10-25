//! Everything that names Elysium testnet: its chain, nodes, explorer and parent, each as the chain
//! and its explorer gave them on 2026-10-25 (`eth_chainId` answers `0x185d9`, the explorer's own
//! config names Conduit's RPC and Sourcify).

use super::*;

#[test]
fn testnet_names_its_chain_nodes_explorer_and_parent() {
    let testnet = Network::Testnet;
    assert_eq!(
        (testnet.chain_id(), TESTNET_CHAIN_ID, format!("{:#x}", testnet.chain_id())),
        (99_801, 99_801, "0x185d9".to_owned())
    );
    assert_eq!(testnet.rpc_url(), "https://testnet-rpc.elysium.kinetiq.xyz");
    assert_eq!(testnet.conduit_rpc_url(), "https://rpc-elysium-testnet.t.conduit.xyz");
    assert_eq!(testnet.log_span(), 2_000, "the public node refuses an unfiltered eth_getLogs past it");
    assert_eq!(testnet.verifier_url(), "https://contracts.conduit.xyz");
    assert_eq!(testnet.faucet_url(), "https://elysium.kinetiq.xyz/testnet-faucet");
    assert_eq!(testnet.bridge_url(), "https://elysium.kinetiq.xyz/testnet-bridge");
    let hash: B256 = "0xc10215c38b8bacde3e39632d81569f3e230ee6940b126ce99f4891fdb5fd72a0".parse().unwrap();
    assert_eq!(
        testnet.tx_url(hash),
        "https://test-explorer.elysium.kinetiq.xyz/tx/0xc10215c38b8bacde3e39632d81569f3e230ee6940b126ce99f4891fdb5fd72a0"
    );
    assert_eq!(testnet.address_url(Address::ZERO), format!("{}/address/{}", testnet.explorer_url(), Address::ZERO));
    assert_eq!(testnet.block_url(15_618_178), "https://test-explorer.elysium.kinetiq.xyz/block/15618178");
    // It settles to HyperEVM testnet, where its rollup lives.
    assert_eq!(testnet.hyperliquid(), df_hyperliquid_core::Network::Testnet);
    assert_eq!(testnet.parent_chain_id(), 998);
    assert_eq!(testnet.contracts().rollup.to_hex(), "0xebf08e34941fd93a0fc1cd89dbbf7447267039df");
    assert!(!testnet.is_mainnet());
    assert_eq!(Network::of_chain(99_801), Some(Network::Testnet));
    assert_eq!(Network::of_chain(998), None, "HyperEVM is the parent, not an Elysium network");
}

#[test]
fn a_network_reads_from_what_a_person_types_and_testnet_is_the_default() {
    for typed in ["testnet", " Testnet ", "TEST"] {
        assert_eq!(typed.parse::<Network>(), Ok(Network::Testnet), "{typed:?}");
    }
    assert_eq!("mainnet".parse::<Network>(), Err(UnknownNetwork("mainnet".to_owned())), "not published yet");
    assert_eq!(Network::ALL.map(|network| network.to_string()), ["testnet"]);
    assert_eq!(Network::default(), Network::Testnet);
}
