//! What the examples share: their flags, the test key from the environment or the workspace's
//! `.env`, and waiting for a receipt on a chain without `eth_sendRawTransactionSync`.

// Each example uses its own part of this.
#![allow(dead_code)]

use std::error::Error;
use std::time::Duration;

use df_elysium_core::account::LocalKey;
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{Client, Receipt};
use df_elysium_core::{Address, B256, Network};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The flags an example was run with: `--send`, `--to 0x..`, `--amount 0.001`.
pub struct Args(Vec<String>);

impl Args {
    pub fn read() -> Args {
        Args(std::env::args().skip(1).collect())
    }

    /// Whether `--name` was given.
    pub fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|arg| arg == &format!("--{name}"))
    }

    /// What follows `--name`.
    pub fn value(&self, name: &str) -> Option<&str> {
        let at = self.0.iter().position(|arg| arg == &format!("--{name}"))?;
        self.0.get(at + 1).map(String::as_str)
    }

    /// What follows `--name`, read as `T`, or `default`.
    pub fn parsed<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T>
    where
        T::Err: Error + 'static,
    {
        match self.value(name) {
            Some(text) => Ok(text.parse()?),
            None => Ok(default),
        }
    }
}

/// The key that signs: `ELYSIUM_TEST_KEY` from the environment, or from the workspace's `.env`.
pub fn key() -> Result<LocalKey> {
    let from_env = std::env::var("ELYSIUM_TEST_KEY").ok();
    let from_file = || {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env");
        let text = std::fs::read_to_string(path).ok()?;
        text.lines().find_map(|line| line.strip_prefix("ELYSIUM_TEST_KEY=").map(|key| key.trim().to_owned()))
    };
    let secret = from_env.or_else(from_file).ok_or("set ELYSIUM_TEST_KEY, or put it in the workspace's .env")?;
    Ok(LocalKey::from_hex(&secret)?)
}

/// A client that signs with [`key`], on Elysium testnet.
pub fn signer() -> Result<Client<Native, LocalKey>> {
    Ok(Client::new(Native::new(), Network::Testnet, key()?))
}

/// An address from a flag, or `default`.
pub fn address(args: &Args, name: &str, default: Address) -> Result<Address> {
    match args.value(name) {
        Some(text) => Ok(text.parse()?),
        None => Ok(default),
    }
}

/// The receipt of `hash` once it is in a block, polling every half second for up to a minute.
pub async fn receipt<S>(client: &Client<Native, S>, hash: B256) -> Result<Receipt> {
    for _ in 0..120 {
        if let Some(receipt) = client.receipt(hash).await? {
            return Ok(receipt);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!("{hash} is not in a block after a minute").into())
}

/// What a run without `--send` prints at its end.
pub fn dry() {
    println!("\nnothing sent: run again with --send to send it");
}
