//! Contracts on Elysium: one created from the account (CREATE, landing at its address and nonce)
//! and one through the deterministic deployer (CREATE2, landing where the salt and code say, whoever
//! sends it), each called once deployed; and a contract's verification on Conduit's Sourcify.

use df_elysium_core::client::web::Web;
use df_elysium_core::client::{BlockTag, Call, CallRequest, create_address};
use df_elysium_core::deploy::{self, CREATE2_DEPLOYER, verify};
use df_elysium_core::utils::hex;
use df_elysium_core::{Address, B256, U256};
use leptos::prelude::*;

use crate::keys;
use crate::net::{Chain, NETWORK};
use crate::state::{Ctx, Out, Status, poll, problem, run, text, transact};
use crate::ui::{Actions, Field, Panel, Part, Row, StatusLine};

/// Copies the ten bytes after it into memory and returns them: a contract that answers 42 to
/// anything (`PUSH1 42, PUSH1 0, MSTORE, PUSH1 32, PUSH1 0, RETURN`).
const FORTY_TWO: [u8; 22] = [
    0x60, 0x0a, 0x60, 0x0c, 0x60, 0x00, 0x39, 0x60, 0x0a, 0x60, 0x00, 0xf3, // init: copy and return
    0x60, 0x2a, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3, // runtime
];

/// The reactor's escrow on Elysium testnet, verified on Conduit's Sourcify.
const ESCROW: &str = "0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB";

fn new_salt() -> String {
    keys::random().map(|bytes| hex::encode_prefixed(&bytes)).unwrap_or_default()
}

/// Where the next CREATE from `me` lands.
async fn next_create(me: Address) -> Out<Address> {
    Ok(create_address(me, Chain::Elysium.reader().nonce(me).await?))
}

/// Deploys `creation`, then calls what it made: the contract and its answer.
async fn deployed(ctx: Ctx, status: RwSignal<Status>, what: &str, creation: Call, at: Address) -> Out<Status> {
    let receipt = transact(ctx, status, Chain::Elysium, what, creation).await?;
    let client = Chain::Elysium.reader();
    let answer = client.rpc().call(&CallRequest::to(at, Vec::new()), BlockTag::Latest).await?;
    let answer = U256::from_be_slice(&answer).ok_or("its answer is not a word")?;
    let code = client.code(at).await?.len();
    let note = format!("{at}: {code} bytes of code, answers {answer}");
    Ok(Status::sent(note, Chain::Elysium, receipt.transaction_hash))
}

#[component]
pub fn DeployPanel(ctx: Ctx) -> impl IntoView {
    let salt = RwSignal::new(new_salt());
    let created = poll(ctx, move || ctx.me().map(next_create));
    let created2 = move || {
        let salt: B256 = salt.get().trim().parse().ok()?;
        Some(deploy::create2_address(CREATE2_DEPLOYER, salt, deploy::init_code_hash(&FORTY_TWO)))
    };
    let status = RwSignal::new(Status::Idle);
    let checked = RwSignal::new(ESCROW.to_owned());
    let verdict = RwSignal::new(Status::Idle);

    let create = move |_| {
        run(status, "deploying with CREATE", async move {
            let at = next_create(ctx.sender()?.address()).await?;
            deployed(ctx, status, "CREATE", deploy::create(&FORTY_TWO, &[]), at).await
        })
    };
    let create2 = move |_| {
        run(status, "deploying with CREATE2", async move {
            let at = created2().ok_or("the salt is not 32 bytes of hex")?;
            let salt: B256 = salt.get_untracked().trim().parse()?;
            if !Chain::Elysium.reader().code(at).await?.is_empty() {
                return Err(format!("{at} is deployed already: take a new salt").into());
            }
            deployed(ctx, status, "CREATE2", deploy::create2(salt, &FORTY_TWO), at).await
        })
    };
    let check = move |_| {
        run(verdict, "asking Sourcify", async move {
            let at: Address = checked.get_untracked().trim().parse()?;
            let found = verify::status(&Web, NETWORK, at).await?;
            Ok(Status::Done(match found.matched {
                Some(matched) => format!("{matched:?} match, verified {}", found.verified_at.unwrap_or_default()),
                None => "not verified".to_owned(),
            }))
        })
    };

    view! {
        <Panel title="Deploy">
            <p class="note">"A contract that answers 42 to any call, 10 bytes of runtime."</p>
            <Row label="CREATE lands at">{text(created, |at: &Address| at.to_string())}</Row>
            {problem(created)}
            <Field label="CREATE2 salt" value=salt />
            <Row label="CREATE2 lands at">
                {move || created2().map(|at| at.to_string()).unwrap_or_else(|| "-".to_owned())}
            </Row>
            <Actions>
                <button class="button primary" on:click=create>"CREATE"</button>
                <button class="button primary" on:click=create2>"CREATE2"</button>
                <button class="button" on:click=move |_| salt.set(new_salt())>"New salt"</button>
            </Actions>
            <StatusLine status=status />
            <Part title="Sourcify" />
            <Field label="Contract" value=checked />
            <Actions>
                <button class="button" on:click=check>"Check"</button>
            </Actions>
            <StatusLine status=verdict />
            <p class="note mono">{format!("forge verify-contract --verifier sourcify --verifier-url {}", NETWORK.verifier_url())}</p>
        </Panel>
    }
}
