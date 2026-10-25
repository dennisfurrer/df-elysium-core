//! A followed filter: windows inside the span, behind the confirmations, never missing or
//! repeating a block across polls.

use crate::client::{BlockTag, LogFilter};
use crate::utils::Address;

use super::*;

#[test]
fn a_cursor_reads_each_confirmed_block_once_in_windows_inside_the_span() {
    let filter = LogFilter { address: vec![Address::ZERO], ..LogFilter::default() };
    let mut cursor = LogCursor::new(filter, 1_000, 2_000, 10);
    assert_eq!(cursor.window(1_005), None, "nothing confirmed yet");
    let mut read = Vec::new();
    for head in [1_010, 1_010, 4_500, 4_500, 4_500, 4_501] {
        if let Some(window) = cursor.window(head) {
            let (Some(BlockTag::Number(from)), Some(BlockTag::Number(to))) = (window.from_block, window.to_block)
            else {
                panic!("{window:?}")
            };
            assert!(to - from <= 2_000 && to <= head - 10, "{from}..{to} at {head}");
            assert_eq!(window.address, vec![Address::ZERO], "the filter kept");
            read.push((from, to));
            cursor.read(&window);
        }
    }
    assert_eq!(read, [(1_000, 1_000), (1_001, 3_001), (3_002, 4_490), (4_491, 4_491)]);
    assert_eq!(cursor.next(), 4_492);
    assert_eq!(cursor.window(9), None, "a head under the confirmations");
}
