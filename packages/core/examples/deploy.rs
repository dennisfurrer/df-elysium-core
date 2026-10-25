//! Contracts on Elysium testnet: a creation from the key (CREATE, landing at the key's address and
//! nonce) and one through the deterministic deployer (CREATE2, landing where the salt and code say
//! whoever sends it), each called once deployed, and Sourcify's verdict on them and on a verified
//! contract. The contract answers 42 to any call. Without `--send` it prints where each would land.
//!
//! `cargo run -p df-elysium-core --example deploy -- [--salt 0x..] [--send]`

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, CallRequest, create_address};
use df_elysium_core::deploy::{self, CREATE2_DEPLOYER, verify};
use df_elysium_core::utils::keccak256;
use df_elysium_core::{Address, B256, U256};

/// Copies the ten bytes after it into memory and returns them: a contract that answers 42 to
/// anything (`PUSH1 42, PUSH1 0, MSTORE, PUSH1 32, PUSH1 0, RETURN`).
const FORTY_TWO: [u8; 22] = [
    0x60, 0x0a, 0x60, 0x0c, 0x60, 0x00, 0x39, 0x60, 0x0a, 0x60, 0x00, 0xf3, // init: copy and return
    0x60, 0x2a, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3, // runtime
];

/// The reactor's escrow on Elysium testnet, verified on Conduit's Sourcify.
const ESCROW: &str = "0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = support::signer()?;
    let me = client.address();
    let head = client.block_number().await?;
    let salt: B256 = match args.value("salt") {
        Some(salt) => salt.parse()?,
        None => B256(keccak256(&head.to_be_bytes())),
    };

    let nonce = client.nonce(me).await?;
    let created = create_address(me, nonce);
    let created2 = deploy::create2_address(CREATE2_DEPLOYER, salt, deploy::init_code_hash(&FORTY_TWO));
    println!("CREATE   from {me} at nonce {nonce}: lands at {created}");
    println!("CREATE2  through {CREATE2_DEPLOYER}, salt {salt}: lands at {created2}");

    if args.flag("send") {
        let receipt = client.send_sync(deploy::create(&FORTY_TWO, &[]), Some(10_000)).await?;
        println!("\ncreated  {}  gas {}", client.network().tx_url(receipt.transaction_hash), receipt.gas_used);
        if receipt.contract_address != Some(created) {
            return Err(format!("the creation landed at {:?}, not {created}", receipt.contract_address).into());
        }
        let receipt = client.send_sync(deploy::create2(salt, &FORTY_TWO), Some(10_000)).await?;
        println!("created2 {}  gas {}", client.network().tx_url(receipt.transaction_hash), receipt.gas_used);
        for at in [created, created2] {
            let code = client.code(at).await?;
            let answer = client.rpc().call(&CallRequest::to(at, Vec::new()), BlockTag::Latest).await?;
            let answer = U256::from_be_slice(&answer).ok_or("not a word")?;
            println!("         {at}: {} bytes of code, answers {answer}", code.len());
        }
    }

    let http = Native::new();
    for at in [ESCROW.parse::<Address>()?, created] {
        let status = verify::status(&http, client.network(), at).await?;
        match status.matched {
            Some(matched) => {
                println!("\nSourcify {at}: {matched:?} match, verified {}", status.verified_at.unwrap_or_default())
            }
            None => println!("\nSourcify {at}: not verified"),
        }
    }
    println!(
        "verify one with `forge verify-contract --verifier sourcify --verifier-url {} <address> <src:Contract>`",
        client.network().verifier_url()
    );
    println!("or deploy::verify::submit with its standard JSON input");
    if !args.flag("send") {
        support::dry();
    }
    Ok(())
}
