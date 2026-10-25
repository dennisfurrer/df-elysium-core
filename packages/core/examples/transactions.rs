//! A transaction from end to end on Elysium testnet: estimated (Elysium's gas and HyperEVM's data),
//! prepared, signed and its signer recovered, sent once, waited for, and read back with Nitro's
//! fields; then three sent back to back on the kept nonce, the last with
//! `eth_sendRawTransactionSync`. Each is a 0 HYPE transfer to the key's own address. Without
//! `--send` it stops after signing.
//!
//! `cargo run -p df-elysium-core --example transactions -- [--send]`

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::U256;
use df_elysium_core::account::recover;
use df_elysium_core::client::{BlockTag, tx_hash};
use df_elysium_core::transfer;
use df_elysium_core::utils::{hex, hype};

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = support::signer()?;
    let me = client.address();
    let call = transfer::hype_wei(me, U256::ZERO);

    let estimate = client.estimate(Some(me), &call).await?;
    println!(
        "estimate  {} gas: {} Elysium, {} HyperEVM data",
        estimate.gas,
        estimate.gas_for_l2(),
        estimate.gas_for_l1
    );
    println!(
        "          base fee {} wei, HyperEVM data {} wei a unit, {} HYPE",
        estimate.base_fee,
        estimate.l1_base_fee,
        estimate.hype()?
    );

    let tx = client.prepare(call.clone()).await?;
    println!("\nprepared  chain {}  nonce {}  gas {} (the node's estimate, 30% over)", tx.chain_id, tx.nonce, tx.gas);
    println!("          fee cap {} wei  tip {}", tx.max_fee, tx.max_priority_fee);
    let raw = client.sign(&tx).await?;
    let signature = client.signer().sign_digest(&tx.signing_hash().0)?;
    println!("signed    {} bytes, type {:#04x}, hash {}", raw.len(), raw[0], tx_hash(&raw));
    println!("          recovers to {}", recover(&tx.signing_hash().0, &signature)?);
    println!("          {}...", &hex::encode_prefixed(&raw)[..42]);
    if !args.flag("send") {
        support::dry();
        return Ok(());
    }

    let hash = client.send_prepared(&tx).await?;
    let receipt = support::receipt(&client, hash).await?;
    println!("\nsent      {}", client.network().tx_url(hash));
    println!(
        "          block {}, HyperEVM block {}",
        receipt.block_number.unwrap_or_default(),
        receipt.l1_block_number.unwrap_or_default()
    );
    println!(
        "          gas {} ({} HyperEVM data), fee {} HYPE",
        receipt.gas_used,
        receipt.gas_used_for_l1,
        hype(receipt.fee().unwrap_or_default())?
    );
    let record = client.transaction(hash).await?.ok_or("the node does not know it")?;
    println!(
        "read back type {:?}  nonce {}  at {}",
        record.tx_kind(),
        record.nonce,
        record.block_timestamp.unwrap_or_default()
    );
    let block =
        client.block(BlockTag::Number(receipt.block_number.unwrap_or_default()), false).await?.ok_or("no block")?;
    println!("its block send count {}, ArbOS {}", block.send_count, block.arbos_version());

    println!("\nthree back to back, the nonce kept:");
    for nth in 0..3 {
        let tx = client.prepare(call.clone()).await?;
        let receipt = match nth {
            2 => client.send_prepared_sync(&tx, Some(10_000)).await?,
            _ => support::receipt(&client, client.send_prepared(&tx).await?).await?,
        };
        let how = if nth == 2 { "sync" } else { "sent, then polled" };
        println!("  nonce {}  block {}  {how}", tx.nonce, receipt.block_number.unwrap_or_default());
    }
    println!("next nonce kept: {:?}, the node's: {}", client.nonces().kept(), client.nonce(me).await?);
    Ok(())
}
