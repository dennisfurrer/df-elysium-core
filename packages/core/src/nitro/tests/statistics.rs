//! ArbStatistics against Elysium testnet's answer (`elysium_testnet.json`): a chain born on Nitro
//! carries nothing over from Classic.

use crate::nitro::support::{decoded, number, sent};

use super::*;

#[test]
fn the_height_and_nothing_from_classic() {
    let stats = sent(&stats(), "ArbStatistics", "getStats", 0);
    let said = decoded("ArbStatistics", "getStats", 0);
    assert_eq!(stats.block_number, number(&said[0]));
    assert_eq!(
        [
            stats.classic_accounts,
            stats.classic_storage_sum,
            stats.classic_gas_sum,
            stats.classic_transactions,
            stats.classic_contracts
        ],
        [U256::ZERO; 5]
    );
}
