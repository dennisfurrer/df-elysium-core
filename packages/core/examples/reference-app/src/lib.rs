//! The reference app for df-elysium-core: every part of the SDK in one page, in the browser, on
//! Elysium testnet. Leptos renders it in the browser; the SDK reads and sends through `fetch`.
//!
//! - [`net`]: the two chains, the account that signs, one send for a burner key or a wallet.
//! - [`wallet`]: the browser's wallet over EIP-1193, without a library.
//! - [`keys`]: the burner key, kept in this browser.
//! - [`state`]: what the panels share, and how they read and send.
//! - [`panels`]: one per part of the SDK.

mod keys;
mod net;
mod panels;
mod state;
mod ui;
mod wallet;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::net::{Account, short};
use crate::panels::account::AccountPanel;
use crate::panels::activity::ActivityPanel;
use crate::panels::bridge::BridgePanel;
use crate::panels::chain::ChainPanel;
use crate::panels::deploy::DeployPanel;
use crate::panels::inspect::InspectPanel;
use crate::panels::live::LivePanel;
use crate::panels::retryable::RetryablePanel;
use crate::panels::send::SendPanel;
use crate::panels::token::TokenPanel;
use crate::state::Ctx;

/// Polled reads run again this often: the public node has no websocket to push them.
const TICK_MS: i32 = 4_000;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let ctx = Ctx::new(keys::stored().map(Account::Burner));
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            net::sleep(TICK_MS).await;
            ctx.refresh();
        }
    });
    view! {
        <TopBar ctx=ctx />
        <main class="grid">
            <AccountPanel ctx=ctx />
            <ChainPanel ctx=ctx />
            <SendPanel ctx=ctx />
            <ActivityPanel ctx=ctx />
            <TokenPanel ctx=ctx />
            <BridgePanel ctx=ctx />
            <RetryablePanel ctx=ctx />
            <DeployPanel ctx=ctx />
            <InspectPanel />
            <LivePanel />
        </main>
    }
}

#[component]
fn TopBar(ctx: Ctx) -> impl IntoView {
    view! {
        <header class="top">
            <div class="brand">
                <span class="brand-name">"Elysium"</span>
                <span class="tag">"testnet"</span>
            </div>
            <div class="top-right mono">
                <span class="dim">{move || ctx.head.get().map(|head| format!("block {head}")).unwrap_or_default()}</span>
                <span>{move || ctx.me().map(|me| short(&me.to_string())).unwrap_or_else(|| "no account".to_owned())}</span>
            </div>
        </header>
    }
}
