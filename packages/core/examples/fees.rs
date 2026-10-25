//! Reads Elysium testnet's chain, ArbOS and gas, and prints what a HYPE transfer costs, split
//! between Elysium's execution and HyperEVM's data: `cargo run -p df-elysium-core --example fees`.

use std::error::Error;

use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, Call};
use df_elysium_core::nitro::{gas_info, sys};
use df_elysium_core::utils::hype;
use df_elysium_core::{Address, Client, Network, U256};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::reader(Native::new(), Network::Testnet);

    let block = client.block(BlockTag::Latest, false).await?.ok_or("no latest block")?;
    let reported = client.read(&sys::arb_os_version()).await?;
    println!("Elysium {}  chain {}", client.network(), client.chain_id().await?);
    println!("        block {}  at HyperEVM block {}", block.number, block.l1_block_number);
    println!("        ArbOS {} (arbOSVersion {reported})  send count {}", sys::arbos(reported), block.send_count);

    let prices = client.read(&gas_info::prices_in_wei()).await?;
    let floor = client.read(&gas_info::minimum_gas_price()).await?;
    println!("\nbase fee {} wei, floor {floor}", prices.per_arb_gas_total);
    println!("HyperEVM data {} wei per unit", prices.per_l1_calldata_unit);

    let sender: Address = "0xA16A365aE8f34EDaDB8157849d4ACe274bBa9164".parse()?;
    let transfer = Call { to: Some(sender), value: U256::ONE, data: Vec::new() };
    let estimate = client.estimate(Some(sender), &transfer).await?;
    println!("\na HYPE transfer: {} gas, {} of it HyperEVM's data", estimate.gas, estimate.gas_for_l1);
    println!("                 {} HYPE, {} HYPE of it data", estimate.hype()?, hype(estimate.cost_for_l1())?);
    Ok(())
}
