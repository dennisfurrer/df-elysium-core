//! Reads a contract's verification on Conduit's Sourcify and, given the compiler's standard JSON
//! input, submits it and waits for the job: `cargo run -p df-elysium-core --example verify
//! [-- --address 0x.. --input std.json --compiler 0.8.37+commit.f401782d --contract src/A.sol:A
//! --creation 0x..]`.

#[path = "support/mod.rs"]
mod support;

use std::time::Duration;

use df_elysium_core::client::native::Native;
use df_elysium_core::deploy::verify::{self, Match, Status};
use df_elysium_core::{Address, B256, Network};

/// The reactor's escrow on Elysium testnet, verified `exact_match`.
const ESCROW: &str = "0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let network = Network::Testnet;
    let address = support::address(&args, "address", ESCROW.parse()?)?;
    let http = Native::new();

    let status = verify::status(&http, network, address).await?;
    show(&status);
    println!("  {}", network.address_url(address));

    let (Some(input), Some(compiler), Some(contract)) =
        (args.value("input"), args.value("compiler"), args.value("contract"))
    else {
        println!("\nrun with --input std.json --compiler <version> --contract <path:Name> to submit one");
        return Ok(());
    };
    let input = std::fs::read_to_string(input)?;
    let creation = args.value("creation").map(str::parse::<B256>).transpose()?;
    let id = verify::submit(&http, network, address, &input, compiler, contract, creation).await?;
    println!("\nsubmitted: job {id}");
    for _ in 0..60 {
        let job = verify::job(&http, network, &id).await?;
        match job.outcome {
            Some(Ok(status)) => {
                show(&status);
                return Ok(());
            }
            Some(Err(refusal)) => return Err(refusal.into()),
            None => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
    Err(format!("job {id} did not finish in a minute").into())
}

fn show(status: &Status) {
    let level = |matched: Option<Match>| match matched {
        Some(Match::Exact) => "exact",
        Some(Match::Partial) => "partial",
        None => "none",
    };
    let address: Address = status.address;
    match status.is_verified() {
        true => println!(
            "{address}  verified: {} (creation {}, runtime {}) at {}",
            level(status.matched),
            level(status.creation),
            level(status.runtime),
            status.verified_at.as_deref().unwrap_or("an unknown time")
        ),
        false => println!("{address}  not verified"),
    }
}
