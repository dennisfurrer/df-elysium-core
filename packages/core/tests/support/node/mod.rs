//! A fake Elysium and its parent, HyperEVM testnet, behind the [`Http`] wire, for whole journeys
//! through a [`Client`] and df-hyperliquid-core's [`Rpc`]:
//!
//! - Each chain's JSON-RPC answers a call from the recordings it was given: by method, by the
//!   contract an `eth_call` or estimate goes to, or by its params exactly (hex in either case), the
//!   most specific first. An answer keeps the HTTP status it was recorded with, and a list of
//!   answers is given out in turn, the last one repeating.
//! - A raw transaction is answered with its hash, as a node does; a synchronous one with the
//!   receipt it was given, rewritten to that hash.
//! - Any other URL (Sourcify) answers from recordings of that URL.
//! - A failure can stand in for the next answer on a chain, or to one method there: an HTTP
//!   status, a 429 with its wait, or no answer at all. The request is still taken ([`failure`]).
//!
//! Anything it was not given an answer for fails the test, naming the request.
//!
//! [`Client`]: df_elysium_core::Client
//! [`Rpc`]: df_elysium_core::client::Rpc

mod failure;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::{Future, ready};
use std::ops::Deref;
use std::rc::Rc;

use df_elysium_core::Network;
use df_elysium_core::client::{Http, HttpMethod, Request, Response, TransportError};
use df_elysium_core::utils::{hex, keccak256};
use serde_json::{Value, json};

/// Which chain a call went to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Elysium,
    HyperEvm,
}

/// A request the node took.
#[derive(Debug, Clone)]
pub enum Sent {
    Rpc { chain: Chain, call: Value },
    Web { method: HttpMethod, url: String, body: String },
}

/// What the wire hands back.
type Answer = Result<Response, TransportError>;

/// One JSON-RPC answer and what it is for.
struct Route {
    chain: Chain,
    method: String,
    to: Option<String>,
    params: Option<Value>,
    status: u16,
    answers: RefCell<VecDeque<Value>>,
}

impl Route {
    /// How closely it names a call: by its params, by its contract, or by its method only.
    const fn rank(&self) -> u8 {
        match (&self.params, &self.to) {
            (Some(_), _) => 2,
            (None, Some(_)) => 1,
            (None, None) => 0,
        }
    }

    fn take(&self) -> Value {
        let mut answers = self.answers.borrow_mut();
        match answers.len() {
            1 => answers[0].clone(),
            _ => answers.pop_front().unwrap(),
        }
    }
}

/// One answer at a URL that is not a chain's.
struct Page {
    method: HttpMethod,
    url: String,
    status: u16,
    body: String,
}

pub struct Node {
    network: Network,
    routes: Vec<Route>,
    pages: Vec<Page>,
    sync_receipt: Option<Value>,
    failures: RefCell<VecDeque<failure::Failure>>,
    sent: RefCell<Vec<Sent>>,
}

impl Node {
    pub fn testnet() -> Node {
        Node {
            network: Network::Testnet,
            routes: Vec::new(),
            pages: Vec::new(),
            sync_receipt: None,
            failures: RefCell::default(),
            sent: RefCell::default(),
        }
    }

    fn route(
        mut self,
        chain: Chain,
        method: &str,
        to: Option<&str>,
        params: Option<Value>,
        status: u16,
        answers: &[&str],
    ) -> Node {
        let answers = answers.iter().map(|answer| serde_json::from_str(answer).unwrap()).collect();
        self.routes.push(Route {
            chain,
            method: method.to_owned(),
            to: to.map(str::to_ascii_lowercase),
            params,
            status,
            answers: RefCell::new(answers),
        });
        self
    }

    /// Answers every call to `method` on `chain` with `answer`, a whole JSON-RPC answer as a node
    /// writes it; its `id` is set to the request's.
    #[must_use]
    pub fn answer(self, chain: Chain, method: &str, answer: &str) -> Node {
        self.route(chain, method, None, None, 200, &[answer])
    }

    /// Answers the calls to `method` with `answers` in turn, the last one repeating.
    #[must_use]
    pub fn answers(self, chain: Chain, method: &str, answers: &[&str]) -> Node {
        self.route(chain, method, None, None, 200, answers)
    }

    /// Answers every call to `method` whose call object goes to `to`.
    #[must_use]
    pub fn answer_to(self, chain: Chain, method: &str, to: &str, answer: &str) -> Node {
        self.route(chain, method, Some(to), None, 200, &[answer])
    }

    /// Answers the call a recording made (its method and params, either case of hex) with the
    /// recorded answer and status.
    #[must_use]
    pub fn recorded(self, chain: Chain, entry: &Value) -> Node {
        let method = entry["method"].as_str().unwrap_or("eth_call").to_owned();
        let status = entry["status"].as_u64().map_or(200, |status| u16::try_from(status).unwrap());
        let answer = entry["answer"].as_str().unwrap().to_owned();
        self.route(chain, &method, None, Some(lower(&entry["params"])), status, &[&answer])
    }

    /// Each recording, on `chain`.
    #[must_use]
    pub fn all_recorded<'a>(self, chain: Chain, entries: impl IntoIterator<Item = &'a Value>) -> Node {
        entries.into_iter().fold(self, |node, entry| node.recorded(chain, entry))
    }

    /// What `eth_sendRawTransactionSync` answers: this receipt, its hash the transaction's.
    #[must_use]
    pub fn sync_receipt(mut self, receipt: &str) -> Node {
        self.sync_receipt = Some(serde_json::from_str(receipt).unwrap());
        self
    }

    /// Answers `method` to `url`, matched whatever the case of its hex, with HTTP `status` and `body`.
    #[must_use]
    pub fn page(mut self, method: HttpMethod, url: &str, status: u16, body: &str) -> Node {
        self.pages.push(Page { method, url: url.to_owned(), status, body: body.to_owned() });
        self
    }

    /// Every request, in order.
    pub fn sent(&self) -> Vec<Sent> {
        self.sent.borrow().clone()
    }

    /// The JSON-RPC calls to `chain`, in order.
    pub fn calls(&self, chain: Chain) -> Vec<Value> {
        self.sent()
            .into_iter()
            .filter_map(|sent| match sent {
                Sent::Rpc { chain: to, call } if to == chain => Some(call),
                _ => None,
            })
            .collect()
    }

    /// The methods called on `chain`, in order.
    pub fn methods(&self, chain: Chain) -> Vec<String> {
        self.calls(chain).iter().map(|call| call["method"].as_str().unwrap().to_owned()).collect()
    }

    /// The raw transactions Elysium was sent, synchronous or not, in order.
    pub fn raws(&self) -> Vec<Vec<u8>> {
        self.calls(Chain::Elysium)
            .iter()
            .filter(|call| call["method"].as_str().is_some_and(|method| method.starts_with("eth_sendRawTransaction")))
            .map(|call| hex::decode(call["params"][0].as_str().unwrap()).unwrap())
            .collect()
    }

    fn chain_of(&self, url: &str) -> Option<Chain> {
        match url {
            url if url == self.network.rpc_url() => Some(Chain::Elysium),
            url if url == self.network.hyperliquid().evm_rpc_url() => Some(Chain::HyperEvm),
            _ => None,
        }
    }

    fn answer_request(&self, request: &Request) -> Answer {
        let body = String::from_utf8(request.body.clone()).unwrap();
        let Some(chain) = self.chain_of(&request.url) else {
            return self.web(request, body);
        };
        assert_eq!(request.method, HttpMethod::Post, "a JSON-RPC call is a post: {}", request.url);
        let call: Value = serde_json::from_str(&body)
            .unwrap_or_else(|error| panic!("a JSON-RPC body that is not JSON ({error}): {body}"));
        self.sent.borrow_mut().push(Sent::Rpc { chain, call: call.clone() });
        let method = call["method"].as_str().unwrap_or_else(|| panic!("a JSON-RPC call with no method: {body}"));
        if let Some(failed) = self.failure(chain, method) {
            return failed;
        }
        let (status, mut answer) = self.node(chain, &call, &body);
        answer["id"] = call["id"].clone();
        Ok(Response { status, headers: Vec::new(), body: answer.to_string().into_bytes() })
    }

    fn node(&self, chain: Chain, call: &Value, body: &str) -> (u16, Value) {
        let method = call["method"].as_str().unwrap();
        let params = lower(&call["params"]);
        let to = params[0]["to"].as_str();
        let route = self
            .routes
            .iter()
            .filter(|route| route.chain == chain && route.method == method)
            .filter(|route| route.params.as_ref().is_none_or(|said| *said == params))
            .filter(|route| route.to.is_none() || route.to.as_deref() == to)
            .max_by_key(|route| (route.rank(), std::cmp::Reverse(self.position(route))));
        if let Some(route) = route {
            return (route.status, route.take());
        }
        let raw = || hex::decode(call["params"][0].as_str().unwrap_or_default()).unwrap();
        match (chain, method) {
            (Chain::Elysium, "eth_sendRawTransaction") => {
                (200, json!({"jsonrpc": "2.0", "result": hex::encode_prefixed(&keccak256(&raw()))}))
            }
            (Chain::Elysium, "eth_sendRawTransactionSync") => {
                let mut receipt = self.sync_receipt.clone().unwrap_or_else(|| panic!("no receipt to answer {body}"));
                receipt["result"]["transactionHash"] = Value::String(hex::encode_prefixed(&keccak256(&raw())));
                (200, receipt)
            }
            _ => panic!("the node has no answer on {chain:?} for {body}"),
        }
    }

    fn position(&self, route: &Route) -> usize {
        self.routes.iter().position(|one| std::ptr::eq(one, route)).unwrap()
    }

    fn web(&self, request: &Request, body: String) -> Answer {
        self.sent.borrow_mut().push(Sent::Web { method: request.method, url: request.url.clone(), body });
        let page =
            self.pages.iter().find(|page| page.method == request.method && page.url.eq_ignore_ascii_case(&request.url));
        let page = page.unwrap_or_else(|| panic!("the node does not serve {:?} {}", request.method, request.url));
        Ok(Response { status: page.status, headers: Vec::new(), body: page.body.clone().into_bytes() })
    }
}

impl Http for Node {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        ready(self.answer_request(&request))
    }
}

/// One node behind several wires: a [`Client`](df_elysium_core::Client) to Elysium and
/// df-hyperliquid-core's `Rpc` to HyperEVM, each request kept in the one order.
#[derive(Clone)]
pub struct Shared(Rc<Node>);

impl Node {
    #[must_use]
    pub fn shared(self) -> Shared {
        Shared(Rc::new(self))
    }
}

impl Deref for Shared {
    type Target = Node;

    fn deref(&self) -> &Node {
        &self.0
    }
}

impl Http for Shared {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        ready(self.0.answer_request(&request))
    }
}

/// The same JSON with every string in lower case: hex compares whatever its case.
fn lower(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(text.to_ascii_lowercase()),
        Value::Array(items) => Value::Array(items.iter().map(lower).collect()),
        Value::Object(fields) => Value::Object(fields.iter().map(|(key, value)| (key.clone(), lower(value))).collect()),
        other => other.clone(),
    }
}
