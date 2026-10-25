//! Reads everything Elysium testnet says of itself: its head and ArbOS, who runs it, who posts its
//! batches, its gas limits and prices, and how far the outbox is confirmed on HyperEVM:
//! `cargo run -p df-elysium-core --example chain`.

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::bridge::claim;
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, Client};
use df_elysium_core::nitro::{aggregator, gas_info, owner, retryable, sys};
use df_elysium_core::{Address, Network};

#[tokio::main]
async fn main() -> support::Result<()> {
    let network = Network::Testnet;
    let client = Client::reader(Native::new(), network);
    let parent = Client::parent(Native::new(), network, ());

    let head = client.block(BlockTag::Latest, false).await?.ok_or("no latest block")?;
    let reported = client.read(&sys::arb_os_version()).await?;
    println!("Elysium {network}  chain {}  ({})", client.chain_id().await?, client.rpc().url());
    println!("  head        block {}  at HyperEVM block {}", head.number, head.l1_block_number);
    println!(
        "  ArbOS       {} from the mix hash, {} from ArbSys ({reported})",
        head.arbos_version(),
        sys::arbos(reported)
    );
    println!("  outbox      {} sends, root {}", head.send_count, head.send_root);

    let owners = client.read(&owner::chain_owners()).await?;
    println!("\nowners       {}", list(&owners));
    println!("network fee  {}", client.read(&owner::network_fee_account()).await?);
    println!("infra fee    {}", client.read(&owner::infra_fee_account()).await?);
    let upgrade = client.read(&owner::scheduled_upgrade()).await?;
    match upgrade.is_scheduled() {
        true => println!("upgrade      ArbOS {} at {}", upgrade.arbos_version, upgrade.at),
        false => println!("upgrade      none scheduled"),
    }
    for poster in client.read(&aggregator::batch_posters()).await? {
        let collector = client.read(&aggregator::fee_collector(poster)).await?;
        println!("poster       {poster}  pays {collector}");
    }

    let prices = client.read(&gas_info::prices_in_wei()).await?;
    println!(
        "\ngas limits   {} a transaction, {} a block",
        client.read(&gas_info::max_tx_gas_limit()).await?,
        client.read(&gas_info::max_block_gas_limit()).await?
    );
    println!(
        "base fee     {} wei = {} + {} congestion, floor {}",
        prices.per_arb_gas_total,
        prices.per_arb_gas_base,
        prices.per_arb_gas_congestion,
        client.read(&gas_info::minimum_gas_price()).await?
    );
    println!("per tx       {} wei, per HyperEVM data unit {} wei", prices.per_l2_tx, prices.per_l1_calldata_unit);
    println!(
        "data price   {} wei, HyperEVM's base fee as ArbOS estimates it",
        client.read(&gas_info::l1_base_fee_estimate()).await?
    );
    for window in client.read(&gas_info::gas_pricing_constraints()).await? {
        println!("pricing      {} gas/s over {} s, backlog {}", window.target, window.window, window.backlog);
    }
    println!(
        "tips         {}",
        match client.read(&owner::collect_tips()).await? {
            true => "collected, but ordering is first come, first served",
            false => "not collected",
        }
    );
    let lifetime = client.read(&retryable::lifetime()).await?;
    println!("tickets      live {lifetime} s");

    let contracts = network.contracts();
    let confirmed = claim::latest(&contracts, parent.rpc()).await?;
    let block = client.block_by_hash(confirmed.block_hash, false).await?.ok_or("the confirmed block is unknown")?;
    println!("\nconfirmed    block {} on HyperEVM's rollup {}", block.number, contracts.rollup);
    println!(
        "             {} of {} sends claimable, {} blocks behind the head",
        block.send_count,
        head.send_count,
        head.number - block.number
    );
    println!("             send root {}", confirmed.send_root);
    println!("bridge       {} HYPE locked on HyperEVM, backing Elysium's", parent.hype(contracts.bridge).await?);
    Ok(())
}

fn list(addresses: &[Address]) -> String {
    addresses.iter().map(Address::to_string).collect::<Vec<_>>().join(", ")
}
