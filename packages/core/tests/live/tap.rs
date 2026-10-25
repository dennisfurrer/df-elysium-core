//! A testnet [`Client`] whose wire keeps every answer, so a test can hold what the node said
//! against the recording the unit and mocked suites read, and an `Rpc` to HyperEVM testnet.

use std::cell::RefCell;
use std::future::Future;

use df_elysium_core::client::native::Native;
use df_elysium_core::client::{Http, Request, Response, Rpc, TransportError};
use df_elysium_core::{Client, Network};
use serde_json::Value;

use crate::shape::added;

/// The real wire, keeping each request's body and its answer's text.
pub struct Tap {
    wire: Native,
    kept: RefCell<Vec<(String, String)>>,
}

impl Http for Tap {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        let body = String::from_utf8_lossy(&request.body).into_owned();
        async move {
            let answer = self.wire.send(request).await?;
            self.kept.borrow_mut().push((body, answer.text()));
            Ok(answer)
        }
    }
}

fn tap() -> Tap {
    Tap { wire: Native::new(), kept: RefCell::default() }
}

/// A reader on Elysium testnet through a [`Tap`].
pub fn reader() -> Client<Tap> {
    Client::reader(tap(), Network::Testnet)
}

/// HyperEVM testnet, Elysium's parent, through a [`Tap`].
pub fn parent() -> Rpc<Tap> {
    Rpc::new(tap(), Network::Testnet.hyperliquid().evm_rpc_url())
}

/// The last answer the wire got, as JSON.
pub fn last(http: &Tap) -> Value {
    let kept = http.kept.borrow();
    let (body, answer) = kept.last().expect("nothing was sent");
    serde_json::from_str(answer).unwrap_or_else(|error| panic!("{error}: the answer to {body} is not JSON: {answer}"))
}

/// Holds the last answer against `recorded`, a recording entry, and fails naming every key path
/// the node has added since it was recorded.
pub fn same_shape(http: &Tap, recorded: &Value) {
    let live = last(http);
    let said: Value = serde_json::from_str(recorded["answer"].as_str().unwrap()).unwrap();
    let added = added(&said, &live);
    let name = recorded.get("name").unwrap_or(&recorded["method"]);
    assert!(added.is_empty(), "the node now answers {name} with fields it did not have: {added:#?}");
}
