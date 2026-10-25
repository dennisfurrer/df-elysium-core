//! A retryable ticket: a call on Elysium sent and paid for from HyperEVM. Built here (its
//! submission cost from HyperEVM's base fee, its gas from Elysium's NodeInterface, its fee cap from
//! Elysium's base fee), simulated on HyperEVM for its message number and ticket id, sent, and
//! followed on Elysium to its redeem. A ticket still live may be redeemed by anyone.

use df_elysium_core::bridge::retryable::{CREATE_RETRYABLE_TICKET, submission_fee};
use df_elysium_core::bridge::{Message, Retryable, Ticket, apply_alias};
use df_elysium_core::client::{BlockTag, Call, CallRequest, with_headroom};
use df_elysium_core::nitro::node::{RetryableEstimate, estimate_retryable_ticket};
use df_elysium_core::nitro::retryable::{self, REDEEM_SCHEDULED};
use df_elysium_core::utils::{Dec, hex, wei};
use df_elysium_core::{Address, B256, U256, gas};
use leptos::prelude::*;

use crate::net::{Chain, HYPE_SYSTEM, NETWORK, short, sleep};
use crate::state::{Ctx, Out, Status, run, transact};
use crate::ui::{Actions, Field, Panel, Part, StatusLine, TxLink, hype_text, when};

/// The ticket `me` sends to `to`, and the HyperEVM call that submits it.
async fn build(me: Address, to: Address, value: Dec, data: Vec<u8>) -> Out<(Retryable, Call)> {
    let elysium = Chain::Elysium.reader();
    let parent = Chain::HyperEvm.reader();
    // HyperEVM's base fee prices the submission; twice it leaves room for a rise before it lands.
    let fee = submission_fee(data.len(), parent.gas_price().await?);
    let estimate = RetryableEstimate {
        sender: me,
        deposit: wei(Dec::ONE)?,
        to,
        l2_call_value: wei(value)?,
        excess_fee_refund_address: me,
        call_value_refund_address: me,
        data: data.clone(),
    };
    let request = CallRequest::of(None, &estimate_retryable_ticket(&estimate));
    let estimated = elysium.rpc().estimate_gas(&request).await?;
    let ticket = Retryable {
        to,
        l2_call_value: wei(value)?,
        max_submission_cost: fee.saturating_mul(U256::from_u64(2)),
        excess_fee_refund_address: me,
        call_value_refund_address: me,
        gas_limit: U256::from_u64(with_headroom(estimated)),
        max_fee_per_gas: gas::fees(elysium.gas_price().await?).max_fee,
        data,
    };
    let call = ticket.call(NETWORK).ok_or("the ticket's deposit does not fit 256 bits")?;
    Ok((ticket, call))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Followed {
    number: U256,
    id: B256,
    to: Address,
    value: U256,
    /// `None` until Elysium has the ticket.
    submitted: Option<bool>,
    redeems: Vec<(B256, Option<bool>)>,
    /// Unix seconds; `None` once redeemed, cancelled or expired.
    live_until: Option<u64>,
}

/// The tickets a HyperEVM transaction submitted, each as Elysium has it.
async fn tickets(hash: B256) -> Out<Vec<Followed>> {
    let elysium = Chain::Elysium.reader();
    let receipt = Chain::HyperEvm.reader().receipt(hash).await?.ok_or("HyperEVM does not know that transaction")?;
    let mut followed = Vec::new();
    for message in Message::in_logs(&receipt.logs, &NETWORK.contracts())? {
        let Some(ticket) = Ticket::from_message(&message) else { continue };
        let id = ticket.id(NETWORK.chain_id());
        let submitted = elysium.receipt(id).await?;
        let mut redeems = Vec::new();
        for log in submitted.iter().flat_map(|receipt| &receipt.logs).filter(|log| REDEEM_SCHEDULED.matches(log)) {
            let tokens = REDEEM_SCHEDULED.decode(log)?;
            let redeem =
                tokens.get(1).and_then(|token| token.as_bytes()).and_then(|bytes| <[u8; 32]>::try_from(bytes).ok());
            let redeem = B256(redeem.ok_or("RedeemScheduled names no redeem")?);
            redeems.push((redeem, elysium.receipt(redeem).await?.map(|ran| ran.succeeded())));
        }
        let live_until = match elysium.read(&retryable::timeout(id)).await {
            Ok(timeout) => timeout.to_u64(),
            Err(error) if retryable::no_ticket(&error) => None,
            Err(error) => return Err(error.into()),
        };
        followed.push(Followed {
            number: message.number,
            id,
            to: ticket.retryable.to,
            value: ticket.retryable.l2_call_value,
            submitted: submitted.map(|receipt| receipt.succeeded()),
            redeems,
            live_until,
        });
    }
    match followed.is_empty() {
        true => Err("it submitted no ticket".into()),
        false => Ok(followed),
    }
}

/// The tickets `followed`, each with its redeems and a Redeem button while it is live; a redeem
/// reads them again from `source`, the HyperEVM transaction that submitted them.
fn listing(
    ctx: Ctx,
    status: RwSignal<Status>,
    followed: RwSignal<Vec<Followed>>,
    source: RwSignal<Option<B256>>,
) -> impl IntoView {
    let redeem = move |id: B256| {
        run(status, "redeeming", async move {
            let receipt = transact(ctx, status, Chain::Elysium, "redeem", retryable::redeem(id)).await?;
            if let Some(hash) = source.get_untracked() {
                followed.set(tickets(hash).await?);
            }
            Ok(Status::sent("redeemed", Chain::Elysium, receipt.transaction_hash))
        })
    };
    move || {
        followed
            .get()
            .into_iter()
            .map(|ticket| {
                let id = ticket.id;
                let redeemed = ticket.redeems.iter().any(|(_, ran)| *ran == Some(true));
                let state = match (ticket.submitted, redeemed, ticket.live_until) {
                    (None, _, _) => "not on Elysium yet".to_owned(),
                    (Some(false), _, _) => "submission failed".to_owned(),
                    (Some(true), true, _) => "redeemed".to_owned(),
                    (Some(true), false, Some(until)) => format!("live until {}", when(until)),
                    (Some(true), false, None) => "gone: cancelled or expired".to_owned(),
                };
                let can_redeem = ticket.submitted == Some(true) && !redeemed && ticket.live_until.is_some();
                view! {
                    <Part title="Ticket" />
                    <div class="entry">
                        <span>{format!("message {}", ticket.number)}</span>
                        <TxLink chain=Chain::Elysium hash=id />
                    </div>
                    <div class="entry">
                        <span class="dim">{format!("to {}", short(&ticket.to.to_string()))}</span>
                        <span>{hype_text(ticket.value)}</span>
                        <span>{state}</span>
                    </div>
                    {ticket
                        .redeems
                        .into_iter()
                        .map(|(hash, ran)| {
                            let (class, text) = match ran {
                                None => ("busy", "scheduled"),
                                Some(true) => ("ok", "ran"),
                                Some(false) => ("crit", "failed"),
                            };
                            view! {
                                <div class="entry">
                                    <span class="dim">"redeem"</span>
                                    <TxLink chain=Chain::Elysium hash=hash />
                                    <span class=class>{text}</span>
                                </div>
                            }
                        })
                        .collect_view()}
                    {can_redeem
                        .then(|| {
                            view! {
                                <Actions>
                                    <button class="button small" on:click=move |_| redeem(id)>"Redeem"</button>
                                </Actions>
                            }
                        })}
                }
            })
            .collect_view()
    }
}

/// A ticket built, simulated and submitted from HyperEVM, then followed until Elysium takes it.
#[component]
pub fn TicketPanel(ctx: Ctx) -> impl IntoView {
    let to = RwSignal::new(String::new());
    let value = RwSignal::new("0".to_owned());
    let data = RwSignal::new(String::new());
    let followed = RwSignal::new(Vec::<Followed>::new());
    let source = RwSignal::new(None::<B256>);
    let status = RwSignal::new(Status::Idle);

    let inputs = move || -> Out<(Address, Address, Dec, Vec<u8>)> {
        let me = ctx.sender()?.address();
        let to = match to.get_untracked().trim() {
            "" => me,
            to => to.parse()?,
        };
        let data = match data.get_untracked().trim() {
            "" | "0x" => Vec::new(),
            data => hex::decode(data)?,
        };
        Ok((me, to, value.get_untracked().trim().parse()?, data))
    };
    let simulate = move |_| {
        run(status, "building the ticket", async move {
            let (me, to, value, data) = inputs()?;
            let (ticket, call) = build(me, to, value, data).await?;
            let parent = Chain::HyperEvm.reader();
            let base = parent.gas_price().await?;
            let sender = if parent.balance(me).await? > call.value { me } else { HYPE_SYSTEM };
            let answer = parent.rpc().call(&CallRequest::of(Some(sender), &call), BlockTag::Latest).await?;
            let number = CREATE_RETRYABLE_TICKET.answer(&answer)?.first().and_then(|token| token.as_u256());
            let number = number.ok_or("createRetryableTicket answered no message number")?;
            let gas_limit = ticket.gas_limit;
            let ahead = Ticket {
                message_number: number,
                from: apply_alias(sender),
                l1_base_fee: base,
                deposit: call.value,
                retryable: ticket,
            };
            Ok(Status::Done(format!(
                "deposit {}, redeem gas {gas_limit}; message {number}, ticket {} if sent now",
                hype_text(call.value),
                short(&ahead.id(NETWORK.chain_id()).to_string())
            )))
        })
    };
    let send = move |_| {
        run(status, "submitting the ticket", async move {
            let (me, to, value, data) = inputs()?;
            let (_, call) = build(me, to, value, data).await?;
            let receipt = transact(ctx, status, Chain::HyperEvm, "retryable ticket", call).await?;
            let hash = receipt.transaction_hash;
            source.set(Some(hash));
            for _ in 0..120 {
                let found = tickets(hash).await?;
                let done = found.iter().all(|ticket| ticket.submitted.is_some());
                followed.set(found);
                if done {
                    return Ok(Status::sent("submitted on Elysium", Chain::HyperEvm, hash));
                }
                status.set(Status::Busy("waiting for Elysium to take the ticket".to_owned()));
                sleep(1_000).await;
            }
            Err("Elysium has not taken the ticket after two minutes".into())
        })
    };

    view! {
        <Panel title="Submit a ticket">
            <p class="note">"Inbox.createRetryableTicket on HyperEVM: a call on Elysium, its gas and submission paid from HyperEVM."</p>
            <Field label="To on Elysium" value=to placeholder="this account" />
            <Field label="Value, HYPE" value=value />
            <Field label="Calldata" value=data placeholder="0x... none for a plain transfer" />
            <Actions>
                <button class="button" on:click=simulate>"Simulate"</button>
                <button class="button primary" on:click=send>"Submit"</button>
            </Actions>
            <StatusLine status=status />
            {listing(ctx, status, followed, source)}
        </Panel>
    }
}

/// Any ticket followed from its HyperEVM transaction to its redeem; a live one redeemed.
#[component]
pub fn TicketsPanel(ctx: Ctx) -> impl IntoView {
    let track = RwSignal::new(String::new());
    let followed = RwSignal::new(Vec::<Followed>::new());
    let source = RwSignal::new(None::<B256>);
    let status = RwSignal::new(Status::Idle);
    let follow_ticket = move |_| {
        run(status, "following", async move {
            let hash: B256 = track.get_untracked().trim().parse()?;
            source.set(Some(hash));
            followed.set(tickets(hash).await?);
            Ok(Status::Idle)
        })
    };

    view! {
        <Panel title="Follow or redeem a ticket">
            <p class="note">"A ticket whose redeem failed stays live for a week: anyone may redeem it."</p>
            <Field label="HyperEVM transaction" value=track placeholder="0x... the one that submitted it" />
            <Actions>
                <button class="button primary" on:click=follow_ticket>"Follow"</button>
            </Actions>
            <StatusLine status=status />
            {listing(ctx, status, followed, source)}
        </Panel>
    }
}
