//! The browser's wallet (`window.ethereum`, EIP-1193), without a library: connect, switch to the
//! chain (adding it first when the wallet does not know it), and send a transaction the SDK built,
//! with the SDK's gas estimate and fees. The wallet keeps its own nonce and signs.

use df_elysium_core::client::{Call, CallRequest, with_headroom};
use df_elysium_core::{Address, B256, gas};
use js_sys::{JSON, Promise, Reflect};
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::net::{Chain, NETWORK};

fn ethereum() -> Option<JsValue> {
    let window = web_sys::window()?;
    Reflect::get(&window, &JsValue::from_str("ethereum")).ok().filter(|value| !value.is_undefined())
}

/// Whether the page has a wallet to connect.
pub fn present() -> bool {
    ethereum().is_some()
}

/// `ethereum.request({method, params})`, its answer as JSON.
async fn request(method: &str, params: Value) -> Result<Value, String> {
    let ethereum = ethereum().ok_or("no wallet in this browser")?;
    let body = JSON::parse(&json!({"method": method, "params": params}).to_string()).map_err(text)?;
    let request = Reflect::get(&ethereum, &JsValue::from_str("request")).map_err(text)?;
    let request: js_sys::Function = request.dyn_into().map_err(text)?;
    let promise: Promise = request.call1(&ethereum, &body).map_err(text)?.dyn_into().map_err(text)?;
    let answer = JsFuture::from(promise).await.map_err(text)?;
    if answer.is_undefined() || answer.is_null() {
        return Ok(Value::Null);
    }
    let written = JSON::stringify(&answer).map_err(text)?.as_string().unwrap_or_default();
    serde_json::from_str(&written).map_err(|error| error.to_string())
}

/// A wallet's refusal: its message when it gave one, and its code (4001 is the user saying no).
fn text(error: JsValue) -> String {
    let field = |name: &str| Reflect::get(&error, &JsValue::from_str(name)).ok();
    let message = field("message").and_then(|message| message.as_string());
    let code = field("code").and_then(|code| code.as_f64()).map(|code| format!(" ({code})"));
    match message {
        Some(message) => format!("{message}{}", code.unwrap_or_default()),
        None => error.as_string().unwrap_or_else(|| "the wallet refused".to_owned()),
    }
}

/// Asks the wallet for its account.
pub async fn connect() -> Result<Address, String> {
    let accounts = request("eth_requestAccounts", json!([])).await?;
    let first = accounts.get(0).and_then(Value::as_str).ok_or("the wallet named no account")?;
    first.parse().map_err(|_| format!("`{first}` is not an address"))
}

/// What `wallet_addEthereumChain` is handed for `chain`.
fn chain_params(chain: Chain) -> Value {
    let currency = json!({"name": "HYPE", "symbol": "HYPE", "decimals": 18});
    match chain {
        Chain::Elysium => json!({
            "chainId": format!("{:#x}", chain.id()),
            "chainName": "Elysium Testnet",
            "nativeCurrency": currency,
            "rpcUrls": [NETWORK.rpc_url()],
            "blockExplorerUrls": [NETWORK.explorer_url()],
        }),
        Chain::HyperEvm => json!({
            "chainId": format!("{:#x}", chain.id()),
            "chainName": "HyperEVM Testnet",
            "nativeCurrency": currency,
            "rpcUrls": [NETWORK.hyperliquid().evm_rpc_url()],
        }),
    }
}

/// Puts the wallet on `chain`, adding it when the wallet does not know it.
async fn switch(chain: Chain) -> Result<(), String> {
    let id = json!([{"chainId": format!("{:#x}", chain.id())}]);
    match request("wallet_switchEthereumChain", id).await {
        Ok(_) => Ok(()),
        Err(_) => request("wallet_addEthereumChain", json!([chain_params(chain)])).await.map(|_| ()),
    }
}

/// Sends `call` from `from` on `chain` through the wallet: the SDK estimates its gas with 30% over
/// and caps its fee at twice the base fee with no tip; the wallet signs and sends it once.
pub async fn send(chain: Chain, from: Address, call: Call) -> Result<B256, String> {
    let client = chain.reader();
    let gas =
        client.rpc().estimate_gas(&CallRequest::of(Some(from), &call)).await.map_err(|error| error.to_string())?;
    let fees = gas::fees(client.gas_price().await.map_err(|error| error.to_string())?);
    switch(chain).await?;
    let mut tx = json!({
        "from": from.to_hex(),
        "value": call.value.to_quantity(),
        "data": df_elysium_core::utils::hex::encode_prefixed(&call.data),
        "gas": format!("{:#x}", with_headroom(gas)),
        "maxFeePerGas": fees.max_fee.to_quantity(),
        "maxPriorityFeePerGas": fees.max_priority_fee.to_quantity(),
    });
    if let (Some(to), Some(fields)) = (call.to, tx.as_object_mut()) {
        fields.insert("to".to_owned(), Value::String(to.to_hex()));
    }
    let hash = request("eth_sendTransaction", json!([tx])).await?;
    let hash = hash.as_str().ok_or("the wallet answered no hash")?;
    hash.parse().map_err(|_| format!("`{hash}` is not a transaction hash"))
}
