//! What the unit tests share: running a future without a runtime, an [`Http`] wire that answers
//! from a queue, and the recordings under `vectors/`. Nothing leaves the process.

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use serde_json::Value;

use crate::client::{Http, Log, Request, Response, TransportError};
use crate::utils::{Address, hex};

/// Runs a future to its end on this thread. The fakes here never wait, so each future is ready
/// on its first poll.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// An [`Http`] wire that answers from a queue, then with its fallback, and keeps every request.
pub(crate) struct Wire {
    answers: RefCell<VecDeque<Result<Response, TransportError>>>,
    otherwise: Result<Response, TransportError>,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    /// Answers with `answers` in turn, then as unreachable.
    pub(crate) fn answering(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Wire {
        Wire {
            answers: RefCell::new(answers.into_iter().collect()),
            otherwise: Err(TransportError::Unreachable("dry".to_owned())),
            sent: RefCell::default(),
        }
    }

    /// Every request as its body's JSON.
    pub(crate) fn bodies(&self) -> Vec<Value> {
        self.sent.borrow().iter().map(|request| serde_json::from_slice(&request.body).unwrap()).collect()
    }

    /// Every request, in order.
    pub(crate) fn sent(&self) -> Vec<Request> {
        self.sent.borrow().clone()
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(self.answers.borrow_mut().pop_front().unwrap_or_else(|| self.otherwise.clone()))
    }
}

/// A `200` with `text` as its body.
pub(crate) fn ok(text: &str) -> Result<Response, TransportError> {
    status(200, text)
}

/// `status` with `text` as its body.
pub(crate) fn status(status: u16, text: &str) -> Result<Response, TransportError> {
    Ok(Response { status, headers: Vec::new(), body: text.as_bytes().to_vec() })
}

const ELYSIUM: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/elysium_testnet.json"));
const HYPEREVM: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/hyperevm_testnet.json"));
const ARBITRUM_SDK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/arbitrum_sdk.json"));

/// Elysium testnet's answers, verbatim.
pub(crate) fn elysium() -> Value {
    serde_json::from_str(ELYSIUM).unwrap()
}

/// HyperEVM testnet's answers to the rollup's contracts, verbatim.
pub(crate) fn hyperevm() -> Value {
    serde_json::from_str(HYPEREVM).unwrap()
}

/// What @arbitrum/sdk and viem compute for the same inputs.
pub(crate) fn arbitrum_sdk() -> Value {
    serde_json::from_str(ARBITRUM_SDK).unwrap()
}

/// The answer text of the `nth` recorded call of `method` to Elysium.
pub(crate) fn answer(method: &str, nth: usize) -> String {
    let file = elysium();
    let found = file["calls"].as_array().unwrap().iter().filter(|call| call["method"] == method).nth(nth);
    found.unwrap_or_else(|| panic!("no {method} #{nth}"))["answer"].as_str().unwrap().to_owned()
}

/// One step of a recorded journey: its answer text.
pub(crate) fn step(journey: &str, step: &str) -> String {
    let file = elysium();
    let found = &file["journeys"][journey][step];
    found["answer"].as_str().unwrap_or_else(|| panic!("no {journey}.{step}")).to_owned()
}

/// An answer's `result`.
pub(crate) fn result(answer: &str) -> Value {
    serde_json::from_str::<Value>(answer).unwrap()["result"].clone()
}

/// The logs of a receipt answer.
pub(crate) fn logs(receipt: &str) -> Vec<Log> {
    serde_json::from_value(result(receipt)["logs"].clone()).unwrap()
}

pub(crate) fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_else(|| panic!("{value} is not text"))
}

pub(crate) fn bytes(value: &Value) -> Vec<u8> {
    hex::decode(text(value)).unwrap()
}

pub(crate) fn address(text: &str) -> Address {
    text.parse().unwrap()
}
