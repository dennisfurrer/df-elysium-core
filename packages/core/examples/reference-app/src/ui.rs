//! The page's parts: a panel, a labelled value, an input, a row of buttons, an action's status,
//! links to the explorer, and amounts as a person reads them.

use df_elysium_core::transfer::erc20;
use df_elysium_core::utils::hype;
use df_elysium_core::{Address, B256, U256};
use leptos::prelude::*;

use crate::net::{Chain, NETWORK, short};
use crate::route::Route;
use crate::state::Status;

#[component]
pub fn Panel(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="panel">
            <h2 class="panel-title">{title}</h2>
            <div class="panel-body">{children()}</div>
        </section>
    }
}

/// A card on the home screen: what a group of tools does, then its entry points.
#[component]
pub fn Card(title: &'static str, note: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="panel card">
            <h2 class="panel-title">{title}</h2>
            <div class="panel-body">
                <p class="note">{note}</p>
                {children()}
            </div>
        </section>
    }
}

/// A button that opens a tool.
#[component]
pub fn Entry(route: Route, label: &'static str, #[prop(optional)] primary: bool) -> impl IntoView {
    view! {
        <a class="button" class:primary=primary href=route.href()>
            {label}
        </a>
    }
}

/// A heading inside a panel.
#[component]
pub fn Part(title: &'static str) -> impl IntoView {
    view! { <h3 class="part">{title}</h3> }
}

#[component]
pub fn Row(label: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="row">
            <span class="row-label">{label}</span>
            <span class="row-value">{children()}</span>
        </div>
    }
}

#[component]
pub fn Field(
    label: &'static str,
    value: RwSignal<String>,
    #[prop(optional)] placeholder: &'static str,
    #[prop(optional)] secret: bool,
) -> impl IntoView {
    view! {
        <label class="field">
            <span class="field-label">{label}</span>
            <input
                class="input"
                type=if secret { "password" } else { "text" }
                spellcheck="false"
                autocomplete="off"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |event| value.set(event_target_value(&event))
            />
        </label>
    }
}

#[component]
pub fn Actions(children: Children) -> impl IntoView {
    view! { <div class="actions">{children()}</div> }
}

#[component]
pub fn StatusLine(status: RwSignal<Status>) -> impl IntoView {
    move || match status.get() {
        Status::Idle => ().into_any(),
        Status::Busy(text) => view! { <p class="status busy">{text}</p> }.into_any(),
        Status::Done(text) => view! { <p class="status ok">{text}</p> }.into_any(),
        Status::Sent { note, chain, hash } => {
            view! { <p class="status ok">{note} " " <TxLink chain=chain hash=hash /></p> }.into_any()
        }
        Status::Failed(text) => view! { <p class="status crit">{text}</p> }.into_any(),
    }
}

/// A transaction, linked to Elysium's explorer; HyperEVM's in full, to copy.
#[component]
pub fn TxLink(chain: Chain, hash: B256) -> impl IntoView {
    let full = hash.to_string();
    match chain.tx_url(hash) {
        Some(url) => {
            view! { <a class="mono" href=url target="_blank" rel="noopener" title=full.clone()>{short(&full)}</a> }
                .into_any()
        }
        None => {
            let title = full.clone();
            view! { <span class="mono select" title=title>{full}</span> }.into_any()
        }
    }
}

/// An account or contract on Elysium's explorer.
#[component]
pub fn AddressLink(address: Address) -> impl IntoView {
    let full = address.to_string();
    view! {
        <a class="mono" href=NETWORK.address_url(address) target="_blank" rel="noopener" title=full.clone()>
            {short(&full)}
        </a>
    }
}

/// `1.5 HYPE` from wei, exactly.
pub fn hype_text(wei: U256) -> String {
    hype(wei).map(|hype| format!("{hype} HYPE")).unwrap_or_else(|_| format!("{wei} wei"))
}

/// `0.02 gwei` from wei, exactly.
pub fn gwei(wei: U256) -> String {
    erc20::amount(wei, 9).map(|gwei| format!("{gwei} gwei")).unwrap_or_else(|_| format!("{wei} wei"))
}

/// Unix seconds as UTC, to the minute.
pub fn when(unix: u64) -> String {
    let ms = wasm_bindgen::JsValue::from_f64(unix.saturating_mul(1_000) as f64);
    let iso = String::from(js_sys::Date::new(&ms).to_iso_string());
    format!("{} UTC", iso.get(..16).unwrap_or(&iso).replace('T', " "))
}
