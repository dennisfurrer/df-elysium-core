//! Reads Stylus's parameters on Elysium testnet and, given a program, its version, time left and
//! what activating it would charge: `cargo run -p df-elysium-core --example stylus
//! [-- --program 0x.. --funds 0.01]`.

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::client::native::Native;
use df_elysium_core::client::{Client, ReadError};
use df_elysium_core::nitro::wasm;
use df_elysium_core::utils::{Dec, hype, wei};
use df_elysium_core::{Address, Network};

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = Client::reader(Native::new(), Network::Testnet);

    println!("Stylus {}", client.read(&wasm::stylus_version()).await?);
    println!("  ink price   {}", client.read(&wasm::ink_price()).await?);
    println!("  stack       {} deep", client.read(&wasm::max_stack_depth()).await?);
    println!(
        "  pages       {} free, {} gas each, at most {}",
        client.read(&wasm::free_pages()).await?,
        client.read(&wasm::page_gas()).await?,
        client.read(&wasm::page_limit()).await?
    );
    println!(
        "  lifetime    {} days, kept alive after {}",
        client.read(&wasm::expiry_days()).await?,
        client.read(&wasm::keepalive_days()).await?
    );
    println!("  cached      {} programs a block", client.read(&wasm::block_cache_size()).await?);
    let managers = client.read(&wasm::cache_managers()).await?;
    println!("  managers    {}", managers.len());

    let Some(program) = args.value("program") else {
        println!("\nrun with --program 0x.. for a program's version and activation quote");
        return Ok(());
    };
    let program: Address = program.parse()?;
    let funds: Dec = args.parsed("funds", Dec::parse("0.01")?)?;
    println!("\nprogram {program}  ({} bytes of code)", client.code(program).await?.len());
    match client.read(&wasm::program_version(program)).await {
        Ok(version) => {
            println!("  version     {version}");
            println!("  time left   {} s", client.read(&wasm::program_time_left(program)).await?);
        }
        Err(refused) => println!("  version     none: {}", reason(&refused)),
    }
    match client.read(&wasm::activation_quote(program, wei(funds)?)).await {
        Ok(quote) => println!("  activation  Stylus {}, data fee {} HYPE", quote.version, hype(quote.data_fee)?),
        Err(refused) => println!("  activation  refused: {}", reason(&refused)),
    }
    Ok(())
}

/// The node's words, and the revert data when there is any.
fn reason(error: &ReadError) -> String {
    match error {
        ReadError::Rpc(df_elysium_core::client::RpcError::Node { error, .. }) => match &error.data {
            Some(data) => format!("{} ({data})", error.message),
            None => error.message.clone(),
        },
        other => other.to_string(),
    }
}
