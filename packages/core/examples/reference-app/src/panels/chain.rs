//! Elysium as it says of itself: its head and the HyperEVM block it is at, ArbOS, the base fee and
//! HyperEVM's data price; its limits; and how far the rollup has confirmed the outbox on HyperEVM,
//! with the HYPE the bridge holds there.

use df_elysium_core::bridge::claim;
use df_elysium_core::nitro::{gas_info, retryable};
use df_elysium_core::{Dec, U256};
use leptos::prelude::*;

use crate::net::{Chain, NETWORK};
use crate::route::Route;
use crate::state::{Ctx, Head, Out, follow, problem, text};
use crate::ui::{Actions, Card, Entry, Panel, Part, Row, gwei};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Limits {
    tx: U256,
    block: u64,
    floor: U256,
    ticket_lifetime: U256,
}

async fn limits() -> Out<Limits> {
    let client = Chain::Elysium.reader();
    Ok(Limits {
        tx: client.read(&gas_info::max_tx_gas_limit()).await?,
        block: client.read(&gas_info::max_block_gas_limit()).await?,
        floor: client.read(&gas_info::minimum_gas_price()).await?,
        ticket_lifetime: client.read(&retryable::lifetime()).await?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outbox {
    block: u64,
    sends: u64,
    locked: Dec,
}

/// The latest assertion the rollup confirmed on HyperEVM, and the Elysium block it confirms.
async fn outbox() -> Out<Outbox> {
    let contracts = NETWORK.contracts();
    let parent = Chain::HyperEvm.reader();
    let confirmed = claim::latest(&contracts, parent.rpc()).await?;
    let block = Chain::Elysium.reader().block_by_hash(confirmed.block_hash, false).await?;
    let block = block.ok_or("Elysium does not know the confirmed block")?;
    Ok(Outbox { block: block.number, sends: block.send_count, locked: parent.hype(contracts.bridge).await? })
}

/// The chain on the home screen: its head and fees, and the tools that look at it.
#[component]
pub fn ChainCard(ctx: Ctx) -> impl IntoView {
    let latest = ctx.head;
    view! {
        <Card title="Chain" note="Elysium testnet as it says of itself, any transaction on either chain, and blocks as they come.">
            <Row label="head">{text(latest, |head: &Head| head.number.to_string())}</Row>
            <Row label="base fee">{text(latest, |head: &Head| gwei(head.base_fee))}</Row>
            <Row label="ArbOS">{text(latest, |head: &Head| head.arbos.to_string())}</Row>
            <Actions>
                <Entry route=Route::Chain label="Chain and gas" />
                <Entry route=Route::Inspect label="Inspect a transaction" />
                <Entry route=Route::Live label="Live blocks" />
            </Actions>
        </Card>
    }
}

#[component]
pub fn ChainPanel(ctx: Ctx) -> impl IntoView {
    let latest = ctx.head;
    let limits = follow(ctx, Some(Route::Chain), || Some(limits()));
    // Once a minute is plenty: the rollup confirms an assertion every few minutes.
    let minute = Memo::new(move |_| ctx.tick.get() / 15);
    let confirmed = follow(ctx, Some(Route::Chain), move || {
        minute.track();
        Some(outbox())
    });

    view! {
        <Panel title="Chain and gas">
            <Row label="chain">{format!("{} ({})", NETWORK.chain_id(), NETWORK.name())}</Row>
            <Row label="head">{text(latest, |head: &Head| head.number.to_string())}</Row>
            <Row label="at HyperEVM block">{text(latest, |head: &Head| head.parent.to_string())}</Row>
            <Row label="ArbOS">{text(latest, |head: &Head| head.arbos.to_string())}</Row>
            <Row label="base fee">{text(latest, |head: &Head| gwei(head.base_fee))}</Row>
            <Row label="HyperEVM data price">{text(latest, |head: &Head| gwei(head.data_price))}</Row>
            {problem(latest)}
            <Part title="Limits" />
            <Row label="gas a transaction">{text(limits, |limits: &Limits| limits.tx.to_string())}</Row>
            <Row label="gas a block">{text(limits, |limits: &Limits| limits.block.to_string())}</Row>
            <Row label="fee floor">{text(limits, |limits: &Limits| gwei(limits.floor))}</Row>
            <Row label="ticket lifetime">{text(limits, |limits: &Limits| format!("{} s", limits.ticket_lifetime))}</Row>
            {problem(limits)}
            <Part title="Outbox" />
            <Row label="sends at the head">{text(latest, |head: &Head| head.sends.to_string())}</Row>
            <Row label="confirmed on HyperEVM">
                {text(confirmed, |outbox: &Outbox| format!("{} sends, block {}", outbox.sends, outbox.block))}
            </Row>
            <Row label="blocks behind">
                {move || match (latest.get(), confirmed.get()) {
                    (Some(Ok(head)), Some(Ok(outbox))) => head.number.saturating_sub(outbox.block).to_string(),
                    _ => "...".to_owned(),
                }}
            </Row>
            <Row label="HYPE in the bridge">{text(confirmed, |outbox: &Outbox| outbox.locked.to_string())}</Row>
            {problem(confirmed)}
        </Panel>
    }
}
