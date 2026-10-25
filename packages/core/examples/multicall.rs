//! Reads several HYPE balances, `block.number` and an ERC-20's metadata in one `eth_call` through
//! Multicall3, one call allowed to fail: `cargo run -p df-elysium-core --example multicall
//! [-- --token 0x..]`.

#[path = "support/mod.rs"]
mod support;

use df_elysium_core::client::Client;
use df_elysium_core::client::native::Native;
use df_elysium_core::nitro::ARB_SYS;
use df_elysium_core::transfer::erc20;
use df_elysium_core::transfer::multicall::{Call3, aggregate3, block_number, eth_balance};
use df_elysium_core::utils::hype;
use df_elysium_core::{Address, Network};

/// USDC on Elysium testnet: 6 decimals, minted by no faucet.
const USDC: &str = "0x7D29d8047B905000459c0E80c34a26CeedcB47b2";

#[tokio::main]
async fn main() -> support::Result<()> {
    let args = support::Args::read();
    let token = support::address(&args, "token", USDC.parse()?)?;
    let client = Client::reader(Native::new(), Network::Testnet);
    let contracts = Network::Testnet.contracts();

    let holders: [(&str, Address); 3] = [
        ("operator", "0xA16A365aE8f34EDaDB8157849d4ACe274bBa9164".parse()?),
        ("network fees", "0x252431E84D5E22435A0c833C2220770c52F59633".parse()?),
        ("test key", support::key().map_or(Address::ZERO, |key| key.address())),
    ];
    let balances: Vec<_> = holders.iter().map(|(_, holder)| eth_balance(*holder)).collect();
    let height = block_number();
    let (symbol, decimals, supply) = (erc20::symbol(token), erc20::decimals(token), erc20::total_supply(token));
    // ArbSys has no balanceOf: a call that reverts, which the batch is allowed to survive.
    let missing = erc20::balance(ARB_SYS, contracts.inbox);

    let mut calls: Vec<Call3> = balances.iter().map(|read| Call3::of(read, false)).collect();
    calls.push(Call3::of(&height, false));
    calls.extend([Call3::of(&symbol, false), Call3::of(&decimals, false), Call3::of(&supply, false)]);
    calls.push(Call3::of(&missing, true));
    let outcomes = client.read(&aggregate3(&calls)).await?;
    println!("{} calls in one eth_call to Multicall3\n", calls.len());

    let mut answers = outcomes.iter();
    let mut next = || answers.next().ok_or("fewer answers than calls");
    for ((name, holder), read) in holders.iter().zip(&balances) {
        println!("{name:<13} {holder}  {} HYPE", hype(read.decode(&next()?.data)?)?);
    }
    println!("block.number  {}  (HyperEVM's, as any contract on Elysium reads it)", height.decode(&next()?.data)?);
    let symbol = symbol.decode(&next()?.data)?;
    let decimals = decimals.decode(&next()?.data)?;
    let supply = supply.decode(&next()?.data)?;
    println!("{symbol:<13} {token}  {decimals} decimals, {} supply", erc20::amount(supply, decimals)?);
    let failed = next()?;
    println!("allowed fail  succeeded {}, {} bytes of revert data", failed.success, failed.data.len());
    Ok(())
}
