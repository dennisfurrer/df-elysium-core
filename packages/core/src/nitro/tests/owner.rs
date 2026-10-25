//! ArbOwnerPublic's typed reads against Elysium testnet's answers (`elysium_testnet.json`, cast's
//! decoding beside them).

use crate::nitro::support::{decoded, recorded, sent, word};
use crate::test_support::text;

use super::*;

#[test]
fn who_runs_the_chain_and_where_its_fees_go_read_as_answered() {
    let owners = sent(&chain_owners(), "ArbOwnerPublic", "getAllChainOwners", 0);
    let said: Vec<String> = decoded("ArbOwnerPublic", "getAllChainOwners", 0)[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| text(one).to_lowercase())
        .collect();
    assert_eq!(owners.iter().map(|owner| owner.to_hex()).collect::<Vec<_>>(), said);
    let asked = word(&recorded("ArbOwnerPublic", "isChainOwner", 0), 0).to_address().unwrap();
    assert!(!sent(&is_chain_owner(asked), "ArbOwnerPublic", "isChainOwner", 0), "the operator is no owner");
    let network = sent(&network_fee_account(), "ArbOwnerPublic", "getNetworkFeeAccount", 0);
    let infra = sent(&infra_fee_account(), "ArbOwnerPublic", "getInfraFeeAccount", 0);
    assert_eq!(network.to_hex(), text(&decoded("ArbOwnerPublic", "getNetworkFeeAccount", 0)[0]).to_lowercase());
    assert_eq!(network, infra, "one account takes both on Elysium testnet");
    assert_eq!(sent(&brotli_compression_level(), "ArbOwnerPublic", "getBrotliCompressionLevel", 0), 1);
}

#[test]
fn nothing_is_scheduled_and_tips_are_collected() {
    let upgrade = sent(&scheduled_upgrade(), "ArbOwnerPublic", "getScheduledUpgrade", 0);
    assert_eq!(upgrade, ScheduledUpgrade { arbos_version: 0, at: 0 });
    assert!(!upgrade.is_scheduled());
    assert!(ScheduledUpgrade { arbos_version: 62, at: 1 }.is_scheduled());
    assert!(sent(&collect_tips(), "ArbOwnerPublic", "getCollectTips", 0));
    assert_eq!(sent(&max_stylus_contract_fragments(), "ArbOwnerPublic", "getMaxStylusContractFragments", 0), 4);
    assert!(!sent(&is_calldata_price_increase_enabled(), "ArbOwnerPublic", "isCalldataPriceIncreaseEnabled", 0));
    assert!(sent(&native_token_owners(), "ArbOwnerPublic", "getAllNativeTokenOwners", 0).is_empty());
    assert!(sent(&transaction_filterers(), "ArbOwnerPublic", "getAllTransactionFilterers", 0).is_empty());
}
