//! The home screen: a card per group of tools, each tool a button that opens it.

use leptos::prelude::*;

use crate::panels::account::AccountCard;
use crate::panels::chain::ChainCard;
use crate::route::Route;
use crate::state::Ctx;
use crate::ui::{Actions, Card, Entry};

#[component]
pub fn Home(ctx: Ctx) -> impl IntoView {
    view! {
        <div class="cards">
            <AccountCard ctx=ctx />
            <Card title="Transfer" note="HYPE and ERC-20s on Elysium, in exact amounts, priced before they are sent.">
                <Actions>
                    <Entry route=Route::Send label="Send HYPE" primary=true />
                    <Entry route=Route::Token label="ERC-20 tokens" />
                </Actions>
            </Card>
            <Card title="Bridge" note="HYPE between HyperEVM and Elysium: in within a block or two, out once the rollup confirms it.">
                <Actions>
                    <Entry route=Route::Deposit label="Deposit from HyperEVM" />
                    <Entry route=Route::Withdraw label="Withdraw to HyperEVM" />
                    <Entry route=Route::Claims label="Claim withdrawals" />
                </Actions>
            </Card>
            <Card title="Retryable tickets" note="A call on Elysium sent and paid for from HyperEVM, redeemable by anyone while it lives.">
                <Actions>
                    <Entry route=Route::Ticket label="Submit a ticket" />
                    <Entry route=Route::Tickets label="Follow or redeem" />
                </Actions>
            </Card>
            <Card title="Contracts" note="Deploy by CREATE or through the CREATE2 deployer, and check verification on Sourcify.">
                <Actions>
                    <Entry route=Route::Deploy label="Deploy a contract" />
                    <Entry route=Route::Verify label="Verify on Sourcify" />
                </Actions>
            </Card>
            <ChainCard ctx=ctx />
        </div>
    }
}
