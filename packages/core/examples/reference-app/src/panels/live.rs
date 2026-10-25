//! Elysium as it moves, polled: each new block ([`Heads`]) and each withdrawal to HyperEVM as
//! ArbSys logs it ([`LogCursor`], a couple of blocks behind the head).

use df_elysium_core::bridge::Withdrawal;
use df_elysium_core::client::rpc::lagging;
use df_elysium_core::client::{BlockTag, BlockTransactions, LogFilter};
use df_elysium_core::live::{Heads, LogCursor};
use df_elysium_core::nitro::{ARB_SYS, sys};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::net::{Chain, NETWORK, short, sleep};
use crate::state::{Out, Status};
use crate::ui::{Actions, Panel, StatusLine, hype_text};

/// Blocks a withdrawal log is read behind the head.
const CONFIRMATIONS: u64 = 2;
/// The most blocks read in one poll; the rest are counted, not read.
const MOST_BLOCKS: u64 = 8;
/// The lines kept.
const KEPT: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    class: &'static str,
    text: String,
}

async fn follow(running: RwSignal<bool>, lines: RwSignal<Vec<Line>>) -> Out<()> {
    let client = Chain::Elysium.reader();
    let start = client.block_number().await?;
    let mut heads = Heads::after(start);
    let withdrawals = LogFilter {
        address: vec![ARB_SYS],
        topics: vec![Some(vec![sys::L2_TO_L1_TX.topic()])],
        ..LogFilter::default()
    };
    let mut cursor = LogCursor::new(withdrawals, start.saturating_add(1), NETWORK.log_span(), CONFIRMATIONS);
    let say = |class: &'static str, text: String| {
        lines.update(|lines| {
            lines.insert(0, Line { class, text });
            lines.truncate(KEPT);
        })
    };
    say("dim", format!("following from block {start}"));
    while running.get_untracked() {
        let head = match client.block_number().await {
            Ok(head) => head,
            Err(error) if lagging(&error) => {
                sleep(1_000).await;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(new) = heads.seen(head) {
            let first = new.end().saturating_sub(MOST_BLOCKS - 1).max(*new.start());
            if first > *new.start() {
                say("dim", format!("{} blocks skipped", first - new.start()));
            }
            for number in first..=*new.end() {
                let Some(block) = client.block(BlockTag::Number(number), false).await? else { continue };
                let count = match &block.transactions {
                    BlockTransactions::Hashes(hashes) => hashes.len(),
                    BlockTransactions::Full(transactions) => transactions.len(),
                };
                say(
                    "",
                    format!(
                        "block {number}  {count} tx  HyperEVM {}  {} sends",
                        block.l1_block_number, block.send_count
                    ),
                );
            }
        }
        if let Some(window) = cursor.window(head) {
            for log in client.logs(&window).await? {
                let sent = Withdrawal::from_log(&log)?;
                say(
                    "warn",
                    format!(
                        "withdrawal  leaf {}  {} to {}",
                        sent.position,
                        hype_text(sent.value),
                        short(&sent.destination.to_string())
                    ),
                );
            }
            cursor.read(&window);
        }
        sleep(1_000).await;
    }
    say("dim", "stopped".to_owned());
    Ok(())
}

#[component]
pub fn LivePanel() -> impl IntoView {
    let running = RwSignal::new(false);
    let lines = RwSignal::new(Vec::<Line>::new());
    let status = RwSignal::new(Status::Idle);
    let toggle = move |_| {
        if running.get_untracked() {
            running.set(false);
            return;
        }
        running.set(true);
        status.set(Status::Idle);
        spawn_local(async move {
            if let Err(error) = follow(running, lines).await {
                status.set(Status::Failed(error.to_string()));
            }
            running.set(false);
        });
    };

    view! {
        <Panel title="Live">
            <Actions>
                <button class="button" class:primary=move || !running.get() on:click=toggle>
                    {move || if running.get() { "Stop" } else { "Follow blocks and withdrawals" }}
                </button>
            </Actions>
            <StatusLine status=status />
            <div class="feed mono">
                {move || {
                    lines
                        .get()
                        .into_iter()
                        .map(|line| view! { <div class=line.class>{line.text}</div> })
                        .collect_view()
                }}
            </div>
        </Panel>
    }
}
