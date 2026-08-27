//! Spec 015: DMCA notice-and-takedown content moderation.
//!
//! `effective_status` is the single enforcement primitive every content
//! read path (actors, items, lore) must consult before returning data —
//! see contracts/graphql-moderation.md's enforcement contract. Auto-
//! restoration (FR-007) is evaluated lazily here rather than via a
//! background job: the first read after `restoration_due_at` has passed
//! with no intervening `resolveModerationCase` call materializes a real
//! `content_restored` row before returning, so the audit trail stays
//! complete without new scheduler infrastructure (see tasks.md's header
//! note and research.md R3).

pub mod validation;

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::models::{ContentModerationAction, NewContentModerationAction};
use crate::schema::content_moderation_actions;
use crate::state::AppState;
use async_graphql::{Error, Result as GraphQLResult};

/// Config: how many days a claimant has to file further legal action
/// after a counter-notice is forwarded before content auto-restores
/// (17 U.S.C. § 512(g)(2)(C); research.md R3 — a policy value, not an
/// engineering constant, hence env-configurable).
pub fn counter_notice_waiting_period_days() -> i64 {
    std::env::var("MODERATION_COUNTER_NOTICE_WAITING_PERIOD_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14)
}

/// Config: how many days back to look when counting an account's
/// unresolved-in-their-favor takedown cases for repeat-infringer
/// evaluation (FR-009).
pub fn repeat_infringer_lookback_days() -> i64 {
    std::env::var("MODERATION_REPEAT_INFRINGER_LOOKBACK_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(365)
}

/// Config: how many upheld, non-restored cases within the lookback
/// window flags an account for the repeat-infringer review/termination
/// path.
pub fn repeat_infringer_threshold() -> i64 {
    std::env::var("MODERATION_REPEAT_INFRINGER_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
}

/// Action-type string constants — kept centralized so every module that
/// writes/reads `action_type` uses the exact same literal (data-model.md's
/// enum, stored as plain text like this codebase's other status columns).
pub mod action_type {
    pub const NOTICE_RECEIVED: &str = "notice_received";
    pub const NOTICE_REJECTED_INCOMPLETE: &str = "notice_rejected_incomplete";
    pub const CONTENT_DISABLED: &str = "content_disabled";
    pub const COUNTER_NOTICE_RECEIVED: &str = "counter_notice_received";
    pub const COUNTER_NOTICE_FORWARDED: &str = "counter_notice_forwarded";
    pub const CONTENT_RESTORED: &str = "content_restored";
    pub const CONTENT_REMAINS_DISABLED: &str = "content_remains_disabled";
}

/// Whether an `action_type` means "this entity's real content should not
/// be shown" — the single source of truth the enforcement contract in
/// contracts/graphql-moderation.md depends on.
fn is_disabled_status(action_type: &str) -> bool {
    matches!(
        action_type,
        action_type::CONTENT_DISABLED | action_type::CONTENT_REMAINS_DISABLED
    )
}

/// Loads the most recent moderation event for `(entity_type, entity_id)`
/// across all its cases (there should only ever be one active case per
/// entity, but this always takes the latest event overall to be safe).
fn latest_event(
    conn: &mut PgConnection,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<Option<ContentModerationAction>, diesel::result::Error> {
    content_moderation_actions::table
        .filter(content_moderation_actions::entity_type.eq(entity_type))
        .filter(content_moderation_actions::entity_id.eq(entity_id))
        .order(content_moderation_actions::created_at.desc())
        .select(ContentModerationAction::as_select())
        .first::<ContentModerationAction>(conn)
        .optional()
}

/// The enforcement primitive (contracts/graphql-moderation.md's
/// `moderationStatus` query, and every content read path's gate).
/// Returns `None` when the entity is fully visible (no case, or its
/// latest event is `content_restored`/`notice_rejected_incomplete`);
/// `Some(action_type::CONTENT_DISABLED)` or
/// `Some(action_type::CONTENT_REMAINS_DISABLED)` when it must be hidden
/// from list queries / placeholder'd in single-entity queries.
pub async fn effective_status(
    state: &AppState,
    entity_type: &str,
    entity_id: Uuid,
) -> GraphQLResult<Option<String>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let entity_type_owned = entity_type.to_string();

    let latest =
        tokio::task::spawn_blocking(move || latest_event(&mut conn, &entity_type_owned, entity_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to load moderation status"))?;

    let Some(event) = latest else {
        return Ok(None);
    };

    // Lazy auto-restoration: a forwarded counter-notice whose waiting
    // period has elapsed, with no newer event recorded, restores now.
    if event.action_type == action_type::COUNTER_NOTICE_FORWARDED {
        if let Some(due_at) = event.restoration_due_at
            && Utc::now() >= due_at
        {
            let case_id = event.case_id;
            let world_id = event.world_id;
            let entity_id_for_insert = event.entity_id;
            let entity_type_for_insert = event.entity_type.clone();
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;

            tokio::task::spawn_blocking(move || {
                diesel::insert_into(content_moderation_actions::table)
                    .values(NewContentModerationAction {
                        case_id,
                        action_type: action_type::CONTENT_RESTORED.to_string(),
                        entity_type: entity_type_for_insert,
                        entity_id: entity_id_for_insert,
                        world_id,
                        account_id: None,
                        claimant_name: String::new(),
                        claimant_contact: String::new(),
                        copyrighted_work_description: String::new(),
                        infringing_material_location: String::new(),
                        good_faith_statement: false,
                        accuracy_statement: false,
                        signature: String::new(),
                        validity_result: None,
                        missing_elements: None,
                        counter_notice_id: None,
                        restoration_due_at: None,
                        created_by: None,
                    })
                    .execute(&mut conn)
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to record auto-restoration"))?;

            return Ok(None);
        }
        // Waiting period not yet elapsed: content stays disabled while forwarded.
        return Ok(Some(action_type::CONTENT_DISABLED.to_string()));
    }

    if is_disabled_status(&event.action_type) {
        Ok(Some(event.action_type))
    } else {
        Ok(None)
    }
}

/// The `case_id` of the entity's currently-active disabling case, if any —
/// used so an owner-facing moderation placeholder can link straight into
/// `submitCounterNotice` (FR-005) without a separate staff-only lookup.
/// Mirrors `effective_status`'s "is this entity disabled right now" logic
/// but returns the case identity instead of just a boolean/status string.
pub async fn active_case_id(
    state: &AppState,
    entity_type: &str,
    entity_id: Uuid,
) -> GraphQLResult<Option<Uuid>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let entity_type_owned = entity_type.to_string();
    let latest =
        tokio::task::spawn_blocking(move || latest_event(&mut conn, &entity_type_owned, entity_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to load moderation status"))?;

    let Some(event) = latest else {
        return Ok(None);
    };

    if is_disabled_status(&event.action_type)
        || event.action_type == action_type::COUNTER_NOTICE_FORWARDED
    {
        Ok(Some(event.case_id))
    } else {
        Ok(None)
    }
}

/// Filters a list of rows down to only those NOT currently
/// moderation-disabled (contracts/graphql-moderation.md's "list queries
/// exclude the entity entirely" enforcement rule). `id_of` extracts each
/// row's id; volume is expected to be low (a world's own content, not a
/// platform-wide table — research.md R5), so a per-row `effective_status`
/// check (itself a single indexed lookup) is simple and correct rather
/// than a more complex batched query.
pub async fn filter_visible<T>(
    state: &AppState,
    entity_type: &str,
    rows: Vec<T>,
    id_of: impl Fn(&T) -> Uuid,
) -> GraphQLResult<Vec<T>> {
    let mut visible = Vec::with_capacity(rows.len());
    for row in rows {
        let id = id_of(&row);
        if effective_status(state, entity_type, id).await?.is_none() {
            visible.push(row);
        }
    }
    Ok(visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_app_state;
    use chrono::Duration;

    fn insert_event(
        conn: &mut PgConnection,
        case_id: Uuid,
        action: &str,
        entity_type: &str,
        entity_id: Uuid,
        world_id: Uuid,
        restoration_due_at: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        diesel::insert_into(content_moderation_actions::table)
            .values(NewContentModerationAction {
                case_id,
                action_type: action.to_string(),
                entity_type: entity_type.to_string(),
                entity_id,
                world_id,
                account_id: None,
                claimant_name: "Acme Corp".to_string(),
                claimant_contact: "legal@acme.example".to_string(),
                copyrighted_work_description: "Acme Sourcebook Vol. 1".to_string(),
                infringing_material_location: entity_id.to_string(),
                good_faith_statement: true,
                accuracy_statement: true,
                signature: "Jane Claimant".to_string(),
                validity_result: Some("valid".to_string()),
                missing_elements: None,
                counter_notice_id: None,
                restoration_due_at,
                created_by: None,
            })
            .execute(conn)
            .expect("failed to insert test moderation event");
    }

    #[tokio::test]
    async fn no_case_means_fully_visible() {
        let state = test_app_state();
        let status = effective_status(&state, "world_actor", Uuid::now_v7())
            .await
            .expect("query should not error");
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn disabled_entity_reports_content_disabled() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let entity_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let case_id = Uuid::now_v7();
        insert_event(
            &mut conn,
            case_id,
            action_type::NOTICE_RECEIVED,
            "world_actor",
            entity_id,
            world_id,
            None,
        );
        insert_event(
            &mut conn,
            case_id,
            action_type::CONTENT_DISABLED,
            "world_actor",
            entity_id,
            world_id,
            None,
        );
        drop(conn);

        let status = effective_status(&state, "world_actor", entity_id)
            .await
            .expect("query should not error");
        assert_eq!(status.as_deref(), Some(action_type::CONTENT_DISABLED));
    }

    #[tokio::test]
    async fn forwarded_counter_notice_past_due_auto_restores() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let entity_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let case_id = Uuid::now_v7();
        insert_event(
            &mut conn,
            case_id,
            action_type::CONTENT_DISABLED,
            "world_item",
            entity_id,
            world_id,
            None,
        );
        insert_event(
            &mut conn,
            case_id,
            action_type::COUNTER_NOTICE_FORWARDED,
            "world_item",
            entity_id,
            world_id,
            Some(Utc::now() - Duration::days(1)),
        );
        drop(conn);

        let status = effective_status(&state, "world_item", entity_id)
            .await
            .expect("query should not error");
        assert!(
            status.is_none(),
            "past-due forwarded case should auto-restore"
        );

        // The restoration should now be a real, durable event.
        let mut conn = state.db_pool.get().unwrap();
        let latest = latest_event(&mut conn, "world_item", entity_id)
            .expect("load should succeed")
            .expect("a latest event must exist");
        assert_eq!(latest.action_type, action_type::CONTENT_RESTORED);
    }

    #[tokio::test]
    async fn forwarded_counter_notice_not_yet_due_stays_disabled() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let entity_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let case_id = Uuid::now_v7();
        insert_event(
            &mut conn,
            case_id,
            action_type::CONTENT_DISABLED,
            "world_lore_entry",
            entity_id,
            world_id,
            None,
        );
        insert_event(
            &mut conn,
            case_id,
            action_type::COUNTER_NOTICE_FORWARDED,
            "world_lore_entry",
            entity_id,
            world_id,
            Some(Utc::now() + Duration::days(5)),
        );
        drop(conn);

        let status = effective_status(&state, "world_lore_entry", entity_id)
            .await
            .expect("query should not error");
        assert_eq!(status.as_deref(), Some(action_type::CONTENT_DISABLED));
    }

    #[tokio::test]
    async fn content_remains_disabled_blocks_auto_restoration() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let entity_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let case_id = Uuid::now_v7();
        insert_event(
            &mut conn,
            case_id,
            action_type::CONTENT_DISABLED,
            "world_actor",
            entity_id,
            world_id,
            None,
        );
        insert_event(
            &mut conn,
            case_id,
            action_type::COUNTER_NOTICE_FORWARDED,
            "world_actor",
            entity_id,
            world_id,
            Some(Utc::now() - Duration::days(1)),
        );
        insert_event(
            &mut conn,
            case_id,
            action_type::CONTENT_REMAINS_DISABLED,
            "world_actor",
            entity_id,
            world_id,
            None,
        );
        drop(conn);

        let status = effective_status(&state, "world_actor", entity_id)
            .await
            .expect("query should not error");
        assert_eq!(
            status.as_deref(),
            Some(action_type::CONTENT_REMAINS_DISABLED),
            "an explicit staff resolution after forwarding must block auto-restoration"
        );
    }
}
