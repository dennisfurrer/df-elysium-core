//! The SDK, wired for the page: a client on Elysium and one on HyperEVM through the browser's
//! `fetch`, the account that signs (a burner key held in the page, or the browser's wallet), one
//! send that works for either, and waiting for a receipt on a timer.

use std::future::Future;
use std::sync::Arc;

use df_elysium_core::account::{LocalKey, SignError, Signature, Signer};
use df_elysium_core::client::web::Web;
use df_elysium_core::client::{Call, Client, Receipt};
use df_elysium_core::{Address, B256, Network};

use crate::wallet;

/// Elysium testnet: the only network Kinetiq has published.
pub const NETWORK: Network = Network::Testnet;

/// HYPE's system address on HyperEVM: it holds HYPE, so a simulation from it never runs short.
pub const HYPE_SYSTEM: Address = match Address::parse_const("0x2222222222222222222222222222222222222222") {
    Some(address) => address,
    None => Address::ZERO,
};

/// The chain a transaction goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Elysium,
    HyperEvm,
}

impl Chain {
    pub const fn name(self) -> &'static str {
        match self {
            Chain::Elysium => "Elysium",
            Chain::HyperEvm => "HyperEVM",
        }
    }

    pub const fn id(self) -> u64 {
        match self {
            Chain::Elysium => NETWORK.chain_id(),
            Chain::HyperEvm => NETWORK.parent_chain_id(),
        }
    }

    /// A transaction on the chain's explorer, when it has one this page links to.
    pub fn tx_url(self, hash: B256) -> Option<String> {
        match self {
            Chain::Elysium => Some(NETWORK.tx_url(hash)),
            Chain::HyperEvm => None,
        }
    }

    /// A client that reads this chain.
    pub fn reader(self) -> Client<Web> {
        match self {
            Chain::Elysium => Client::reader(Web, NETWORK),
            Chain::HyperEvm => Client::parent(Web, NETWORK, ()),
        }
    }
}

/// A key held in the page, as the SDK's [`Signer`].
#[derive(Clone)]
pub struct Burner(pub Arc<LocalKey>);

impl Signer for Burner {
    fn address(&self) -> Address {
        self.0.address()
    }

    fn sign_hash(&self, hash: &B256) -> impl Future<Output = Result<Signature, SignError>> {
        std::future::ready(self.0.sign_digest(&hash.0))
    }
}

/// Who signs: a burner key, or the browser's wallet at an address.
#[derive(Clone)]
pub enum Account {
    Burner(Burner),
    Wallet(Address),
}

impl Account {
    pub fn address(&self) -> Address {
        match self {
            Account::Burner(key) => key.address(),
            Account::Wallet(address) => *address,
        }
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Account::Burner(_) => "burner key",
            Account::Wallet(_) => "browser wallet",
        }
    }
}

/// Sends `call` on `chain` once. A burner key goes through the SDK's own prepare, sign and send;
/// a wallet is handed the call with the SDK's gas and fees and sends it itself.
pub async fn send(account: &Account, chain: Chain, call: Call) -> Result<B256, String> {
    match account {
        Account::Burner(key) => {
            let client = match chain {
                Chain::Elysium => Client::new(Web, NETWORK, key.clone()),
                Chain::HyperEvm => Client::parent(Web, NETWORK, key.clone()),
            };
            client.send(call).await.map_err(|error| error.to_string())
        }
        Account::Wallet(from) => wallet::send(chain, *from, call).await,
    }
}

/// The receipt of `hash` on `chain`, polling twice a second for a minute.
pub async fn wait(chain: Chain, hash: B256) -> Result<Receipt, String> {
    let client = chain.reader();
    for _ in 0..120 {
        match client.receipt(hash).await {
            Ok(Some(receipt)) => return Ok(receipt),
            Ok(None) => {}
            Err(error) if df_elysium_core::client::rpc::lagging(&error) => {}
            Err(error) => return Err(error.to_string()),
        }
        sleep(500).await;
    }
    Err(format!("{hash} is not in a block after a minute"))
}

/// Resolves after `ms` milliseconds, on the browser's timer.
pub async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// `0x1234...abcd`.
pub fn short(text: &str) -> String {
    match text.len() > 14 {
        true => format!("{}...{}", &text[..6], &text[text.len() - 4..]),
        false => text.to_owned(),
    }
}
