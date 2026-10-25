//! HYPE between HyperEVM and Elysium.
//!
//! In: `Inbox.depositEth()` on HyperEVM, simulated first for the message number it gets and the
//! hash its transaction will have on Elysium, then followed until it lands there.
//!
//! Out: `ArbSys.withdrawEth` on Elysium burns the HYPE and makes a leaf of the outbox's tree. Each
//! withdrawal to this account is listed with where its claim stands; once the rollup confirms it on
//! HyperEVM, the claim (`Outbox.executeTransaction`) is sent there and pays the HYPE out.

use df_elysium_core::bridge::deposit::DEPOSIT_ETH;
use df_elysium_core::bridge::{self, Claim, Deposit, Message, Withdrawal, apply_alias};
use df_elysium_core::client::{BlockTag, Call, CallRequest, LogFilter};
use df_elysium_core::nitro::{ARB_SYS, sys};
use df_elysium_core::{Address, B256};
use leptos::prelude::*;

use crate::net::{Chain, HYPE_SYSTEM, NETWORK, short, sleep};
use crate::route::Route;
use crate::state::{Ctx, Out, Status, follow, problem, run, transact};
use crate::ui::{Actions, Entry, Field, Panel, Part, StatusLine, hype_text};

/// How many of the account's withdrawals are listed, newest first.
const LISTED: usize = 6;

/// The deposits a HyperEVM transaction made, and where each landed on Elysium.
async fn landings(hash: B256) -> Out<Vec<(Deposit, B256)>> {
    let receipt = Chain::HyperEvm.reader().receipt(hash).await?.ok_or("HyperEVM does not know that transaction")?;
    let messages = Message::in_logs(&receipt.logs, &NETWORK.contracts())?;
    let deposits: Vec<_> = messages
        .iter()
        .filter_map(Deposit::from_message)
        .map(|deposit| (deposit, deposit.tx_hash(NETWORK.chain_id())))
        .collect();
    match deposits.is_empty() {
        true => Err("it deposited nothing to Elysium".into()),
        false => Ok(deposits),
    }
}

/// Waits up to two minutes for `hash` on Elysium: a deposit lands a block or two after HyperEVM's.
async fn landed(hash: B256) -> Out<bool> {
    let elysium = Chain::Elysium.reader();
    for _ in 0..120 {
        if let Some(receipt) = elysium.receipt(hash).await? {
            return Ok(receipt.succeeded());
        }
        sleep(1_000).await;
    }
    Err(format!("{hash} has not landed on Elysium after two minutes").into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Standing {
    Spent,
    Waiting { confirmed: u64 },
    Ready(Call),
}

/// The account's latest withdrawals, each with where its claim stands.
async fn withdrawals(me: Address) -> Out<Vec<(Withdrawal, Standing)>> {
    let elysium = Chain::Elysium.reader();
    let parent = Chain::HyperEvm.reader();
    // Named by address and topic, so the node takes the whole chain in one query.
    let filter = LogFilter {
        from_block: Some(BlockTag::Earliest),
        to_block: Some(BlockTag::Latest),
        address: vec![ARB_SYS],
        topics: vec![Some(vec![sys::L2_TO_L1_TX.topic()]), Some(vec![B256::from_address(me)])],
        ..LogFilter::default()
    };
    let logs = elysium.logs(&filter).await?;
    let mut listed = Vec::new();
    for log in logs.iter().rev().take(LISTED) {
        let withdrawal = Withdrawal::from_log(log)?;
        let standing = match bridge::claim(&elysium, parent.rpc(), &withdrawal).await? {
            Claim::Spent => Standing::Spent,
            Claim::Waiting { confirmed } => Standing::Waiting { confirmed },
            Claim::Ready { call, .. } => Standing::Ready(call),
        };
        listed.push((withdrawal, standing));
    }
    Ok(listed)
}

/// HYPE in from HyperEVM: simulated, deposited and followed until it lands; or any deposit followed
/// from its HyperEVM transaction.
#[component]
pub fn DepositPanel(ctx: Ctx) -> impl IntoView {
    let amount = RwSignal::new("0.001".to_owned());
    let track = RwSignal::new(String::new());
    let status = RwSignal::new(Status::Idle);

    let simulate = move |_| {
        run(status, "simulating on HyperEVM", async move {
            let me = ctx.sender()?.address();
            let call = bridge::deposit(NETWORK, amount.get_untracked().trim().parse()?)?;
            let parent = Chain::HyperEvm.reader();
            let funded = parent.balance(me).await? > call.value;
            let sender = if funded { me } else { HYPE_SYSTEM };
            let request = CallRequest::of(Some(sender), &call);
            let answer = parent.rpc().call(&request, BlockTag::Latest).await?;
            let number = DEPOSIT_ETH.answer(&answer)?.first().and_then(|token| token.as_u256());
            let number = number.ok_or("depositEth answered no message number")?;
            let gas = parent.rpc().estimate_gas(&request).await?;
            let ahead = Deposit { message_number: number, from: apply_alias(sender), to: sender, value: call.value };
            let from =
                if funded { String::new() } else { format!(", from {} as you hold none", short(&sender.to_string())) };
            Ok(Status::Done(format!(
                "message {number}, {gas} gas on HyperEVM{from}; lands as {} if sent now",
                short(&ahead.tx_hash(NETWORK.chain_id()).to_string())
            )))
        })
    };
    let deposit = move |_| {
        run(status, "depositing", async move {
            let call = bridge::deposit(NETWORK, amount.get_untracked().trim().parse()?)?;
            let receipt = transact(ctx, status, Chain::HyperEvm, "deposit", call).await?;
            let mut last = None;
            for (deposit, hash) in landings(receipt.transaction_hash).await? {
                status.set(Status::Busy(format!("message {}: waiting for Elysium", deposit.message_number)));
                landed(hash).await?;
                last = Some(hash);
            }
            let hash = last.ok_or("it deposited nothing")?;
            Ok(Status::sent("landed on Elysium", Chain::Elysium, hash))
        })
    };
    let follow_deposit = move |_| {
        run(status, "following the deposit", async move {
            let hash: B256 = track.get_untracked().trim().parse()?;
            let mut last = None;
            for (deposit, hash) in landings(hash).await? {
                status.set(Status::Busy(format!("message {}: waiting for Elysium", deposit.message_number)));
                landed(hash).await?;
                last = Some((deposit, hash));
            }
            let (deposit, hash) = last.ok_or("it deposited nothing")?;
            let note = format!("{} to {} landed", hype_text(deposit.value), short(&deposit.to.to_string()));
            Ok(Status::sent(note, Chain::Elysium, hash))
        })
    };

    view! {
        <Panel title="Deposit from HyperEVM">
            <p class="note">"Inbox.depositEth() on HyperEVM; it lands on Elysium a block or two later. Gas is paid on HyperEVM."</p>
            <Field label="Amount, HYPE" value=amount />
            <Actions>
                <button class="button" on:click=simulate>"Simulate"</button>
                <button class="button primary" on:click=deposit>"Deposit"</button>
            </Actions>
            <Part title="Follow a deposit" />
            <Field label="HyperEVM transaction" value=track placeholder="0x... its hash" />
            <Actions>
                <button class="button" on:click=follow_deposit>"Follow"</button>
            </Actions>
            <StatusLine status=status />
        </Panel>
    }
}

/// HYPE out to HyperEVM: burnt here at once, claimable there once the rollup confirms it.
#[component]
pub fn WithdrawPanel(ctx: Ctx) -> impl IntoView {
    let to = RwSignal::new(String::new());
    let amount = RwSignal::new("0.0001".to_owned());
    let status = RwSignal::new(Status::Idle);
    let withdraw = move |_| {
        run(status, "withdrawing", async move {
            let me = ctx.sender()?.address();
            let to = match to.get_untracked().trim() {
                "" => me,
                to => to.parse()?,
            };
            let call = bridge::withdraw(to, amount.get_untracked().trim().parse()?)?;
            let receipt = transact(ctx, status, Chain::Elysium, "withdraw", call).await?;
            let leaves: Vec<String> =
                Withdrawal::in_logs(&receipt.logs)?.iter().map(|sent| sent.position.to_string()).collect();
            let note = format!("leaf {}: claimable on HyperEVM once confirmed", leaves.join(", "));
            Ok(Status::sent(note, Chain::Elysium, receipt.transaction_hash))
        })
    };

    view! {
        <Panel title="Withdraw to HyperEVM">
            <p class="note">"ArbSys.withdrawEth burns the HYPE here and makes a leaf of the outbox. It is claimable on HyperEVM once the rollup confirms it; Chain and gas shows how far behind that runs."</p>
            <Field label="To on HyperEVM" value=to placeholder="this account" />
            <Field label="Amount, HYPE" value=amount />
            <Actions>
                <button class="button primary" on:click=withdraw>"Withdraw"</button>
                <Entry route=Route::Claims label="Claims" />
            </Actions>
            <StatusLine status=status />
        </Panel>
    }
}

/// The account's withdrawals, where each claim stands, and the claim once it can be made.
#[component]
pub fn ClaimsPanel(ctx: Ctx) -> impl IntoView {
    let status = RwSignal::new(Status::Idle);
    let reload = RwSignal::new(0u32);
    let again = move || reload.update(|count| *count = count.wrapping_add(1));
    let listed = follow(ctx, Some(Route::Claims), move || {
        reload.track();
        ctx.me().map(withdrawals)
    });
    let claim = move |call: Call, position: u64| {
        run(status, format!("claiming leaf {position}"), async move {
            let receipt = transact(ctx, status, Chain::HyperEvm, "claim", call).await?;
            again();
            Ok(Status::sent(format!("leaf {position} paid out"), Chain::HyperEvm, receipt.transaction_hash))
        })
    };

    view! {
        <Panel title="Claim withdrawals">
            <p class="note">"Withdrawals to this account, newest first. A claim runs Outbox.executeTransaction on HyperEVM and pays gas there."</p>
            {move || listed.with(|listed| listed.is_none()).then(|| view! { <p class="status busy">"reading the outbox"</p> })}
            {problem(listed)}
            {move || {
                listed
                    .get()
                    .and_then(Result::ok)
                    .map(|list| {
                        if list.is_empty() {
                            return view! { <p class="note">"No withdrawals to this account yet."</p> }.into_any();
                        }
                        list.into_iter()
                            .map(|(withdrawal, standing)| {
                                let label = format!("leaf {}", withdrawal.position);
                                let what = hype_text(withdrawal.value);
                                let position = withdrawal.position;
                                let state = match standing {
                                    Standing::Spent => view! { <span class="ok">"claimed"</span> }.into_any(),
                                    Standing::Waiting { confirmed } => {
                                        let more = position.saturating_add(1).saturating_sub(confirmed);
                                        view! { <span class="warn">{format!("waiting, {more} leaves to go")}</span> }
                                            .into_any()
                                    }
                                    Standing::Ready(call) => {
                                        view! {
                                            <button class="button small primary" on:click=move |_| claim(call.clone(), position)>
                                                "Claim"
                                            </button>
                                        }
                                            .into_any()
                                    }
                                };
                                view! {
                                    <div class="entry">
                                        <span>{label}</span>
                                        <span>{what}</span>
                                        {state}
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any()
                    })
            }}
            <Actions>
                <button class="button" on:click=move |_| again()>"Refresh"</button>
            </Actions>
            <StatusLine status=status />
        </Panel>
    }
}
