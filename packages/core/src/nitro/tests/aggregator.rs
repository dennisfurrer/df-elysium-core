//! ArbAggregator's typed reads against Elysium testnet's answers (`elysium_testnet.json`, cast's
//! decoding beside them).

use crate::nitro::support::{decoded, recorded, sent, word};
use crate::test_support::text;

use super::*;

#[test]
fn the_sequencer_and_one_batch_poster_post_and_collect_their_own_fees() {
    let posters = sent(&batch_posters(), "ArbAggregator", "getBatchPosters", 0);
    let said: Vec<String> = decoded("ArbAggregator", "getBatchPosters", 0)[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| text(one).to_lowercase())
        .collect();
    assert_eq!(posters.iter().map(|poster| poster.to_hex()).collect::<Vec<_>>(), said);
    assert_eq!(posters[0], SEQUENCER, "the sequencer is one of them");
    assert_eq!(SEQUENCER.bytes()[11..], *b"sequencer", "\"sequencer\" in ASCII");
    for (nth, poster) in posters.iter().enumerate() {
        let asked = word(&recorded("ArbAggregator", "getFeeCollector", nth), 0).to_address().unwrap();
        assert_eq!(asked, *poster);
        assert_eq!(sent(&fee_collector(*poster), "ArbAggregator", "getFeeCollector", nth), *poster, "paid to itself");
    }
    assert_eq!(sent(&default_aggregator(), "ArbAggregator", "getDefaultAggregator", 0), SEQUENCER);
    let asked = word(&recorded("ArbAggregator", "getPreferredAggregator", 0), 0).to_address().unwrap();
    let (preferred, is_default) = sent(&preferred_aggregator(asked), "ArbAggregator", "getPreferredAggregator", 0);
    assert_eq!((preferred, is_default), (SEQUENCER, true));
    assert_eq!(sent(&tx_base_fee(SEQUENCER), "ArbAggregator", "getTxBaseFee", 0), U256::ZERO, "deprecated");
}
