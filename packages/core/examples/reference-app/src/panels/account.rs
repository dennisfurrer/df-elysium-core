//! The account: a burner key the page makes and keeps, one pasted in, or the browser's wallet; its
//! HYPE on both chains, its next nonce, and where to get testnet HYPE.

use df_elysium_core::{Address, Dec};
use leptos::prelude::*;

use crate::net::{Account, Chain, NETWORK};
use crate::route::{Route, go};
use crate::state::{Ctx, Out, Status, poll, problem, run, text};
use crate::ui::{Actions, AddressLink, Card, Entry, Field, Panel, Row, StatusLine};
use crate::{keys, wallet};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Holdings {
    elysium: Dec,
    hyperevm: Dec,
    nonce: u64,
}

async fn holdings(me: Address) -> Out<Holdings> {
    let elysium = Chain::Elysium.reader();
    Ok(Holdings {
        elysium: elysium.hype(me).await?,
        hyperevm: Chain::HyperEvm.reader().hype(me).await?,
        nonce: elysium.nonce(me).await?,
    })
}

/// The account on the home screen: who signs, what it holds, and the ways to set it.
#[component]
pub fn AccountCard(ctx: Ctx) -> impl IntoView {
    let held = poll(ctx, Some(Route::Home), move || ctx.me().map(holdings));
    let status = RwSignal::new(Status::Idle);

    let make = move |_| match keys::generate() {
        Ok(burner) => {
            ctx.account.set(Some(Account::Burner(burner)));
            status.set(Status::Done("a new burner key, kept in this browser: fund it from the faucet".to_owned()));
        }
        Err(error) => status.set(Status::Failed(error.to_string())),
    };
    let connect = move |_| {
        run(status, "asking the wallet", async move {
            let address = wallet::connect().await?;
            ctx.account.set(Some(Account::Wallet(address)));
            Ok(Status::Done("wallet connected: it signs, the SDK prices".to_owned()))
        })
    };
    let copy = move |_| {
        run(status, "copying", async move {
            keys::copy().await?;
            Ok(Status::Done("the key is on the clipboard".to_owned()))
        })
    };
    let forget = move |_| {
        let burner = matches!(ctx.account.get_untracked(), Some(Account::Burner(_)));
        if burner && !keys::confirm("Forget the burner key? Copy it first if it holds anything.") {
            return;
        }
        if burner {
            keys::forget();
        }
        ctx.account.set(None);
        status.set(Status::Idle);
    };
    let sent = move || ctx.activity.with(Vec::len);

    view! {
        <Card title="Account" note="Signs everything on this page: a burner key kept in this browser, or your wallet.">
            {move || match ctx.account.get() {
                None => {
                    view! {
                        <Actions>
                            <button class="button primary" on:click=make>"New burner key"</button>
                            <button class="button" on:click=connect disabled=!wallet::present()>"Connect wallet"</button>
                            <Entry route=Route::Import label="Import a key" />
                        </Actions>
                    }
                        .into_any()
                }
                Some(account) => {
                    let (burner, address, kind) =
                        (matches!(account, Account::Burner(_)), account.address(), account.kind());
                    view! {
                        <Row label="address">
                            <AddressLink address=address />
                        </Row>
                        <Row label="signs">{kind}</Row>
                        <Row label="HYPE on Elysium">{text(held, |held: &Holdings| held.elysium.to_string())}</Row>
                        <Row label="HYPE on HyperEVM">{text(held, |held: &Holdings| held.hyperevm.to_string())}</Row>
                        <Row label="next nonce">{text(held, |held: &Holdings| held.nonce.to_string())}</Row>
                        {problem(held)}
                        <Actions>
                            <a class="button" href=NETWORK.faucet_url() target="_blank" rel="noopener">"Faucet"</a>
                            <a class="button" href=Route::Activity.href()>{move || format!("Activity ({})", sent())}</a>
                            {burner.then(|| view! { <button class="button" on:click=copy>"Copy key"</button> })}
                            <button class="button" on:click=forget>{if burner { "Forget key" } else { "Disconnect" }}</button>
                        </Actions>
                    }
                        .into_any()
                }
            }}
            <StatusLine status=status />
        </Card>
    }
}

/// A key pasted in, kept as the burner.
#[component]
pub fn ImportPanel(ctx: Ctx) -> impl IntoView {
    let secret = RwSignal::new(String::new());
    let status = RwSignal::new(Status::Idle);
    let import = move |_| match keys::import(&secret.get_untracked()) {
        Ok(burner) => {
            secret.set(String::new());
            ctx.account.set(Some(Account::Burner(burner)));
            status.set(Status::Idle);
            go(Route::Home);
        }
        Err(error) => status.set(Status::Failed(error.to_string())),
    };

    view! {
        <Panel title="Import a key">
            <p class="note">"Kept in this browser's local storage, as the burner key is: testnet keys only."</p>
            <Field label="Private key" value=secret placeholder="0x..." secret=true />
            <Actions>
                <button class="button primary" on:click=import>"Import"</button>
            </Actions>
            <StatusLine status=status />
        </Panel>
    }
}
