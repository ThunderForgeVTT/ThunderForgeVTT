//! Spec 015: DMCA moderation status/history queries. See
//! contracts/graphql-moderation.md.

use async_graphql::Context;
use uuid::Uuid;

use crate::graphql::types::{
    GraphQLModerationAction, GraphQLModerationCase, ModerationActionType, ModerationEntityType,
};
use crate::graphql::*;
use crate::models::ContentModerationAction;
use crate::moderation::{action_type, repeat_infringer_lookback_days, repeat_infringer_threshold};
use crate::schema::content_moderation_actions;
use crate::state::AppState;

fn load_case_events(
    conn: &mut PgConnection,
    case_id: Uuid,
) -> Result<Vec<ContentModerationAction>, diesel::result::Error> {
    content_moderation_actions::table
        .filter(content_moderation_actions::case_id.eq(case_id))
        .order(content_moderation_actions::created_at.asc())
        .select(ContentModerationAction::as_select())
        .load::<ContentModerationAction>(conn)
}

fn to_graphql_case(events: Vec<ContentModerationAction>) -> Option<GraphQLModerationCase> {
    let last = events.last()?;
    let entity_type = ModerationEntityType::from_db_str(&last.entity_type)?;
    let current_status = ModerationActionType::from_db_str(&last.action_type)?;
    Some(GraphQLModerationCase {
        case_id: last.case_id,
        entity_type,
        entity_id: last.entity_id,
        world_id: last.world_id,
        current_status,
        events: events
            .into_iter()
            .map(GraphQLModerationAction::from)
            .collect(),
    })
}

/// Testable core of `ModerationQuery::moderation_status` — a thin wrapper
/// over the shared enforcement primitive, exposed for direct client
/// queries even though every content resolver also calls this
/// internally (contracts/graphql-moderation.md).
pub async fn moderation_status_impl(
    state: &AppState,
    entity_type: ModerationEntityType,
    entity_id: Uuid,
) -> GraphQLResult<Option<ModerationActionType>> {
    let status =
        crate::moderation::effective_status(state, entity_type.as_db_str(), entity_id).await?;
    Ok(status.and_then(|s| ModerationActionType::from_db_str(&s)))
}

/// Testable core of `ModerationQuery::moderation_case`.
pub async fn moderation_case_impl(
    state: &AppState,
    case_id: Uuid,
) -> GraphQLResult<Option<GraphQLModerationCase>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let events = tokio::task::spawn_blocking(move || load_case_events(&mut conn, case_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load case"))?;

    Ok(to_graphql_case(events))
}

/// Testable core of `ModerationQuery::moderation_history_for_account`.
pub async fn moderation_history_for_account_impl(
    state: &AppState,
    account_id: Uuid,
) -> GraphQLResult<Vec<GraphQLModerationCase>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        content_moderation_actions::table
            .filter(content_moderation_actions::account_id.eq(account_id))
            .order(content_moderation_actions::created_at.asc())
            .select(ContentModerationAction::as_select())
            .load::<ContentModerationAction>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load account moderation history"))?;

    let mut by_case: std::collections::BTreeMap<Uuid, Vec<ContentModerationAction>> =
        std::collections::BTreeMap::new();
    for row in rows {
        by_case.entry(row.case_id).or_default().push(row);
    }

    Ok(by_case.into_values().filter_map(to_graphql_case).collect())
}

/// Testable core of `ModerationQuery::repeat_infringer_flags` (FR-009).
/// Counts, per account, distinct cases whose *latest* event is
/// `content_disabled`/`content_remains_disabled` within the configured
/// lookback window, and returns accounts at/over the configured
/// threshold.
pub async fn repeat_infringer_flags_impl(state: &AppState) -> GraphQLResult<Vec<Uuid>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let lookback_days = repeat_infringer_lookback_days();
    let threshold = repeat_infringer_threshold();

    let rows = tokio::task::spawn_blocking(move || {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(lookback_days);
        content_moderation_actions::table
            .filter(content_moderation_actions::created_at.ge(cutoff))
            .filter(content_moderation_actions::account_id.is_not_null())
            .order(content_moderation_actions::created_at.asc())
            .select(ContentModerationAction::as_select())
            .load::<ContentModerationAction>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load moderation history"))?;

    // Group by case, take each case's latest event, then count upheld
    // (disabled/remains-disabled) cases per account.
    let mut by_case: std::collections::BTreeMap<Uuid, ContentModerationAction> =
        std::collections::BTreeMap::new();
    for row in rows {
        by_case
            .entry(row.case_id)
            .and_modify(|existing| {
                if row.created_at > existing.created_at {
                    *existing = row.clone();
                }
            })
            .or_insert(row);
    }

    let mut counts: std::collections::BTreeMap<Uuid, i64> = std::collections::BTreeMap::new();
    for case in by_case.values() {
        let upheld = matches!(
            case.action_type.as_str(),
            v if v == action_type::CONTENT_DISABLED || v == action_type::CONTENT_REMAINS_DISABLED
        );
        if upheld && let Some(account_id) = case.account_id {
            *counts.entry(account_id).or_insert(0) += 1;
        }
    }

    Ok(counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(account_id, _)| account_id)
        .collect())
}

#[derive(Default)]
pub struct ModerationQuery;

#[async_graphql::Object]
impl ModerationQuery {
    async fn moderation_status(
        &self,
        ctx: &Context<'_>,
        entity_type: ModerationEntityType,
        entity_id: Uuid,
    ) -> GraphQLResult<Option<ModerationActionType>> {
        let state = app_state(ctx)?;
        moderation_status_impl(state, entity_type, entity_id).await
    }

    async fn moderation_case(
        &self,
        ctx: &Context<'_>,
        case_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLModerationCase>> {
        let state = app_state(ctx)?;
        admin_user(ctx)?;
        moderation_case_impl(state, case_id).await
    }

    async fn moderation_history_for_account(
        &self,
        ctx: &Context<'_>,
        account_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLModerationCase>> {
        let state = app_state(ctx)?;
        admin_user(ctx)?;
        moderation_history_for_account_impl(state, account_id).await
    }

    async fn repeat_infringer_flags(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<Uuid>> {
        let state = app_state(ctx)?;
        admin_user(ctx)?;
        repeat_infringer_flags_impl(state).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_moderation::{
        SubmitTakedownNoticeInput, submit_takedown_notice_impl,
    };
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    #[tokio::test]
    async fn moderation_status_reflects_a_disabled_entity() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item_id = crate::graphql::mutations_items::create_item_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_items::CreateItemInput {
                world_id,
                name: "Infringing Sword".to_string(),
                description: None,
            },
        )
        .await
        .unwrap()
        .id;

        submit_takedown_notice_impl(
            &state,
            SubmitTakedownNoticeInput {
                entity_type: ModerationEntityType::WorldItem,
                entity_id: item_id,
                claimant_name: "Acme".to_string(),
                claimant_contact: "legal@acme.example".to_string(),
                copyrighted_work_description: "Acme Sourcebook".to_string(),
                infringing_material_location: item_id.to_string(),
                good_faith_statement: true,
                accuracy_statement: true,
                signature: "Jane".to_string(),
            },
        )
        .await
        .unwrap();

        let status = moderation_status_impl(&state, ModerationEntityType::WorldItem, item_id)
            .await
            .unwrap();
        assert_eq!(status, Some(ModerationActionType::ContentDisabled));
    }

    #[tokio::test]
    async fn repeat_infringer_flags_includes_accounts_at_threshold() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        // Default threshold is 3 (moderation::repeat_infringer_threshold).
        for i in 0..3 {
            let item_id = crate::graphql::mutations_items::create_item_impl(
                &state,
                owner_id,
                false,
                crate::graphql::mutations_items::CreateItemInput {
                    world_id,
                    name: format!("Infringing Item {i}"),
                    description: None,
                },
            )
            .await
            .unwrap()
            .id;

            submit_takedown_notice_impl(
                &state,
                SubmitTakedownNoticeInput {
                    entity_type: ModerationEntityType::WorldItem,
                    entity_id: item_id,
                    claimant_name: "Acme".to_string(),
                    claimant_contact: "legal@acme.example".to_string(),
                    copyrighted_work_description: "Acme Sourcebook".to_string(),
                    infringing_material_location: item_id.to_string(),
                    good_faith_statement: true,
                    accuracy_statement: true,
                    signature: "Jane".to_string(),
                },
            )
            .await
            .unwrap();
        }

        let flags = repeat_infringer_flags_impl(&state).await.unwrap();
        assert!(
            flags.contains(&owner_id),
            "an account with 3 upheld takedowns should be flagged (default threshold)"
        );
    }
}
