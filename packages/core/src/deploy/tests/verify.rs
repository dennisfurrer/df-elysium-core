//! Conduit's Sourcify v2 as it answered on 2026-10-25: the reactor escrow on Elysium testnet
//! verified `exact_match` both ways, an address never verified (404, every match `null`), a job
//! that does not exist, and a submission without its input.

use crate::client::{HttpMethod, Response};

use super::*;

fn answered(status: u16, body: &str) -> Response {
    Response { status, headers: Vec::new(), body: body.as_bytes().to_vec() }
}

const ESCROW: &str = r#"{"matchId":"475585","creationMatch":"exact_match","runtimeMatch":"exact_match","verifiedAt":"2026-10-25T08:38:44Z","match":"exact_match","chainId":"99801","address":"0xD5f63c6f776555BA0b5976408d4F4be2Ec5B6FdB"}"#;
const NEVER: &str = r#"{"match":null,"creationMatch":null,"runtimeMatch":null,"chainId":"99801","address":"0x0000000000000000000000000000000000000001"}"#;
const NO_JOB: &str = r#"{"customCode":"job_not_found","message":"No verification job found for id 00000000-0000-0000-0000-000000000000","errorId":"a5a885bf-2096-4493-97a0-edaa635ef42b"}"#;
const NO_INPUT: &str = r#"{"customCode":"invalid_parameter","message":"request/body must have required property 'stdJsonInput'","errorId":"4b06530a-2c99-46cb-b652-799707f311c8"}"#;

#[test]
fn a_status_reads_verified_and_never_verified_alike() {
    let escrow = read_status(&answered(200, ESCROW)).unwrap();
    assert_eq!(escrow.address.to_hex(), "0xd5f63c6f776555ba0b5976408d4f4be2ec5b6fdb");
    assert_eq!(
        (escrow.matched, escrow.creation, escrow.runtime),
        (Some(Match::Exact), Some(Match::Exact), Some(Match::Exact))
    );
    assert_eq!(escrow.verified_at.as_deref(), Some("2026-10-25T08:38:44Z"));
    assert!(escrow.is_verified());
    let never = read_status(&answered(404, NEVER)).unwrap();
    assert_eq!((never.matched, never.is_verified()), (None, false));
    let request = status_request(Network::Testnet, escrow.address);
    assert_eq!(request.method, HttpMethod::Get);
    assert_eq!(
        request.url,
        "https://contracts.conduit.xyz/v2/contract/99801/0xd5f63c6f776555ba0b5976408d4f4be2ec5b6fdb"
    );
}

#[test]
fn a_refusal_reads_in_sourcifys_own_words() {
    let refused = read_job(&answered(404, NO_JOB)).unwrap_err();
    assert_eq!(
        refused,
        VerifyError::Refused(Refusal {
            code: "job_not_found".to_owned(),
            message: "No verification job found for id 00000000-0000-0000-0000-000000000000".to_owned()
        })
    );
    let refused = read_submitted(&answered(400, NO_INPUT)).unwrap_err();
    assert!(matches!(&refused, VerifyError::Refused(refusal) if refusal.code == "invalid_parameter"), "{refused:?}");
    assert!(matches!(read_status(&answered(502, "bad gateway")), Err(VerifyError::Transport(_))));
    assert_eq!(job_request(Network::Testnet, "abc").url, "https://contracts.conduit.xyz/v2/verify/abc");
}

#[test]
fn a_submission_carries_the_input_as_written_and_a_job_reads_to_its_end() {
    let input = r#"{"language":"Solidity","sources":{"src/A.sol":{"content":"contract A {}"}},"settings":{"evmVersion":"osaka"}}"#;
    let creation: B256 =
        "0xe6f595b8000000000000000000000000000000000000000000000000000079e5c".parse::<B256>().unwrap_or(B256::ZERO);
    let request =
        submit_request(Network::Testnet, Address::ZERO, input, "0.8.37+commit.f401782d", "src/A.sol:A", Some(creation))
            .unwrap();
    assert_eq!(request.url, format!("https://contracts.conduit.xyz/v2/verify/99801/{}", Address::ZERO));
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["stdJsonInput"], serde_json::from_str::<serde_json::Value>(input).unwrap());
    assert_eq!(
        (body["compilerVersion"].as_str(), body["contractIdentifier"].as_str()),
        (Some("0.8.37+commit.f401782d"), Some("src/A.sol:A"))
    );
    assert_eq!(body["creationTransactionHash"], creation.to_hex());
    assert!(submit_request(Network::Testnet, Address::ZERO, "{not json", "v", "c", None).is_err());

    assert_eq!(read_submitted(&answered(202, r#"{"verificationId":"72a5"}"#)), Ok("72a5".to_owned()));
    let running = read_job(&answered(
        200,
        r#"{"isJobCompleted":false,"verificationId":"72a5","jobStartTime":"2026-10-25T08:38:40Z"}"#,
    ))
    .unwrap();
    assert_eq!((running.completed, running.outcome), (false, None));
    let done = format!(r#"{{"isJobCompleted":true,"verificationId":"72a5","contract":{ESCROW}}}"#);
    let done = read_job(&answered(200, &done)).unwrap();
    assert!(matches!(done.outcome, Some(Ok(Status { matched: Some(Match::Exact), .. }))), "{done:?}");
    let failed = r#"{"isJobCompleted":true,"verificationId":"72a5","error":{"customCode":"no_match","message":"the bytecode does not match"}}"#;
    assert!(matches!(read_job(&answered(200, failed)).unwrap().outcome, Some(Err(Refusal { .. }))));
}
