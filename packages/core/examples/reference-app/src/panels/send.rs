//! HYPE to anyone on Elysium: the estimate split as the NodeInterface splits it (Elysium's
//! execution, HyperEVM's data), then the transfer.

use df_elysium_core::client::Call;
use df_elysium_core::gas::Estimate;
use df_elysium_core::transfer;
use df_elysium_core::utils::hype;
use leptos::prelude::*;

use crate::net::Chain;
use crate::state::{Ctx, Out, Status, run, transact};
use crate::ui::{Actions, Field, Panel, Row, StatusLine, gwei, hype_text};

#[component]
pub fn SendPanel(ctx: Ctx) -> impl IntoView {
    let to = RwSignal::new(String::new());
    let amount = RwSignal::new("0.0001".to_owned());
    let quote = RwSignal::new(None::<Estimate>);
    let status = RwSignal::new(Status::Idle);

    let call = move || -> Out<Call> {
        let to = to.get_untracked().trim().parse()?;
        Ok(transfer::hype(to, amount.get_untracked().trim().parse()?)?)
    };
    let estimate = move |_| {
        run(status, "estimating", async move {
            let from = ctx.sender()?.address();
            quote.set(Some(Chain::Elysium.reader().estimate(Some(from), &call()?).await?));
            Ok(Status::Idle)
        })
    };
    let send = move |_| {
        run(status, "sending", async move {
            let receipt = transact(ctx, status, Chain::Elysium, "send HYPE", call()?).await?;
            let paid = receipt.fee().map(hype_text).unwrap_or_default();
            Ok(Status::sent(
                format!("sent: {} gas, {paid}", receipt.gas_used),
                Chain::Elysium,
                receipt.transaction_hash,
            ))
        })
    };

    view! {
        <Panel title="Send HYPE">
            <Field label="To" value=to placeholder="0x..." />
            <Field label="Amount, HYPE" value=amount />
            <Actions>
                <button class="button" on:click=estimate>"Estimate"</button>
                <button class="button primary" on:click=send>"Send"</button>
            </Actions>
            {move || {
                quote
                    .get()
                    .map(|quote| {
                        let cost = hype(quote.cost()).map(|cost| cost.to_string()).unwrap_or_default();
                        let data = hype(quote.cost_for_l1()).map(|data| data.to_string()).unwrap_or_default();
                        view! {
                            <Row label="gas">{quote.gas.to_string()}</Row>
                            <Row label="of it Elysium">{quote.gas_for_l2().to_string()}</Row>
                            <Row label="of it HyperEVM data">{quote.gas_for_l1.to_string()}</Row>
                            <Row label="base fee">{gwei(quote.base_fee)}</Row>
                            <Row label="costs">{format!("{cost} HYPE, {data} of it data")}</Row>
                        }
                    })
            }}
            <StatusLine status=status />
        </Panel>
    }
}
