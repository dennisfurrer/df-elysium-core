//! What the page shares and how it acts: the account, the head, a tick that keeps reads fresh,
//! every transaction it sent, each action's status, and the one way an action sends.

use std::cell::Cell;
use std::error::Error;
use std::future::Future;
use std::rc::Rc;

use df_elysium_core::client::{Call, Receipt};
use df_elysium_core::{Address, B256};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::net::{self, Account, Chain, short};

/// What an action ends with, or why it stopped.
pub type Out<T> = Result<T, Box<dyn Error>>;

/// A read's latest answer: `None` until the first, the error as text when it failed, and an empty
/// error when there was nothing to read.
pub type Loaded<T> = RwSignal<Option<Result<T, String>>>;

/// A transaction the page sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub what: String,
    pub chain: Chain,
    pub hash: B256,
    /// `None` until it is in a block.
    pub succeeded: Option<bool>,
}

/// What every panel shares.
#[derive(Clone, Copy)]
pub struct Ctx {
    pub account: RwSignal<Option<Account>>,
    /// Elysium's latest block, as the chain panel last read it.
    pub head: RwSignal<Option<u64>>,
    /// Counts up every few seconds and after each transaction: polled reads run again on it.
    pub tick: RwSignal<u64>,
    pub activity: RwSignal<Vec<Activity>>,
}

impl Ctx {
    pub fn new(account: Option<Account>) -> Ctx {
        Ctx {
            account: RwSignal::new(account),
            head: RwSignal::new(None),
            tick: RwSignal::new(0),
            activity: RwSignal::new(Vec::new()),
        }
    }

    /// The account's address, tracked: a read that names it runs again when it changes.
    pub fn me(&self) -> Option<Address> {
        self.account.with(|account| account.as_ref().map(Account::address))
    }

    /// The account that sends, or why there is none.
    pub fn sender(&self) -> Out<Account> {
        self.account.get_untracked().ok_or_else(|| "no account: make a burner key or connect a wallet first".into())
    }

    /// Runs every polled read again now.
    pub fn refresh(&self) {
        self.tick.update(|tick| *tick = tick.wrapping_add(1));
    }
}

/// Where an action stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    Busy(String),
    Done(String),
    /// Done, in a transaction.
    Sent {
        note: String,
        chain: Chain,
        hash: B256,
    },
    Failed(String),
}

impl Status {
    pub fn sent(note: impl Into<String>, chain: Chain, hash: B256) -> Status {
        Status::Sent { note: note.into(), chain, hash }
    }

    pub const fn is_busy(&self) -> bool {
        matches!(self, Status::Busy(_))
    }
}

/// Runs `work` in the page, `busy` on `status` until it ends. A press while it runs is ignored.
pub fn run<F>(status: RwSignal<Status>, busy: impl Into<String>, work: F)
where
    F: Future<Output = Out<Status>> + 'static,
{
    if status.get_untracked().is_busy() {
        return;
    }
    status.set(Status::Busy(busy.into()));
    spawn_local(async move {
        let done = work.await.unwrap_or_else(|error| Status::Failed(error.to_string()));
        status.set(done);
    });
}

/// Sends `call` on `chain` from the page's account, notes it in the activity, and waits for its
/// receipt; a reverted one is an error.
pub async fn transact(ctx: Ctx, status: RwSignal<Status>, chain: Chain, what: &str, call: Call) -> Out<Receipt> {
    let account = ctx.sender()?;
    status.set(Status::Busy(match account {
        Account::Burner(_) => format!("{what}: signing for {}", chain.name()),
        Account::Wallet(_) => format!("{what}: confirm it in the wallet, on {}", chain.name()),
    }));
    let hash = net::send(&account, chain, call).await?;
    ctx.activity.update(|list| list.insert(0, Activity { what: what.to_owned(), chain, hash, succeeded: None }));
    status.set(Status::Busy(format!("{what}: {} sent, waiting for its block", short(&hash.to_string()))));
    let receipt = net::wait(chain, hash).await?;
    let succeeded = receipt.succeeded();
    ctx.activity.update(|list| {
        for entry in list.iter_mut().filter(|entry| entry.hash == hash) {
            entry.succeeded = Some(succeeded);
        }
    });
    ctx.refresh();
    match succeeded {
        true => Ok(receipt),
        false => Err(format!("{what} reverted: {hash}").into()),
    }
}

/// A failed read is tried again after these waits: the public nodes refuse bursts (HTTP 429) and
/// now and then lag a block. Reads are safe to repeat; sends never are, and are never repeated.
const RETRY_AFTER_MS: [i32; 2] = [1_000, 3_000];

/// Runs `read` now, again on every tick, and again when a signal it reads changes. `read` answers
/// `None` when there is nothing to read yet (no account).
pub fn poll<T, F>(ctx: Ctx, read: impl Fn() -> Option<F> + 'static) -> Loaded<T>
where
    T: Send + Sync + 'static,
    F: Future<Output = Out<T>> + 'static,
{
    reread(Some(ctx), read)
}

/// Runs `read` now, and again only when a signal it reads changes.
pub fn follow<T, F>(read: impl Fn() -> Option<F> + 'static) -> Loaded<T>
where
    T: Send + Sync + 'static,
    F: Future<Output = Out<T>> + 'static,
{
    reread(None, read)
}

fn reread<T, F>(ticks: Option<Ctx>, read: impl Fn() -> Option<F> + 'static) -> Loaded<T>
where
    T: Send + Sync + 'static,
    F: Future<Output = Out<T>> + 'static,
{
    let loaded = RwSignal::new(None);
    let read = Rc::new(read);
    let latest = Rc::new(Cell::new(0u64));
    let running = Rc::new(Cell::new(false));
    Effect::new(move |_| {
        if let Some(ctx) = ticks {
            ctx.tick.track();
        }
        // Called every time, so the effect keeps tracking what it reads.
        let Some(answer) = (*read)() else {
            latest.set(latest.get().wrapping_add(1));
            loaded.set(Some(Err(String::new())));
            return;
        };
        // A poll still running when the tick comes is left to finish.
        if ticks.is_some() && running.get() {
            return;
        }
        let id = latest.get().wrapping_add(1);
        latest.set(id);
        running.set(true);
        let (read, latest, running) = (read.clone(), latest.clone(), running.clone());
        spawn_local(async move {
            let mut answer = answer.await;
            for wait in RETRY_AFTER_MS {
                if answer.is_ok() || latest.get() != id {
                    break;
                }
                net::sleep(wait).await;
                let Some(again) = untrack(|| (*read)()) else { break };
                answer = again.await;
            }
            running.set(false);
            // Only the latest read's answer is shown.
            if latest.get() == id {
                loaded.set(Some(answer.map_err(|error| error.to_string())));
            }
        });
    });
    loaded
}

/// A loaded value as text: `...` while it loads, `-` when it failed (the panel says why).
pub fn text<T>(loaded: Loaded<T>, show: impl Fn(&T) -> String + Send + 'static) -> impl Fn() -> String + Send + 'static
where
    T: Send + Sync + 'static,
{
    move || {
        loaded.with(|answer| match answer {
            None => "...".to_owned(),
            Some(Ok(value)) => show(value),
            Some(Err(_)) => "-".to_owned(),
        })
    }
}

/// Why a read failed, when it did.
pub fn problem<T: Send + Sync + 'static>(loaded: Loaded<T>) -> impl IntoView {
    move || {
        loaded.with(|answer| match answer {
            Some(Err(error)) if !error.is_empty() => Some(view! { <p class="status crit">{error.clone()}</p> }),
            _ => None,
        })
    }
}
