//! Failures in place of a chain's answer, for its next call, or its next call of one method: an
//! HTTP status, a 429 with its wait, or no answer at all. The call is still taken.

use df_elysium_core::client::{Response, TransportError};

use super::{Answer, Chain, Node};

/// An answer in place of the node's own.
pub(super) struct Failure {
    chain: Chain,
    method: Option<String>,
    answer: Answer,
}

impl Node {
    /// Answers the next call on `chain` (of `method`, when given) with HTTP `status` and `body`.
    #[must_use]
    pub fn status(self, chain: Chain, method: Option<&str>, status: u16, body: &str) -> Node {
        let answer = Ok(Response { status, headers: Vec::new(), body: body.as_bytes().to_vec() });
        self.failing(chain, method, answer)
    }

    /// Answers the next call on `chain` with a 429 whose `Retry-After` asks for `seconds`.
    #[must_use]
    pub fn retry_after(self, chain: Chain, seconds: u64) -> Node {
        let headers = vec![("Retry-After".to_owned(), seconds.to_string())];
        self.failing(chain, None, Ok(Response { status: 429, headers, body: b"null".to_vec() }))
    }

    /// The next call on `chain` (of `method`, when given) is taken but its answer never comes back.
    #[must_use]
    pub fn unreachable(self, chain: Chain, method: Option<&str>) -> Node {
        self.failing(chain, method, Err(TransportError::Unreachable("the connection was reset".to_owned())))
    }

    fn failing(self, chain: Chain, method: Option<&str>, answer: Answer) -> Node {
        self.failures.borrow_mut().push_back(Failure { chain, method: method.map(str::to_owned), answer });
        self
    }

    /// The failure queued first for this call, taken.
    pub(super) fn failure(&self, chain: Chain, method: &str) -> Option<Answer> {
        let mut failures = self.failures.borrow_mut();
        let at = failures
            .iter()
            .position(|failure| failure.chain == chain && failure.method.as_deref().is_none_or(|one| one == method))?;
        failures.remove(at).map(|failure| failure.answer)
    }
}
