//! Sends HYPE on Elysium testnet from the test key and prints the receipt: what it cost, how much
//! of that was HyperEVM's data, and the explorer link. Without `--send` it prepares the transaction
//! and prints it.
//!
//! `cargo run -p df-elysium-core --example send_hype -- [--to 0x..] [--amount 0.0001] [--send]`
//! (`--to` defaults to the key's own address).

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::gas;
use df_elysium_core::transfer;
use df_elysium_core::utils::hype;

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = support::signer()?;
    let me = client.address();
    let to = support::address(&args, "to", me)?;
    let amount = args.parsed("amount", "0.0001".parse()?)?;

    let before = client.hype(me).await?;
    println!("from    {me}  {before} HYPE");
    println!("to      {to}");
    println!("amount  {amount} HYPE");

    let call = transfer::hype(to, amount)?;
    let tx = client.prepare(call).await?;
    let fees = df_elysium_core::client::Fees { max_priority_fee: tx.max_priority_fee, max_fee: tx.max_fee };
    println!(
        "\nchain {}  nonce {}  gas {}  fee cap {} wei  tip {}",
        tx.chain_id, tx.nonce, tx.gas, tx.max_fee, tx.max_priority_fee
    );
    println!("at most {} HYPE, gas and amount", gas::most_spent(tx.gas, fees, tx.value)?);
    if !args.flag("send") {
        support::dry();
        return Ok(());
    }

    let receipt = client.send_prepared_sync(&tx, Some(10_000)).await?;
    let fee = receipt.fee().ok_or("the receipt names no gas price")?;
    println!("\nsent    {}", client.network().tx_url(receipt.transaction_hash));
    println!(
        "block   {}  {}",
        receipt.block_number.unwrap_or_default(),
        if receipt.succeeded() { "succeeded" } else { "reverted" }
    );
    println!("gas     {} used, {} of it HyperEVM's data", receipt.gas_used, receipt.gas_used_for_l1);
    println!("fee     {} HYPE", hype(fee)?);
    println!("now     {} HYPE", client.hype(me).await?);
    Ok(())
}
