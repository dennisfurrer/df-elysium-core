//! Source verification on Conduit's Sourcify ([`Network::verifier_url`]), the one Elysium's
//! explorer reads; its own reports no verification API. Sourcify's v2 API: a contract's status,
//! a submission of its standard JSON input, and the job that verifies it. A submission is
//! answered with a job id at once; the job finishes in seconds.
//!
//! Foundry does the same with `forge verify-contract --verifier sourcify --verifier-url
//! https://contracts.conduit.xyz <address> src/<File>.sol:<Contract>`.

use serde::Deserialize;

use df_hyperliquid_core::client::{Http, Request, Response, TransportError};

use crate::B256;
use crate::client::Network;
use crate::utils::{Address, Json};

/// How well the deployed code matches the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Match {
    /// Byte for byte, metadata hash included.
    Exact,
    /// All but the metadata hash.
    Partial,
}

/// A contract's verification, as Sourcify keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub address: Address,
    /// `None` for a contract never verified.
    pub matched: Option<Match>,
    pub creation: Option<Match>,
    pub runtime: Option<Match>,
    /// ISO 8601, as Sourcify writes it.
    pub verified_at: Option<String>,
}

impl Status {
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        self.matched.is_some()
    }
}

/// A verification job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub completed: bool,
    /// Once completed: the contract as verified, or why not.
    pub outcome: Option<Result<Status, Refusal>>,
}

/// Sourcify's refusal in its own words: `job_not_found`, `invalid_parameter` and the like.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Refusal {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("Sourcify refused: {0}")]
    Refused(Refusal),
    #[error("Sourcify's answer could not be read: {0}")]
    Unreadable(String),
}

/// `GET /v2/contract/{chainId}/{address}`: answered for a contract never verified too, with
/// every match `null` (and HTTP 404).
#[must_use]
pub fn status_request(network: Network, address: Address) -> Request {
    Request::get(format!("{}/v2/contract/{}/{address}", network.verifier_url(), network.chain_id()))
}

/// `POST /v2/verify/{chainId}/{address}`: the compiler's standard JSON input as written, its
/// version (`0.8.37+commit.f401782d`), the contract (`src/Escrow.sol:Escrow`), and the creation's
/// transaction when known, which lets the creation bytecode match too.
pub fn submit_request(
    network: Network,
    address: Address,
    std_json_input: &str,
    compiler_version: &str,
    contract: &str,
    creation: Option<B256>,
) -> Result<Request, VerifyError> {
    let input = Json::parse(std_json_input).map_err(|error| VerifyError::Unreadable(error.to_string()))?;
    let mut body = vec![
        ("stdJsonInput".to_owned(), input),
        ("compilerVersion".to_owned(), Json::String(compiler_version.to_owned())),
        ("contractIdentifier".to_owned(), Json::String(contract.to_owned())),
    ];
    if let Some(hash) = creation {
        body.push(("creationTransactionHash".to_owned(), Json::String(hash.to_hex())));
    }
    let url = format!("{}/v2/verify/{}/{address}", network.verifier_url(), network.chain_id());
    Ok(Request::post_json(url, Json::Object(body).compact()))
}

/// `GET /v2/verify/{verificationId}`.
#[must_use]
pub fn job_request(network: Network, id: &str) -> Request {
    Request::get(format!("{}/v2/verify/{id}", network.verifier_url()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusBody {
    address: Address,
    #[serde(rename = "match")]
    matched: Option<String>,
    creation_match: Option<String>,
    runtime_match: Option<String>,
    #[serde(default)]
    verified_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefusalBody {
    custom_code: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobBody {
    verification_id: String,
    is_job_completed: bool,
    #[serde(default)]
    contract: Option<StatusBody>,
    #[serde(default)]
    error: Option<RefusalBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Submitted {
    verification_id: String,
}

fn level(text: Option<&str>) -> Option<Match> {
    match text {
        Some("exact_match") => Some(Match::Exact),
        Some("match") => Some(Match::Partial),
        _ => None,
    }
}

impl From<StatusBody> for Status {
    fn from(body: StatusBody) -> Status {
        Status {
            address: body.address,
            matched: level(body.matched.as_deref()),
            creation: level(body.creation_match.as_deref()),
            runtime: level(body.runtime_match.as_deref()),
            verified_at: body.verified_at,
        }
    }
}

fn unreadable(error: &serde_json::Error, text: &str) -> VerifyError {
    VerifyError::Unreadable(format!("{error} in {}", text.chars().take(300).collect::<String>()))
}

/// A refusal when the body is one, else the status as a transport error.
fn refused(response: &Response) -> VerifyError {
    let text = response.text();
    match serde_json::from_str::<RefusalBody>(&text) {
        Ok(body) => VerifyError::Refused(Refusal { code: body.custom_code, message: body.message }),
        Err(_) => TransportError::Status { status: response.status, body: text }.into(),
    }
}

/// A [`status_request`]'s answer.
pub fn read_status(response: &Response) -> Result<Status, VerifyError> {
    if !response.is_success() && response.status != 404 {
        return Err(refused(response));
    }
    let text = response.text();
    serde_json::from_str::<StatusBody>(&text).map(Status::from).map_err(|error| match response.status {
        404 => refused(response),
        _ => unreadable(&error, &text),
    })
}

/// A [`submit_request`]'s answer: the job's id.
pub fn read_submitted(response: &Response) -> Result<String, VerifyError> {
    if !response.is_success() {
        return Err(refused(response));
    }
    let text = response.text();
    serde_json::from_str::<Submitted>(&text).map(|body| body.verification_id).map_err(|error| unreadable(&error, &text))
}

/// A [`job_request`]'s answer.
pub fn read_job(response: &Response) -> Result<Job, VerifyError> {
    if !response.is_success() {
        return Err(refused(response));
    }
    let text = response.text();
    let body: JobBody = serde_json::from_str(&text).map_err(|error| unreadable(&error, &text))?;
    let outcome = match (body.is_job_completed, body.error, body.contract) {
        (false, _, _) => None,
        (true, Some(error), _) => Some(Err(Refusal { code: error.custom_code, message: error.message })),
        (true, None, Some(contract)) => Some(Ok(contract.into())),
        (true, None, None) => None,
    };
    Ok(Job { id: body.verification_id, completed: body.is_job_completed, outcome })
}

/// A contract's verification, read once.
pub async fn status<H: Http>(http: &H, network: Network, address: Address) -> Result<Status, VerifyError> {
    read_status(&http.send(status_request(network, address)).await?)
}

/// Submits a verification once; the job's id.
pub async fn submit<H: Http>(
    http: &H,
    network: Network,
    address: Address,
    std_json_input: &str,
    compiler_version: &str,
    contract: &str,
    creation: Option<B256>,
) -> Result<String, VerifyError> {
    let request = submit_request(network, address, std_json_input, compiler_version, contract, creation)?;
    read_submitted(&http.send(request).await?)
}

/// A job, read once.
pub async fn job<H: Http>(http: &H, network: Network, id: &str) -> Result<Job, VerifyError> {
    read_job(&http.send(job_request(network, id)).await?)
}

#[cfg(test)]
#[path = "tests/verify.rs"]
mod tests;
