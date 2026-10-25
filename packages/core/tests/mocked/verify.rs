//! Source verification against Conduit's Sourcify as it answered, recorded verbatim under
//! `fixtures/sourcify/`: an exact match, a contract never verified (404 with every match `null`),
//! a job that does not exist, and a submission refused for its body. The requests go to Sourcify's
//! v2 paths with the body its API takes.

use df_elysium_core::client::HttpMethod;
use df_elysium_core::deploy::verify::{self, Match, Refusal, VerifyError};
use df_elysium_core::{B256, Network};
use serde_json::{Value, json};

use crate::support::node::{Node, Sent};
use crate::support::{address, fixture};

fn recording(name: &str) -> Value {
    serde_json::from_str(&fixture(&format!("sourcify/{name}.json"))).unwrap()
}

/// A node serving each recording at its URL.
fn sourcify() -> Node {
    ["status_exact", "status_unverified", "job_not_found", "submit_invalid"].iter().map(|name| recording(name)).fold(
        Node::testnet(),
        |node, said| {
            let method = if said["method"] == "GET" { HttpMethod::Get } else { HttpMethod::Post };
            let status = u16::try_from(said["status"].as_u64().unwrap()).unwrap();
            node.page(method, said["url"].as_str().unwrap(), status, said["body"].as_str().unwrap())
        },
    )
}

#[tokio::test]
async fn a_verified_contract_reads_exact_and_one_never_verified_reads_unverified() {
    let node = sourcify();
    let escrow = address("0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB");
    let status = verify::status(&node, Network::Testnet, escrow).await.unwrap();
    assert_eq!(
        (status.address, status.matched, status.creation, status.runtime),
        (escrow, Some(Match::Exact), Some(Match::Exact), Some(Match::Exact))
    );
    assert_eq!(status.verified_at.as_deref(), Some("2026-10-25T08:38:44Z"));
    assert!(status.is_verified());

    let nobody = address("0x0000000000000000000000000000000000000001");
    let status = verify::status(&node, Network::Testnet, nobody).await.unwrap();
    assert_eq!((status.matched, status.creation, status.runtime), (None, None, None), "a 404 that still answers");
    assert!(!status.is_verified());

    let urls: Vec<String> = node
        .sent()
        .into_iter()
        .filter_map(|sent| if let Sent::Web { url, .. } = sent { Some(url.to_ascii_lowercase()) } else { None })
        .collect();
    let recorded =
        ["status_exact", "status_unverified"].map(|name| recording(name)["url"].as_str().unwrap().to_ascii_lowercase());
    assert_eq!(urls, recorded);
}

#[tokio::test]
async fn a_submission_goes_out_as_sourcify_takes_it_and_its_refusals_read_in_its_words() {
    let node = sourcify();
    let missing = verify::job(&node, Network::Testnet, "00000000-0000-0000-0000-000000000000").await;
    let Err(VerifyError::Refused(Refusal { code, message })) = missing else { panic!("{missing:?}") };
    assert_eq!(code, "job_not_found");
    assert!(message.contains("00000000-0000-0000-0000-000000000000"), "{message}");

    let input = r#"{"language":"Solidity","sources":{"src/A.sol":{"content":"contract A {}"}},"settings":{"evmVersion":"osaka"}}"#;
    let creation: B256 = "0xe6f595b8aa000000000000000000000000000000000000000000000000079e5c".parse().unwrap();
    let nobody = address("0x0000000000000000000000000000000000000001");
    let refused =
        verify::submit(&node, Network::Testnet, nobody, input, "0.8.37+commit.f401782d", "src/A.sol:A", Some(creation))
            .await;
    let Err(VerifyError::Refused(refusal)) = refused else { panic!("{refused:?}") };
    assert_eq!(refusal.code, "invalid_parameter", "the recorded refusal, served whatever the body");

    let Some(Sent::Web { method, url, body }) = node.sent().last().cloned() else { panic!("nothing sent") };
    assert_eq!((method, url), (HttpMethod::Post, recording("submit_invalid")["url"].as_str().unwrap().to_owned()));
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "stdJsonInput": serde_json::from_str::<Value>(input).unwrap(),
            "compilerVersion": "0.8.37+commit.f401782d",
            "contractIdentifier": "src/A.sol:A",
            "creationTransactionHash": creation.to_hex(),
        })
    );
    assert!(matches!(
        verify::submit_request(Network::Testnet, nobody, "not json", "0.8.37", "A", None),
        Err(VerifyError::Unreadable(_))
    ));
}
