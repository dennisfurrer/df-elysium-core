//! [`Signer`]: whoever signs a client's transactions, handed the hash a transaction commits to. A
//! key held here is df-hyperliquid-core's [`LocalKey`]; one held elsewhere (a KMS, a hardware
//! wallet, a remote signer) implements the trait. Every signature is recovered and must match the
//! signer before anything is sent.

use std::future::Future;

#[cfg(feature = "sign")]
pub use df_hyperliquid_core::account::{LocalKey, recover, verify};
pub use df_hyperliquid_core::account::{SignError, Signature};

use crate::B256;
use crate::utils::Address;

/// Whoever signs for an address. No `Send` bound: the browser is single-threaded.
pub trait Signer {
    fn address(&self) -> Address;

    /// A signature over `hash`, `v` as 27 or 28.
    fn sign_hash(&self, hash: &B256) -> impl Future<Output = Result<Signature, SignError>>;
}

/// RFC 6979, low-s: the same hash always signs the same.
#[cfg(feature = "sign")]
impl Signer for LocalKey {
    fn address(&self) -> Address {
        LocalKey::address(self)
    }

    fn sign_hash(&self, hash: &B256) -> impl Future<Output = Result<Signature, SignError>> {
        std::future::ready(self.sign_digest(&hash.0))
    }
}

impl<S: Signer> Signer for &S {
    fn address(&self) -> Address {
        (**self).address()
    }

    fn sign_hash(&self, hash: &B256) -> impl Future<Output = Result<Signature, SignError>> {
        (**self).sign_hash(hash)
    }
}
