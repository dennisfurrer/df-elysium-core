//! An ERC-20 on Elysium: its name, symbol, decimals, supply and the account's balance in one
//! `eth_call` through Multicall3; a transfer and an approval, amounts typed as decimals and sent in
//! the token's units exactly; and a token of the account's own to try them with (ETEST, a million
//! at six decimals, `examples/support/TestToken.sol`).

use df_elysium_core::client::create_address;
use df_elysium_core::transfer::erc20;
use df_elysium_core::transfer::multicall::{Call3, aggregate3};
use df_elysium_core::utils::hex;
use df_elysium_core::{Address, U256, deploy};
use leptos::prelude::*;

use crate::net::Chain;
use crate::state::{Ctx, Out, Status, poll, problem, run, text, transact};
use crate::ui::{Actions, AddressLink, Field, Panel, Part, Row, StatusLine};

/// The test token's init code, compiled from `examples/support/TestToken.sol` with solc 0.8.30.
const TEST_TOKEN: &str = include_str!("../../../support/TestToken.hex");

/// Kinetiq's USDC on Elysium testnet: nobody's to mint, so read only unless the account holds some.
const USDC: &str = "0x7D29d8047B905000459c0E80c34a26CeedcB47b2";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    at: Address,
    name: String,
    symbol: String,
    decimals: u8,
    supply: U256,
    held: Option<U256>,
}

impl Token {
    fn amount(&self, units: U256) -> String {
        erc20::amount(units, self.decimals).map(|amount| format!("{amount} {}", self.symbol)).unwrap_or_default()
    }
}

/// Five reads in one call to Multicall3.
async fn token(at: Address, me: Option<Address>) -> Out<Token> {
    let (name, symbol, decimals, supply) =
        (erc20::name(at), erc20::symbol(at), erc20::decimals(at), erc20::total_supply(at));
    let held = erc20::balance(at, me.unwrap_or(Address::ZERO));
    let calls =
        [Call3::of(&name, false), Call3::of(&symbol, false), Call3::of(&decimals, false), Call3::of(&supply, false)];
    let mut calls = calls.to_vec();
    calls.push(Call3::of(&held, true));
    let outcomes = Chain::Elysium.reader().read(&aggregate3(&calls)).await?;
    let mut answers = outcomes.iter();
    let mut next = || answers.next().ok_or("fewer answers than calls");
    Ok(Token {
        at,
        name: name.decode(&next()?.data)?,
        symbol: symbol.decode(&next()?.data)?,
        decimals: decimals.decode(&next()?.data)?,
        supply: supply.decode(&next()?.data)?,
        held: match (me, next()?) {
            (Some(_), outcome) if outcome.success => Some(held.decode(&outcome.data)?),
            _ => None,
        },
    })
}

#[component]
pub fn TokenPanel(ctx: Ctx) -> impl IntoView {
    let address = RwSignal::new(USDC.to_owned());
    let loaded = poll(ctx, move || {
        let at = address.get().trim().parse().ok()?;
        Some(token(at, ctx.me()))
    });
    let to = RwSignal::new(String::new());
    let amount = RwSignal::new("1.5".to_owned());
    let status = RwSignal::new(Status::Idle);

    let units = move || -> Out<(Token, Address, U256)> {
        let token = loaded.get_untracked().ok_or("the token has not loaded")??;
        let to = to.get_untracked().trim().parse()?;
        let units = erc20::units(amount.get_untracked().trim().parse()?, token.decimals)?;
        Ok((token, to, units))
    };
    let send = move |_| {
        run(status, "transferring", async move {
            let (token, to, units) = units()?;
            let receipt =
                transact(ctx, status, Chain::Elysium, "transfer", erc20::transfer(token.at, to, units)).await?;
            let note = format!("sent {}", token.amount(units));
            Ok(Status::sent(note, Chain::Elysium, receipt.transaction_hash))
        })
    };
    let approve = move |_| {
        run(status, "approving", async move {
            let (token, spender, units) = units()?;
            let call = erc20::approve(token.at, spender, units);
            let receipt = transact(ctx, status, Chain::Elysium, "approve", call).await?;
            let me = ctx.sender()?.address();
            let allowed = Chain::Elysium.reader().read(&erc20::allowance(token.at, me, spender)).await?;
            let note = format!("{spender} may move {} of yours", token.amount(allowed));
            Ok(Status::sent(note, Chain::Elysium, receipt.transaction_hash))
        })
    };
    let create = move |_| {
        run(status, "deploying ETEST", async move {
            let me = ctx.sender()?.address();
            let supply = erc20::units("1000000".parse()?, 6)?;
            let creation = deploy::create(&hex::decode(TEST_TOKEN.trim())?, &supply.to_be_bytes());
            let expected = create_address(me, Chain::Elysium.reader().nonce(me).await?);
            let receipt = transact(ctx, status, Chain::Elysium, "deploy ETEST", creation).await?;
            let made = receipt.contract_address.unwrap_or(expected);
            address.set(made.to_string());
            Ok(Status::sent(format!("ETEST at {made}, a million yours"), Chain::Elysium, receipt.transaction_hash))
        })
    };

    view! {
        <Panel title="ERC-20">
            <Field label="Token" value=address placeholder="0x..." />
            {move || {
                loaded
                    .get()
                    .and_then(Result::ok)
                    .map(|token| {
                        view! {
                            <Row label="contract">
                                <AddressLink address=token.at />
                            </Row>
                        }
                    })
            }}
            <Row label="name">{text(loaded, |token: &Token| format!("{} ({})", token.name, token.symbol))}</Row>
            <Row label="decimals">{text(loaded, |token: &Token| token.decimals.to_string())}</Row>
            <Row label="supply">{text(loaded, |token: &Token| token.amount(token.supply))}</Row>
            <Row label="yours">
                {text(loaded, |token: &Token| token.held.map(|held| token.amount(held)).unwrap_or_else(|| "-".to_owned()))}
            </Row>
            {problem(loaded)}
            <Part title="Transfer or approve" />
            <Field label="To, or the spender" value=to placeholder="0x..." />
            <Field label="Amount" value=amount />
            <Actions>
                <button class="button primary" on:click=send>"Transfer"</button>
                <button class="button" on:click=approve>"Approve"</button>
                <button class="button" on:click=create>"Deploy ETEST"</button>
            </Actions>
            <StatusLine status=status />
        </Panel>
    }
}
