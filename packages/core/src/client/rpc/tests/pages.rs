//! Windows of a log scan against the public node's cap, `toBlock - fromBlock <= 2000`: the
//! 4999-block scan it refused (`elysium_testnet.json`) goes out as three it takes.

use crate::utils::Address;

use super::*;

#[test]
fn a_scan_is_cut_into_windows_the_node_takes_none_missed_none_repeated() {
    // The refused scan: 0xee66f2 to 0xee7a79.
    let windows: Vec<(u64, u64)> = pages(0xee66f2, 0xee7a79, 2_000).collect();
    assert_eq!(
        windows,
        [(0xee66f2, 0xee66f2 + 2_000), (0xee66f2 + 2_001, 0xee66f2 + 4_001), (0xee66f2 + 4_002, 0xee7a79)]
    );
    assert!(windows.iter().all(|(from, to)| to - from <= 2_000));
    assert!(windows.windows(2).all(|pair| pair[1].0 == pair[0].1 + 1), "each starts where the last ended");
    assert_eq!(pages(5, 5, 2_000).collect::<Vec<_>>(), [(5, 5)], "one block");
    assert_eq!(pages(6, 5, 2_000).count(), 0, "an empty range");
    assert_eq!(pages(0, 4, 0).collect::<Vec<_>>(), [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)]);
    assert_eq!(pages(u64::MAX - 1, u64::MAX, 2_000).collect::<Vec<_>>(), [(u64::MAX - 1, u64::MAX)], "no overflow");
}

#[test]
fn a_filter_that_names_an_address_or_a_topic_is_narrow_and_a_window_keeps_it() {
    let wide = LogFilter { from_block: Some(BlockTag::Latest), ..LogFilter::default() };
    assert!(!is_narrow(&wide));
    let by_address = LogFilter { address: vec![Address::ZERO], ..wide.clone() };
    assert!(is_narrow(&by_address));
    assert!(!is_narrow(&LogFilter { topics: vec![None, None], ..wide.clone() }), "only wildcards");
    assert!(is_narrow(&LogFilter { topics: vec![None, Some(Vec::new())], ..wide.clone() }));
    let narrowed = window(&by_address, 10, 20);
    assert_eq!((narrowed.from_block, narrowed.to_block), (Some(BlockTag::Number(10)), Some(BlockTag::Number(20))));
    assert_eq!((narrowed.address, narrowed.block_hash), (vec![Address::ZERO], None));
}
