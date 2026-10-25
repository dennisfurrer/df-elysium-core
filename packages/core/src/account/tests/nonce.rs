//! The kept nonce: read once, moved past each send, forgotten on a failure, and never moved back
//! by a stale read.

use super::*;

#[test]
fn a_nonce_is_kept_past_each_send_and_forgotten_on_a_failure() {
    let nonces = Nonces::new();
    assert_eq!(nonces.kept(), None);
    assert_eq!(nonces.read(7), 7);
    nonces.sent(7);
    assert_eq!(nonces.kept(), Some(8));
    assert_eq!(nonces.read(5), 8, "a stale count never moves it back");
    nonces.sent(3);
    assert_eq!(nonces.kept(), Some(8), "an old send never moves it back either");
    nonces.forget();
    assert_eq!((nonces.kept(), nonces.read(9)), (None, 9));
    nonces.sent(u64::MAX);
    assert_eq!(nonces.kept(), Some(u64::MAX), "saturates");
}
