//! Any transaction by its hash. On Elysium: its Nitro type, gas split between Elysium and
//! HyperEVM's data, fee, the parent-chain fields a deposit, ticket or redeem carries, and the
//! withdrawals it made. On HyperEVM: the messages it delivered to Elysium's inbox, and the
//! transaction each becomes on Elysium.

use df_elysium_core::bridge::{Deposit, Message, Ticket, Withdrawal};
use df_elysium_core::client::{Receipt, Transaction};
use df_elysium_core::{B256, U256};
use leptos::prelude::*;

use crate::net::{Chain, NETWORK, short};
use crate::state::{Out, Status, run};
use crate::ui::{AddressLink, Field, Panel, Part, Row, StatusLine, TxLink, hype_text};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Delivered {
    number: U256,
    kind: String,
    /// Its transaction on Elysium: a deposit's hash or a ticket's id.
    becomes: Option<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    Elysium { tx: Box<Transaction>, receipt: Option<Box<Receipt>>, withdrawals: Vec<Withdrawal> },
    HyperEvm { receipt: Box<Receipt>, delivered: Vec<Delivered> },
}

async fn find(hash: B256) -> Out<Found> {
    let elysium = Chain::Elysium.reader();
    if let Some(tx) = elysium.transaction(hash).await? {
        let receipt = elysium.receipt(hash).await?;
        let withdrawals = match &receipt {
            Some(receipt) => Withdrawal::in_logs(&receipt.logs)?,
            None => Vec::new(),
        };
        return Ok(Found::Elysium { tx: Box::new(tx), receipt: receipt.map(Box::new), withdrawals });
    }
    let receipt = Chain::HyperEvm.reader().receipt(hash).await?.ok_or("neither Elysium nor HyperEVM knows it")?;
    let delivered = Message::in_logs(&receipt.logs, &NETWORK.contracts())?
        .iter()
        .map(|message| Delivered {
            number: message.number,
            kind: format!("{:?}", message.kind),
            becomes: Deposit::from_message(message)
                .map(|deposit| deposit.tx_hash(NETWORK.chain_id()))
                .or_else(|| Ticket::from_message(message).map(|ticket| ticket.id(NETWORK.chain_id()))),
        })
        .collect();
    Ok(Found::HyperEvm { receipt: Box::new(receipt), delivered })
}

fn elysium(tx: &Transaction, receipt: Option<&Receipt>, withdrawals: &[Withdrawal]) -> impl IntoView {
    let kind = format!("{:?} ({:#04x})", tx.tx_kind(), tx.kind);
    let outcome = match receipt {
        None => "pending".to_owned(),
        Some(receipt) if receipt.succeeded() => {
            format!("succeeded in block {}", receipt.block_number.unwrap_or_default())
        }
        Some(receipt) => format!("reverted in block {}", receipt.block_number.unwrap_or_default()),
    };
    let gas = receipt.map(|receipt| {
        format!(
            "{} = {} Elysium + {} HyperEVM data",
            receipt.gas_used,
            receipt.gas_used_for_l2(),
            receipt.gas_used_for_l1
        )
    });
    let fee = receipt.and_then(Receipt::fee).map(hype_text);
    let (from, to, hash, value, nonce) = (tx.from, tx.to, tx.hash, hype_text(tx.value), tx.nonce.to_string());
    let sends: Vec<(String, String)> = withdrawals
        .iter()
        .map(|sent| {
            let text = format!("{} to {}", hype_text(sent.value), short(&sent.destination.to_string()));
            (sent.position.to_string(), text)
        })
        .collect();
    let parent = [
        ("request id", tx.request_id.map(|id| id.to_string())),
        ("ticket id", tx.ticket_id.map(|id| id.to_string())),
        ("deposit", tx.deposit_value.map(hype_text)),
        ("refund to", tx.refund_to.map(|to| to.to_string())),
    ];
    view! {
        <Row label="on">"Elysium"</Row>
        <Row label="hash">
            <TxLink chain=Chain::Elysium hash=hash />
        </Row>
        <Row label="type">{kind}</Row>
        <Row label="outcome">{outcome}</Row>
        <Row label="from">
            <AddressLink address=from />
        </Row>
        <Row label="to">
            {to.map(|to| view! { <AddressLink address=to /> }.into_any()).unwrap_or_else(|| "a creation".into_any())}
        </Row>
        <Row label="value">{value}</Row>
        <Row label="nonce">{nonce}</Row>
        {gas.map(|gas| view! { <Row label="gas">{gas}</Row> })}
        {fee.map(|fee| view! { <Row label="fee">{fee}</Row> })}
        {parent
            .into_iter()
            .filter_map(|(label, value)| value.map(|value| view! { <Row label=label>{short(&value)}</Row> }))
            .collect_view()}
        {sends
            .into_iter()
            .map(|(leaf, text)| {
                view! {
                    <Part title="Withdrawal" />
                    <Row label="leaf">{leaf}</Row>
                    <Row label="sends">{text}</Row>
                }
            })
            .collect_view()}
    }
}

fn hyperevm(receipt: &Receipt, delivered: &[Delivered]) -> impl IntoView {
    let outcome = format!(
        "{} in block {}",
        if receipt.succeeded() { "succeeded" } else { "reverted" },
        receipt.block_number.unwrap_or_default()
    );
    let count = delivered.len().to_string();
    let lines: Vec<(String, Option<B256>)> = delivered
        .iter()
        .map(|message| (format!("message {}, {}", message.number, message.kind), message.becomes))
        .collect();
    view! {
        <Row label="on">"HyperEVM"</Row>
        <Row label="outcome">{outcome}</Row>
        <Row label="messages to Elysium">{count}</Row>
        {lines
            .into_iter()
            .map(|(label, becomes)| {
                view! {
                    <div class="entry">
                        <span>{label}</span>
                        {becomes.map(|hash| view! { <TxLink chain=Chain::Elysium hash=hash /> })}
                    </div>
                }
            })
            .collect_view()}
    }
}

#[component]
pub fn InspectPanel() -> impl IntoView {
    let hash = RwSignal::new(String::new());
    let found = RwSignal::new(None::<Found>);
    let status = RwSignal::new(Status::Idle);
    let look = move |_| {
        run(status, "looking on both chains", async move {
            found.set(None);
            found.set(Some(find(hash.get_untracked().trim().parse()?).await?));
            Ok(Status::Idle)
        })
    };

    view! {
        <Panel title="Inspect">
            <Field label="Transaction" value=hash placeholder="0x... on Elysium or HyperEVM" />
            <div class="actions">
                <button class="button" on:click=look>"Look up"</button>
            </div>
            <StatusLine status=status />
            {move || {
                found
                    .get()
                    .map(|found| match found {
                        Found::Elysium { tx, receipt, withdrawals } => {
                            elysium(&tx, receipt.as_deref(), &withdrawals).into_any()
                        }
                        Found::HyperEvm { receipt, delivered } => hyperevm(&receipt, &delivered).into_any(),
                    })
            }}
        </Panel>
    }
}
