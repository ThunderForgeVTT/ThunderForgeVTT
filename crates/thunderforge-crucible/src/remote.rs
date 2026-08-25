//! Spec 024 (User Story 2): `RemoteAdjudicator` — delegates to a standalone
//! `crucible-server` process over HTTP, per
//! `specs/024-thunderforge-crucible-crate/contracts/crucible-server-http.md`.
//! Selected via `CRUCIBLE_MODE=remote` + `CRUCIBLE_ENDPOINT` (`main.rs`).

use std::time::Duration;

use reqwest::{Client, StatusCode, Url};

use crate::{AdjudicationRequest, AdjudicationResult, SessionAdjudicator, SessionAdjudicatorError};

/// Bounded timeout for a single adjudication call — research.md §3: a fixed
/// constant for this spec, not user-configurable yet. Keeps SC-004's "clear
/// error within a bounded time, not an indefinite hang" true regardless of
/// TCP-level timeout defaults.
const ADJUDICATE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct RemoteAdjudicator {
    client: Client,
    endpoint: Url,
}

impl RemoteAdjudicator {
    pub fn new(endpoint: Url) -> Self {
        Self {
            client: Client::new(),
            endpoint,
        }
    }

    fn adjudicate_url(&self) -> Url {
        self.endpoint
            .join("/adjudicate")
            .expect("endpoint + \"/adjudicate\" must always be a valid URL")
    }
}

#[async_trait::async_trait]
impl SessionAdjudicator for RemoteAdjudicator {
    async fn resolve(
        &self,
        request: AdjudicationRequest,
    ) -> Result<AdjudicationResult, SessionAdjudicatorError> {
        let response = self
            .client
            .post(self.adjudicate_url())
            .timeout(ADJUDICATE_TIMEOUT)
            .json(&request)
            .send()
            .await
            .map_err(|err| SessionAdjudicatorError::RemoteUnavailable(err.to_string()))?;

        match response.status() {
            StatusCode::OK => response
                .json::<AdjudicationResult>()
                .await
                .map_err(|err| SessionAdjudicatorError::RemoteUnavailable(err.to_string())),
            StatusCode::BAD_REQUEST => {
                let body = response.text().await.unwrap_or_default();
                Err(SessionAdjudicatorError::InvalidRequest(body))
            }
            other => Err(SessionAdjudicatorError::RemoteUnavailable(format!(
                "unexpected status {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionKind;
    use uuid::Uuid;

    /// Spec 024 (SC-002): `RemoteAdjudicator` against a locally-spawned
    /// `crucible-server` router must produce identical results to
    /// `LocalAdjudicator` for the same input — proven here in-process, no
    /// separately-run process needed in CI (plan.md's Testing note).
    #[tokio::test]
    async fn produces_identical_results_to_local_adjudicator() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, crate::server::router())
                .await
                .expect("serve crucible-server router");
        });

        let endpoint = Url::parse(&format!("http://{addr}")).unwrap();
        let remote = RemoteAdjudicator::new(endpoint);
        let local = crate::local::LocalAdjudicator;

        let request = AdjudicationRequest {
            world_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::json!({ "x": 5, "y": 6 }),
        };

        let remote_result = remote
            .resolve(request.clone())
            .await
            .expect("remote adjudicator must succeed");
        let local_result = local
            .resolve(request)
            .await
            .expect("local adjudicator must succeed");

        assert_eq!(remote_result.outcome, local_result.outcome);
        assert_eq!(remote_result.payload, local_result.payload);
        assert_eq!(remote_result.reason, local_result.reason);
    }

    /// Spec 024 (SC-004): an unreachable remote adjudicator must produce a
    /// clear error within the bounded timeout, not an indefinite hang.
    #[tokio::test]
    async fn unreachable_endpoint_produces_a_clear_error() {
        // Bind then immediately drop the listener: the port is very likely
        // closed again by the time we connect, giving a fast connection
        // refusal rather than waiting out the full timeout — keeps this
        // test itself fast without weakening what it proves (a clear,
        // bounded-time error either way).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        let endpoint = Url::parse(&format!("http://{addr}")).unwrap();
        let remote = RemoteAdjudicator::new(endpoint);

        let request = AdjudicationRequest {
            world_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::Value::Null,
        };

        let result = remote.resolve(request).await;

        assert!(matches!(
            result,
            Err(SessionAdjudicatorError::RemoteUnavailable(_))
        ));
    }
}
