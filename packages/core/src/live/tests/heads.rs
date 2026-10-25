//! New heads, each block once, whatever order the polls answer in.

use super::*;

#[test]
fn each_block_is_new_once_and_a_lagging_answer_is_nothing_new() {
    let mut heads = Heads::new();
    assert_eq!(heads.seen(100), Some(100..=100), "the first head alone");
    assert_eq!(heads.seen(100), None, "it has not moved");
    assert_eq!(heads.seen(103), Some(101..=103));
    assert_eq!(heads.seen(102), None, "a node behind the last one asked");
    assert_eq!(heads.last(), Some(103));
    assert_eq!(Heads::after(5).seen(6), Some(6..=6));
}
