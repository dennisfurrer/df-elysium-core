//! An ERC-20 on Elysium testnet: its name, symbol, decimals, supply and the key's balance, then a
//! transfer and an approval, amounts typed as decimals and sent in the token's units exactly.
//! `--deploy` first creates a token of the key's own (`support/TestToken.sol`, a million ETEST at
//! six decimals) so there is something to send; `--token` names any other. Without `--send` it
//! prints the calls.
//!
//! `cargo run -p df-elysium-core --example erc20 -- [--deploy | --token 0x..] [--to 0x..]
//! [--amount 1.5] [--send]`

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::client::create_address;
use df_elysium_core::transfer::erc20;
use df_elysium_core::utils::hex;
use df_elysium_core::{Address, deploy};

/// The test token's init code, compiled from `support/TestToken.sol` with solc 0.8.30.
const TEST_TOKEN: &str = include_str!("support/TestToken.hex");

/// Kinetiq's USDC on Elysium testnet, nobody's to mint: read only, unless the key holds some.
const USDC: &str = "0x7D29d8047B905000459c0E80c34a26CeedcB47b2";

/// The reactor's guardian, the default recipient.
const GUARDIAN: &str = "0x8499E50474B6E8C988b232F52583439aBf37A8A1";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let client = support::signer()?;
    let me = client.address();
    let send = args.flag("send");

    let token = match (args.flag("deploy"), args.value("token")) {
        (true, _) => {
            let supply = erc20::units("1000000".parse()?, 6)?;
            let creation = deploy::create(&hex::decode(TEST_TOKEN.trim())?, &supply.to_be_bytes());
            let at = create_address(me, client.nonce(me).await?);
            println!("deploy  ETEST, a million at six decimals, to land at {at}");
            if !send {
                support::dry();
                return Ok(());
            }
            let receipt = client.send_sync(creation, Some(10_000)).await?;
            let made = receipt.contract_address.ok_or("the creation made no contract")?;
            println!("        {}  gas {}", client.network().tx_url(receipt.transaction_hash), receipt.gas_used);
            made
        }
        (false, Some(token)) => token.parse()?,
        (false, None) => USDC.parse()?,
    };

    let decimals = client.read(&erc20::decimals(token)).await?;
    let symbol = client.read(&erc20::symbol(token)).await?;
    let supply = client.read(&erc20::total_supply(token)).await?;
    let held = client.read(&erc20::balance(token, me)).await?;
    println!("\ntoken   {} ({symbol}) at {token}", client.read(&erc20::name(token)).await?);
    println!("        {decimals} decimals, supply {}", erc20::amount(supply, decimals)?);
    println!("held    {} {symbol} by {me}", erc20::amount(held, decimals)?);

    let to: Address = support::address(&args, "to", GUARDIAN.parse()?)?;
    let amount = erc20::units(args.parsed("amount", "1.5".parse()?)?, decimals)?;
    let transfer = erc20::transfer(token, to, amount);
    let approve = erc20::approve(token, to, amount);
    println!("\ntransfer {} {symbol} to {to}", erc20::amount(amount, decimals)?);
    println!("        calldata {}", hex::encode_prefixed(&transfer.data));
    println!("approve  {to} for the same: {}", hex::encode_prefixed(&approve.data));
    if !send {
        support::dry();
        return Ok(());
    }
    if held < amount {
        return Err(format!(
            "{me} holds {} {symbol}: run with --deploy for a token of its own",
            erc20::amount(held, decimals)?
        )
        .into());
    }

    let sent = client.send_sync(transfer, Some(10_000)).await?;
    println!(
        "\nsent    {}  {}",
        client.network().tx_url(sent.transaction_hash),
        if sent.succeeded() { "succeeded" } else { "reverted" }
    );
    let approved = client.send_sync(approve, Some(10_000)).await?;
    println!(
        "approve {}  {}",
        client.network().tx_url(approved.transaction_hash),
        if approved.succeeded() { "succeeded" } else { "reverted" }
    );
    let theirs = client.read(&erc20::balance(token, to)).await?;
    let allowed = client.read(&erc20::allowance(token, me, to)).await?;
    println!(
        "now     {to} holds {} {symbol}, may move {} of {me}'s",
        erc20::amount(theirs, decimals)?,
        erc20::amount(allowed, decimals)?
    );
    println!(
        "        {me} holds {} {symbol}",
        erc20::amount(client.read(&erc20::balance(token, me)).await?, decimals)?
    );
    Ok(())
}
