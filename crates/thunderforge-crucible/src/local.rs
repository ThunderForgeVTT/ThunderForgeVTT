//! Spec 024 (User Story 1): `LocalAdjudicator` — in-process, zero-config,
//! no network. What every self-hosted deployment gets by default
//! (`CRUCIBLE_MODE=local`, the default when unset).
//!
//! The placeholder ruleset here is deliberate (ADR-047): it unconditionally
//! accepts any well-formed request. It does not supersede or duplicate the
//! Bevy engine's simulation authority — the real future ruleset is expected
//! to run the same plugin-modular engine code headless, server-side, not to
//! be hand-written here.

use crate::{AdjudicationRequest, AdjudicationResult, SessionAdjudicator, SessionAdjudicatorError};

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalAdjudicator;

#[async_trait::async_trait]
impl SessionAdjudicator for LocalAdjudicator {
    async fn resolve(
        &self,
        request: AdjudicationRequest,
    ) -> Result<AdjudicationResult, SessionAdjudicatorError> {
        if request.world_id.is_nil() || request.actor_id.is_nil() {
            return Err(SessionAdjudicatorError::InvalidRequest(
                "world_id and actor_id must be non-nil".to_string(),
            ));
        }
        Ok(AdjudicationResult::accepted())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionKind;
    use uuid::Uuid;

    #[tokio::test]
    async fn accepts_a_well_formed_request() {
        let adjudicator = LocalAdjudicator;
        let request = AdjudicationRequest {
            world_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Move,
            payload: serde_json::json!({ "x": 1, "y": 2 }),
        };

        let result = adjudicator
            .resolve(request)
            .await
            .expect("a well-formed request must be accepted");

        assert_eq!(result.outcome, crate::Outcome::Accepted);
    }

    #[tokio::test]
    async fn rejects_a_malformed_request() {
        let adjudicator = LocalAdjudicator;
        let request = AdjudicationRequest {
            world_id: Uuid::nil(),
            actor_id: Uuid::now_v7(),
            kind: ActionKind::Manipulate,
            payload: serde_json::Value::Null,
        };

        let result = adjudicator.resolve(request).await;

        assert!(matches!(
            result,
            Err(SessionAdjudicatorError::InvalidRequest(_))
        ));
    }
}
