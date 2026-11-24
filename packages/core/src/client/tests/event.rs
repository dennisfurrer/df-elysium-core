//! Events against viem's `decodeEventLog` over the same logs (`arbitrum_sdk.json`): live ones from
//! Elysium and HyperEVM testnet, and synthetic ones built with viem's encoders.

use serde_json::Value;

use df_hyperliquid_core::client::evm::abi::{Function, Kind, Token};

use crate::test_support::{arbitrum_sdk, text};

use super::*;

/// The canonical signature of a declaration as abitype writes it.
fn canonical(declared: &str) -> String {
    let declared = declared.trim_start_matches("event ");
    let function = Function::parse(&declared.replace(" indexed", "")).unwrap();
    function.signature()
}

/// A value as viem writes it: decimal integers, lowercase hex, lists, and named tuples as objects.
fn shown(token: &Token, kind: &Kind) -> Value {
    match (token, kind) {
        (Token::Array(items) | Token::Tuple(items), Kind::Array(inner) | Kind::Sized(inner, _)) => {
            Value::Array(items.iter().map(|item| shown(item, inner)).collect())
        }
        (Token::Tuple(items), Kind::Tuple(params)) => named(items, params),
        (Token::Bool(value), _) => Value::Bool(*value),
        _ => Value::String(token.show(kind).to_lowercase()),
    }
}

/// Values by their parameters' names, as viem's decoding keys them.
fn named(tokens: &[Token], params: &[df_hyperliquid_core::client::evm::abi::Param]) -> Value {
    let fields =
        tokens.iter().zip(params).map(|(token, param)| (param.name.clone().unwrap(), shown(token, &param.kind)));
    Value::Object(fields.collect())
}

/// viem's decoding, its hex lowercase.
fn lowered(value: &Value) -> Value {
    match value {
        Value::Object(fields) => {
            Value::Object(fields.iter().map(|(key, value)| (key.clone(), lowered(value))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(lowered).collect()),
        Value::String(text) => Value::String(text.to_lowercase()),
        other => other.clone(),
    }
}

#[test]
fn every_event_viem_decoded_reads_the_same_here() {
    let file = arbitrum_sdk();
    let events = file["events"].as_array().unwrap();
    assert!(events.len() >= 15);
    for case in events {
        let declared = text(&case["signature"]).trim_start_matches("event ");
        let params = &declared[declared.find('(').unwrap() + 1..declared.rfind(')').unwrap()];
        let signature = canonical(declared);
        let event = Event::new(Box::leak(signature.into_boxed_str()), Box::leak(params.to_owned().into_boxed_str()));
        assert_eq!(event.topic().to_hex(), text(&case["topic"]), "{declared}");
        let log: Log = serde_json::from_value(serde_json::json!({
            "address": case["log"]["address"],
            "topics": case["log"]["topics"],
            "data": case["log"]["data"],
        }))
        .unwrap();
        assert!(event.matches(&log));
        let tokens = event.decode(&log).unwrap();
        let kinds = df_hyperliquid_core::client::evm::abi::params(params).unwrap();
        assert_eq!(named(&tokens, &kinds), lowered(&case["decoded"]), "{declared}");
    }
}

#[test]
fn a_log_of_another_event_or_cut_short_is_refused() {
    let event = Event::new("Ping(uint256)", "uint256 indexed id");
    let other = Event::new("Pong(uint256)", "uint256 indexed id");
    // A log as a node answers it, with only the fields the decoder reads.
    let log: Log = serde_json::from_value(serde_json::json!({
        "address": "0x0000000000000000000000000000000000000000",
        "topics": [other.topic().to_hex(), crate::B256::ZERO.to_hex()],
        "data": "0x",
    }))
    .unwrap();
    assert_eq!(event.decode(&log), Err(BadLog::NotThisEvent("Ping")));
    let mut short = log;
    short.topics = vec![event.topic()];
    assert!(matches!(event.decode(&short), Err(BadLog::Abi { event: "Ping", .. })), "its indexed id is missing");
    assert_eq!(event.name(), "Ping");
}
