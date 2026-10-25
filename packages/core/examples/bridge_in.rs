//! HYPE in from HyperEVM: the deposit `Inbox.depositEth()` on HyperEVM makes, simulated there with
//! the message number it would get and its hash on Elysium worked out ahead; or a deposit followed
//! from its HyperEVM transaction to the transaction it became on Elysium:
//! `cargo run -p df-elysium-core --example bridge_in [-- --amount 0.001 | --track 0x.. | --send]`.
//!
//! `--send` deposits from the test key, which needs HYPE on HyperEVM testnet for the amount and
//! the gas.

#[path = "support/mod.rs"]
mod support;

use std::time::Duration;

use df_elysium_core::bridge::deposit::DEPOSIT_ETH;
use df_elysium_core::bridge::{self, Deposit, Message, apply_alias};
use df_elysium_core::client::native::Native;
use df_elysium_core::client::{BlockTag, CallRequest, Client, Transaction};
use df_elysium_core::utils::{Dec, hype, wei};
use df_elysium_core::{Address, B256, Network};

/// A deposit HyperEVM testnet delivered to Elysium testnet on 2026-10-25.
const RECORDED: &str = "0x6396795cebf8f138546d3b2a79014561278ae69efde699100888bdfbca11f5b2";

/// HYPE's system address on HyperEVM: it holds HYPE, so a simulation from it never runs short.
const HYPE_SYSTEM: &str = "0x2222222222222222222222222222222222222222";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let network = Network::Testnet;
    let elysium = Client::reader(Native::new(), network);
    let parent = Client::parent(Native::new(), network, support::key()?);
    let me = parent.address();

    if args.flag("track") {
        let hash = args.value("track").filter(|value| value.starts_with("0x")).unwrap_or(RECORDED);
        return track(&elysium, &parent, hash.parse()?).await;
    }

    let amount: Dec = args.parsed("amount", Dec::parse("0.001")?)?;
    let call = bridge::deposit(network, amount)?;
    println!("{me}");
    println!("  HYPE on HyperEVM  {}", parent.hype(me).await?);
    println!("  HYPE on Elysium   {}", elysium.hype(me).await?);
    println!(
        "\ndeposit {amount} HYPE: depositEth() on the inbox {} on chain {}",
        network.contracts().inbox,
        parent.chain()
    );

    let funded = parent.balance(me).await? > wei(amount)?;
    let sender: Address = if funded { me } else { HYPE_SYSTEM.parse()? };
    if !funded {
        println!("  (the test key has no HYPE on HyperEVM: simulated from {sender} instead)");
    }
    let request = CallRequest::of(Some(sender), &call);
    let answer = parent.rpc().call(&request, BlockTag::Latest).await?;
    let number = DEPOSIT_ETH.answer(&answer)?.first().and_then(|token| token.as_u256()).ok_or("no message number")?;
    let gas = parent.rpc().estimate_gas(&request).await?;
    let deposit = Deposit { message_number: number, from: apply_alias(sender), to: sender, value: call.value };
    println!("  simulated         message {number}, {gas} gas on HyperEVM");
    println!("  on Elysium        {} from {}", deposit.tx_hash(network.chain_id()), deposit.from);
    println!(
        "                    (the hash if it is message {number}: the next deposit's number is known only once sent)"
    );

    if !args.flag("send") {
        support::dry();
        return Ok(());
    }
    let hash = parent.send(call).await?;
    println!("\nsent on HyperEVM  {hash}");
    let receipt = support::receipt(&parent, hash).await?;
    println!("  in block {}, succeeded {}", receipt.block_number.unwrap_or_default(), receipt.succeeded());
    track(&elysium, &parent, hash).await
}

/// The deposits a HyperEVM transaction made, followed to Elysium.
async fn track<S>(elysium: &Client<Native>, parent: &Client<Native, S>, hash: B256) -> support::Result<()> {
    let network = elysium.network();
    let receipt = parent.receipt(hash).await?.ok_or("that transaction is not on HyperEVM")?;
    println!("HyperEVM {hash}  block {}", receipt.block_number.unwrap_or_default());
    let messages = Message::in_logs(&receipt.logs, &network.contracts())?;
    if messages.is_empty() {
        return Err("it delivered nothing to Elysium's inbox".into());
    }
    for message in &messages {
        let Some(deposit) = Deposit::from_message(message) else {
            println!("  message {}  {:?}, not a deposit", message.number, message.kind);
            continue;
        };
        let l2 = deposit.tx_hash(network.chain_id());
        println!("  message {}  {} HYPE to {}", message.number, hype(deposit.value)?, deposit.to);
        println!("  on Elysium  {l2}");
        let landed = landed(elysium, l2).await?;
        let block = landed.block_number.ok_or("pending")?;
        let succeeded = elysium.receipt(l2).await?.is_some_and(|receipt| receipt.succeeded());
        println!("              in block {block}, type {:#x}, succeeded {succeeded}", landed.kind);
        println!("              {}", network.tx_url(l2));
    }
    Ok(())
}

/// The deposit's transaction on Elysium, polled for up to a minute: a deposit lands a block or two
/// after HyperEVM's.
async fn landed(elysium: &Client<Native>, hash: B256) -> support::Result<Transaction> {
    for _ in 0..120 {
        if let Some(transaction) = elysium.transaction(hash).await? {
            return Ok(transaction);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!("{hash} has not landed on Elysium after a minute").into())
}
