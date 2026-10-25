//! The reference app for df-elysium-core: every part of the SDK in one page, in the browser, on
//! Elysium testnet. Leptos renders it in the browser; the SDK reads and sends through `fetch`.
//!
//! - [`net`]: the two chains, the account that signs, one send for a burner key or a wallet.
//! - [`wallet`]: the browser's wallet over EIP-1193, without a library.
//! - [`keys`]: the burner key, kept in this browser.
//! - [`route`]: the home screen or one tool, named in the URL's hash.
//! - [`state`]: what the screens share, and how they read and send.
//! - [`panels`]: the home screen's cards, and one screen per tool.

mod keys;
mod net;
mod panels;
mod route;
mod state;
mod ui;
mod wallet;

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::net::{Account, short};
use crate::panels::account::ImportPanel;
use crate::panels::activity::ActivityPanel;
use crate::panels::bridge::{ClaimsPanel, DepositPanel, WithdrawPanel};
use crate::panels::chain::ChainPanel;
use crate::panels::deploy::{DeployPanel, VerifyPanel};
use crate::panels::home::Home;
use crate::panels::inspect::InspectPanel;
use crate::panels::live::LivePanel;
use crate::panels::retryable::{TicketPanel, TicketsPanel};
use crate::panels::send::SendPanel;
use crate::panels::token::TokenPanel;
use crate::route::Route;
use crate::state::{Ctx, Head, text};

/// Polled reads run again this often: the public node has no websocket to push them.
const TICK_MS: i32 = 4_000;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

/// Every screen is mounted once and the others hidden, so a send keeps going when you leave its
/// screen and its outcome is there when you come back; a hidden screen reads nothing.
#[component]
fn App() -> impl IntoView {
    let ctx = Ctx::new(keys::stored().map(Account::Burner));
    let _ = window_event_listener(ev::hashchange, move |_| {
        ctx.route.set(Route::current());
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
    });
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            net::sleep(TICK_MS).await;
            ctx.refresh();
        }
    });
    view! {
        <TopBar ctx=ctx />
        <main>
            <Screen ctx=ctx route=Route::Home>
                <Home ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Import>
                <ImportPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Activity>
                <ActivityPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Send>
                <SendPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Token>
                <TokenPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Deposit>
                <DepositPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Withdraw>
                <WithdrawPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Claims>
                <ClaimsPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Ticket>
                <TicketPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Tickets>
                <TicketsPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Deploy>
                <DeployPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Verify>
                <VerifyPanel />
            </Screen>
            <Screen ctx=ctx route=Route::Chain>
                <ChainPanel ctx=ctx />
            </Screen>
            <Screen ctx=ctx route=Route::Inspect>
                <InspectPanel />
            </Screen>
            <Screen ctx=ctx route=Route::Live>
                <LivePanel />
            </Screen>
        </main>
    }
}

/// One screen, shown while the URL names it; a tool's has a way back home.
#[component]
fn Screen(ctx: Ctx, route: Route, children: Children) -> impl IntoView {
    let home = route == Route::Home;
    view! {
        <div class=if home { "screen home" } else { "screen tool" } class:hidden=move || !ctx.showing(route)>
            {(!home).then(|| view! { <a class="back" href=Route::Home.href()>"All tools"</a> })}
            {children()}
        </div>
    }
}

#[component]
fn TopBar(ctx: Ctx) -> impl IntoView {
    view! {
        <header class="top">
            <a class="brand" href=Route::Home.href()>
                <span class="brand-name">"Elysium"</span>
                <span class="tag">"testnet"</span>
            </a>
            <div class="top-right mono">
                <span class="dim">{text(ctx.head, |head: &Head| format!("block {}", head.number))}</span>
                <span>{move || ctx.me().map(|me| short(&me.to_string())).unwrap_or_else(|| "no account".to_owned())}</span>
            </div>
        </header>
    }
}
