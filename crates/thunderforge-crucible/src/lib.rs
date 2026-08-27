//! Spec 024, ADR-047: `SessionAdjudicator` — the seam for server-authoritative
//! movement/manipulation resolution. Two implementations satisfy this trait:
//! [`local::LocalAdjudicator`] (in-process, zero-config, what every
//! self-hosted deployment gets by default) and [`remote::RemoteAdjudicator`]
//! (delegates over HTTP to a standalone `crucible-server` process). Callers
//! depend only on this trait, never on which implementation is active
//! (spec.md FR-006).
//!
//! The ruleset implemented here today is a deliberate placeholder
//! pass-through (ADR-047) — it does not supersede or duplicate the Bevy
//! engine's simulation authority. The real future ruleset is expected to run
//! the same plugin-modular engine code headless, server-side (client
//! predicts, server reconciles), not to be hand-written here.

pub mod local;
pub mod remote;
pub mod server;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The kind of action being adjudicated. Matches spec.md's "movement and
/// manipulation" framing exactly — not a broader action-type enum, to avoid
/// speculative scope (data-model.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Move,
    Manipulate,
}

/// A proposed action to resolve. `payload` is deliberately untyped at this
/// layer (data-model.md) — the placeholder ruleset does not need to
/// interpret it; a typed payload-per-`kind` is a natural evolution once the
/// real ruleset needs to actually inspect it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjudicationRequest {
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub kind: ActionKind,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// The authoritative outcome of resolving an [`AdjudicationRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Accepted,
    Rejected,
    Adjusted,
}

/// The full result of resolving an [`AdjudicationRequest`]. `payload` is
/// present only for `Outcome::Adjusted` (the corrected action); `reason` is
/// present only for `Outcome::Rejected`/`Outcome::Adjusted`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjudicationResult {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AdjudicationResult {
    /// The placeholder ruleset's only ever-produced result (ADR-047) —
    /// unconditional acceptance, carrying no adjustment or rejection reason.
    pub fn accepted() -> Self {
        Self {
            outcome: Outcome::Accepted,
            payload: None,
            reason: None,
        }
    }
}

/// Errors a [`SessionAdjudicator`] implementation can produce.
/// `RemoteUnavailable` is only ever produced by [`remote::RemoteAdjudicator`]
/// — [`local::LocalAdjudicator`] never produces it (data-model.md).
#[derive(Debug, thiserror::Error)]
pub enum SessionAdjudicatorError {
    #[error("the configured remote adjudicator could not be reached: {0}")]
    RemoteUnavailable(String),
    #[error("invalid adjudication request: {0}")]
    InvalidRequest(String),
}

/// The capability/contract for resolving a proposed movement or
/// manipulation action into an authoritative result (spec.md FR-001).
/// `dyn`-compatible so `AppState` can hold `Arc<dyn SessionAdjudicator + Send + Sync>`
/// regardless of which implementation is active.
#[async_trait::async_trait]
pub trait SessionAdjudicator {
    async fn resolve(
        &self,
        request: AdjudicationRequest,
    ) -> Result<AdjudicationResult, SessionAdjudicatorError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjudication_request_round_trips_through_json() {
        let request = AdjudicationRequest {
            world_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::json!({ "x": 1, "y": 2 }),
        };
        let json = serde_json::to_string(&request).expect("serialize");
        let round_tripped: AdjudicationRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_tripped.world_id, request.world_id);
        assert_eq!(round_tripped.actor_id, request.actor_id);
        assert_eq!(round_tripped.kind, request.kind);
        assert_eq!(round_tripped.payload, request.payload);
    }

    #[test]
    fn adjudication_result_round_trips_through_json_for_every_outcome() {
        for result in [
            AdjudicationResult::accepted(),
            AdjudicationResult {
                outcome: Outcome::Rejected,
                payload: None,
                reason: Some("out of range".to_string()),
            },
            AdjudicationResult {
                outcome: Outcome::Adjusted,
                payload: Some(serde_json::json!({ "x": 3, "y": 4 })),
                reason: Some("clamped to max distance".to_string()),
            },
        ] {
            let json = serde_json::to_string(&result).expect("serialize");
            let round_tripped: AdjudicationResult =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(round_tripped.outcome, result.outcome);
            assert_eq!(round_tripped.payload, result.payload);
            assert_eq!(round_tripped.reason, result.reason);
        }
    }
}
