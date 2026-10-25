//! Every transaction this page sent, newest first, and how it ended.

use leptos::prelude::*;

use crate::state::Ctx;
use crate::ui::{Panel, TxLink};

#[component]
pub fn ActivityPanel(ctx: Ctx) -> impl IntoView {
    view! {
        <Panel title="Activity">
            {move || {
                let list = ctx.activity.get();
                if list.is_empty() {
                    return view! { <p class="note">"Nothing sent from this page yet."</p> }.into_any();
                }
                list.into_iter()
                    .map(|entry| {
                        let (class, state) = match entry.succeeded {
                            None => ("busy", "pending"),
                            Some(true) => ("ok", "succeeded"),
                            Some(false) => ("crit", "reverted"),
                        };
                        view! {
                            <div class="entry">
                                <span>{entry.what}</span>
                                <span class="dim">{entry.chain.name()}</span>
                                <TxLink chain=entry.chain hash=entry.hash />
                                <span class=class>{state}</span>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </Panel>
    }
}
