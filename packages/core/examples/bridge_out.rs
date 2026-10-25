//! HYPE out of Elysium to HyperEVM. A withdrawal is `ArbSys.withdrawEth` on Elysium: the HYPE is
//! burnt at once and becomes a leaf of the outbox's tree. Once the rollup confirms an assertion
//! past that leaf on HyperEVM, anyone executes it there from the outbox, paying HyperEVM's gas,
//! and the HYPE is paid out.
//!
//! - no flags: the withdrawal prepared, and how far the confirmed outbox is behind the head;
//! - `--send`: the withdrawal sent, its leaf read from the receipt, and where its claim stands;
//! - `--claim <tx hash>`: where the withdrawal in that Elysium transaction stands, its claim built
//!   and simulated on HyperEVM once it can be; with `--send` too, the claim sent from the key on
//!   HyperEVM (it needs HYPE there for gas).
//!
//! `cargo run -p df-elysium-core --example bridge_out -- [--to 0x..] [--amount 0.0001] [--send]`
//! `cargo run -p df-elysium-core --example bridge_out -- --claim 0x.. [--send]`

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::bridge::{self, Claim, Withdrawal};
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, CallRequest, Client};
use df_elysium_core::nitro::sys;
use df_elysium_core::utils::hype;
use df_elysium_core::{B256, Network};

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = support::signer()?;
    let parent = Client::parent(Native::new(), Network::Testnet, support::key()?);
    let me = client.address();

    if let Some(hash) = args.value("claim") {
        let hash: B256 = hash.parse()?;
        let receipt = client.receipt(hash).await?.ok_or("Elysium does not know that transaction")?;
        for withdrawal in Withdrawal::in_logs(&receipt.logs)? {
            show(&withdrawal)?;
            claim(&client, &parent, &withdrawal, args.flag("send")).await?;
        }
        return Ok(());
    }

    let to = support::address(&args, "to", me)?;
    let amount = args.parsed("amount", "0.0001".parse()?)?;
    let call = bridge::withdraw(to, amount)?;
    let tree = client.read(&sys::send_merkle_tree_state()).await?;
    let confirmed = bridge::claim::latest(&client.network().contracts(), parent.rpc()).await?;
    let block = client.block_by_hash(confirmed.block_hash, false).await?.ok_or("no confirmed block")?;
    let head = client.block_number().await?;
    println!("withdraw {amount} HYPE from {me} on Elysium to {to} on HyperEVM");
    println!("         through ArbSys, {} HYPE held", client.hype(me).await?);
    println!("outbox   {} leaves; the rollup has confirmed {} of them,", tree.size, block.send_count);
    println!("         up to block {} of {head}, {} blocks behind", block.number, head.saturating_sub(block.number));
    if !args.flag("send") {
        support::dry();
        return Ok(());
    }

    let receipt = client.send_sync(call, Some(10_000)).await?;
    println!(
        "\nsent     {}  {}",
        client.network().tx_url(receipt.transaction_hash),
        if receipt.succeeded() { "succeeded" } else { "reverted" }
    );
    for withdrawal in Withdrawal::in_logs(&receipt.logs)? {
        show(&withdrawal)?;
        claim(&client, &parent, &withdrawal, false).await?;
    }
    println!("\nclaim it later: --claim {}", receipt.transaction_hash);
    Ok(())
}

fn show(withdrawal: &Withdrawal) -> support::Result<()> {
    println!("\nleaf     {} in the outbox's tree, hash {}", withdrawal.position, withdrawal.leaf());
    println!("         {} HYPE from {} to {}", hype(withdrawal.value)?, withdrawal.caller, withdrawal.destination);
    println!(
        "         Elysium block {}, HyperEVM block {}, at {}",
        withdrawal.arb_block, withdrawal.eth_block, withdrawal.timestamp
    );
    Ok(())
}

async fn claim<S>(
    client: &Client<Native, S>,
    parent: &Client<Native, df_elysium_core::account::LocalKey>,
    withdrawal: &Withdrawal,
    send: bool,
) -> support::Result<()> {
    match bridge::claim(client, parent.rpc(), withdrawal).await? {
        Claim::Spent => println!("claim    executed already: the HYPE was paid"),
        Claim::Waiting { confirmed } => println!(
            "claim    waiting: the rollup has confirmed {confirmed} leaves, {} more to this one",
            withdrawal.position + 1 - confirmed
        ),
        Claim::Ready { call, confirmed } => {
            println!(
                "claim    ready, against Elysium block {} (send root {})",
                confirmed.block_hash, confirmed.send_root
            );
            let from = parent.address();
            let simulated = CallRequest { from: Some(from), ..CallRequest::of(Some(from), &call) };
            parent.rpc().call(&simulated, BlockTag::Latest).await?;
            let gas = parent.rpc().estimate_gas(&simulated).await?;
            println!("         Outbox.executeTransaction simulated on HyperEVM: it goes through, {gas} gas");
            println!("         {} HYPE held on HyperEVM for its gas", parent.hype(from).await?);
            if !send {
                support::dry();
                return Ok(());
            }
            let hash = parent.send(call).await?;
            let receipt = support::receipt(parent, hash).await?;
            println!("claimed  tx {hash} on HyperEVM  {}", if receipt.succeeded() { "succeeded" } else { "reverted" });
        }
    }
    Ok(())
}
