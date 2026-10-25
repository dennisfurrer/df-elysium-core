//! Follows Elysium testnet for a while by polling: each new block with its transactions, and each
//! withdrawal to HyperEVM as ArbSys logs it: `cargo run -p df-elysium-core --example follow
//! [-- --seconds 20]`.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use df_elysium_core::Network;
use df_elysium_core::bridge::Withdrawal;
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, BlockTransactions, Client, LogFilter};
use df_elysium_core::live::{Heads, LogCursor};
use df_elysium_core::nitro::{ARB_SYS, sys};
use df_elysium_core::utils::hype;

/// Blocks a withdrawal log is read behind the head.
const CONFIRMATIONS: u64 = 2;

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let seconds: u64 = args.parsed("seconds", 20)?;
    let client = Client::reader(Native::new(), Network::Testnet);

    let start = client.block_number().await?;
    let mut heads = Heads::after(start);
    let withdrawals = LogFilter {
        address: vec![ARB_SYS],
        topics: vec![Some(vec![sys::L2_TO_L1_TX.topic()])],
        ..LogFilter::default()
    };
    let mut cursor = LogCursor::new(withdrawals, start + 1, Network::Testnet.log_span(), CONFIRMATIONS);
    println!("following from block {start} for {seconds} s\n");

    let (until, mut blocks, mut seen) = (Instant::now() + Duration::from_secs(seconds), 0, 0);
    while Instant::now() < until {
        let head = client.block_number().await?;
        if let Some(new) = heads.seen(head) {
            for number in new {
                let Some(block) = client.block(BlockTag::Number(number), false).await? else { continue };
                let count = match &block.transactions {
                    BlockTransactions::Hashes(hashes) => hashes.len(),
                    BlockTransactions::Full(transactions) => transactions.len(),
                };
                println!(
                    "block {number}  {count} tx  at HyperEVM {}  {} sends",
                    block.l1_block_number, block.send_count
                );
                blocks += 1;
            }
        }
        if let Some(window) = cursor.window(head) {
            for log in client.logs(&window).await? {
                let sent = Withdrawal::from_log(&log)?;
                println!(
                    "  withdrawal  leaf {}  {} HYPE to {} from {}",
                    sent.position,
                    hype(sent.value)?,
                    sent.destination,
                    sent.caller
                );
                seen += 1;
            }
            cursor.read(&window);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    println!("\n{blocks} blocks, {seen} withdrawals; logs read to block {}", cursor.next() - 1);
    Ok(())
}
