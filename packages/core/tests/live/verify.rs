//! Conduit's Sourcify, read only: the operator's escrow on Elysium testnet is verified exactly, as
//! recorded (`fixtures/sourcify/status_exact.json`), and an address with no code reads unverified.

use df_elysium_core::Network;
use df_elysium_core::client::native::Native;
use df_elysium_core::deploy::verify::{self, Match};

use crate::support::address;

#[tokio::test]
#[ignore = "testnet"]
async fn the_escrow_is_verified_exactly_and_an_empty_address_is_not() {
    let escrow = address("0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB");
    let status = verify::status(&Native::new(), Network::Testnet, escrow).await.unwrap();
    assert_eq!(
        (status.matched, status.creation, status.runtime),
        (Some(Match::Exact), Some(Match::Exact), Some(Match::Exact))
    );
    assert_eq!(status.verified_at.as_deref(), Some("2026-10-25T08:38:44Z"));
    let nobody =
        verify::status(&Native::new(), Network::Testnet, address("0x0000000000000000000000000000000000000001")).await;
    assert!(!nobody.unwrap().is_verified());
}
