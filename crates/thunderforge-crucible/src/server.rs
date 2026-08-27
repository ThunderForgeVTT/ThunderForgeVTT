//! Spec 024 (User Story 2): the HTTP surface `crucible-server` exposes —
//! `POST /adjudicate` and `GET /health` — per
//! `specs/024-thunderforge-crucible-crate/contracts/crucible-server-http.md`.
//! Reused by both the `crucible-server` binary (`bin/crucible-server.rs`)
//! and the in-process integration test that proves `RemoteAdjudicator`
//! produces identical results to `LocalAdjudicator` (quickstart.md), so
//! there is exactly one source of truth for this contract regardless of how
//! it's deployed.

use axum::{
    Json, Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};

use crate::local::LocalAdjudicator;
use crate::{AdjudicationRequest, SessionAdjudicator, SessionAdjudicatorError};

async fn adjudicate(Json(request): Json<AdjudicationRequest>) -> impl IntoResponse {
    match LocalAdjudicator.resolve(request).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(SessionAdjudicatorError::InvalidRequest(message)) => {
            (StatusCode::BAD_REQUEST, message).into_response()
        }
        // `LocalAdjudicator` never produces `RemoteUnavailable` — see
        // data-model.md — but handled explicitly rather than silently
        // dropped, in case a future non-placeholder ruleset changes that.
        Err(SessionAdjudicatorError::RemoteUnavailable(message)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
        }
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}

/// Builds the router `crucible-server` serves — per
/// contracts/crucible-server-http.md.
pub fn router() -> Router {
    Router::new()
        .route("/adjudicate", post(adjudicate))
        .route("/health", get(health))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionKind;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn adjudicate_accepts_a_well_formed_request() {
        let app = router();
        let request = AdjudicationRequest {
            world_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::json!({ "x": 1, "y": 2 }),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/adjudicate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let result: crate::AdjudicationResult = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.outcome, crate::Outcome::Accepted);
    }

    #[tokio::test]
    async fn adjudicate_rejects_a_malformed_request_with_400() {
        let app = router();
        let request = AdjudicationRequest {
            world_id: Uuid::nil(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::Value::Null,
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/adjudicate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
