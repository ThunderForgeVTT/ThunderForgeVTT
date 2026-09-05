//! Spec 015: DMCA takedown notice / counter-notice / staff-resolution
//! mutations. See contracts/graphql-moderation.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::{
    GraphQLModerationAction, GraphQLModerationCase, ModerationActionType, ModerationEntityType,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ContentModerationAction, NewContentModerationAction};
use crate::moderation::validation::{
    CounterNoticeFields, TakedownNoticeFields, validate_counter_notice, validate_takedown_notice,
};
use crate::moderation::{action_type, counter_notice_waiting_period_days};
use crate::schema::content_moderation_actions;
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct SubmitTakedownNoticeInput {
    pub entity_type: ModerationEntityType,
    pub entity_id: Uuid,
    pub claimant_name: String,
    pub claimant_contact: String,
    pub copyrighted_work_description: String,
    pub infringing_material_location: String,
    pub good_faith_statement: bool,
    pub accuracy_statement: bool,
    pub signature: String,
}

#[derive(InputObject, Debug, Clone)]
pub struct SubmitCounterNoticeInput {
    pub case_id: Uuid,
    pub removed_material_description: String,
    pub good_faith_mistake_statement: bool,
    pub consent_to_jurisdiction: bool,
    pub contact_information: String,
    pub signature: String,
}

/// Resolves `(entity_type, entity_id)` to its owning `(world_id,
/// account_id)` by querying the matching content table directly —
/// denormalized onto every `content_moderation_actions` row at write
/// time (data-model.md), since the moderation table itself carries no
/// FK to any content table.
fn resolve_entity_owner(
    conn: &mut PgConnection,
    entity_type: ModerationEntityType,
    entity_id: Uuid,
) -> Result<(Uuid, Option<Uuid>), String> {
    use crate::schema::{world_abilities, world_actors, world_items, world_lore_entries};

    match entity_type {
        ModerationEntityType::WorldActor => world_actors::table
            .filter(world_actors::id.eq(entity_id))
            .select((world_actors::world_id, world_actors::created_by))
            .first::<(Uuid, Uuid)>(conn)
            .map(|(w, a)| (w, Some(a)))
            .map_err(|_| "Actor not found".to_string()),
        ModerationEntityType::WorldItem => world_items::table
            .filter(world_items::id.eq(entity_id))
            .select((world_items::world_id, world_items::created_by))
            .first::<(Uuid, Uuid)>(conn)
            .map(|(w, a)| (w, Some(a)))
            .map_err(|_| "Item not found".to_string()),
        ModerationEntityType::WorldLoreEntry => world_lore_entries::table
            .filter(world_lore_entries::id.eq(entity_id))
            .select((world_lore_entries::world_id, world_lore_entries::created_by))
            .first::<(Uuid, Uuid)>(conn)
            .map(|(w, a)| (w, Some(a)))
            .map_err(|_| "Lore entry not found".to_string()),
        // Spec 025 (T010): abilities are moderatable at individual-entry
        // granularity, per spec 015 FR-010.
        ModerationEntityType::WorldAbility => world_abilities::table
            .filter(world_abilities::id.eq(entity_id))
            .select((world_abilities::world_id, world_abilities::created_by))
            .first::<(Uuid, Uuid)>(conn)
            .map(|(w, a)| (w, Some(a)))
            .map_err(|_| "Ability not found".to_string()),
    }
}

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

fn to_graphql_case(events: Vec<ContentModerationAction>) -> GraphQLResult<GraphQLModerationCase> {
    let last = events
        .last()
        .ok_or_else(|| Error::new("Case has no events"))?;
    let case_id = last.case_id;
    let entity_type = ModerationEntityType::from_db_str(&last.entity_type)
        .ok_or_else(|| Error::new("Unknown entity type"))?;
    let entity_id = last.entity_id;
    let world_id = last.world_id;
    let current_status = ModerationActionType::from_db_str(&last.action_type)
        .ok_or_else(|| Error::new("Unknown action type"))?;

    Ok(GraphQLModerationCase {
        case_id,
        entity_type,
        entity_id,
        world_id,
        current_status,
        events: events
            .into_iter()
            .map(GraphQLModerationAction::from)
            .collect(),
    })
}

/// Testable core of `ModerationMutation::submit_takedown_notice`.
/// Public — no auth required (contracts/graphql-moderation.md). A
/// statutorily-incomplete notice is recorded as
/// `notice_rejected_incomplete` (never silently dropped, FR-003) rather
/// than returning a GraphQL error.
pub async fn submit_takedown_notice_impl(
    state: &AppState,
    input: SubmitTakedownNoticeInput,
) -> GraphQLResult<GraphQLModerationCase> {
    let missing = validate_takedown_notice(&TakedownNoticeFields {
        claimant_name: &input.claimant_name,
        claimant_contact: &input.claimant_contact,
        copyrighted_work_description: &input.copyrighted_work_description,
        infringing_material_location: &input.infringing_material_location,
        good_faith_statement: input.good_faith_statement,
        accuracy_statement: input.accuracy_statement,
        signature: &input.signature,
    });

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let entity_type = input.entity_type;
    let entity_id = input.entity_id;
    let case_id = Uuid::now_v7();

    let events =
        tokio::task::spawn_blocking(move || -> Result<Vec<ContentModerationAction>, String> {
            let (world_id, account_id) = resolve_entity_owner(&mut conn, entity_type, entity_id)?;

            if !missing.is_empty() {
                diesel::insert_into(content_moderation_actions::table)
                    .values(NewContentModerationAction {
                        case_id,
                        action_type: action_type::NOTICE_REJECTED_INCOMPLETE.to_string(),
                        entity_type: entity_type.as_db_str().to_string(),
                        entity_id,
                        world_id,
                        account_id,
                        claimant_name: input.claimant_name.clone(),
                        claimant_contact: input.claimant_contact.clone(),
                        copyrighted_work_description: input.copyrighted_work_description.clone(),
                        infringing_material_location: input.infringing_material_location.clone(),
                        good_faith_statement: input.good_faith_statement,
                        accuracy_statement: input.accuracy_statement,
                        signature: input.signature.clone(),
                        validity_result: Some("invalid_missing_elements".to_string()),
                        missing_elements: Some(missing.clone()),
                        counter_notice_id: None,
                        restoration_due_at: None,
                        created_by: None,
                    })
                    .execute(&mut conn)
                    .map_err(|e| e.to_string())?;
                return load_case_events(&mut conn, case_id).map_err(|e| e.to_string());
            }

            diesel::insert_into(content_moderation_actions::table)
                .values(NewContentModerationAction {
                    case_id,
                    action_type: action_type::NOTICE_RECEIVED.to_string(),
                    entity_type: entity_type.as_db_str().to_string(),
                    entity_id,
                    world_id,
                    account_id,
                    claimant_name: input.claimant_name.clone(),
                    claimant_contact: input.claimant_contact.clone(),
                    copyrighted_work_description: input.copyrighted_work_description.clone(),
                    infringing_material_location: input.infringing_material_location.clone(),
                    good_faith_statement: input.good_faith_statement,
                    accuracy_statement: input.accuracy_statement,
                    signature: input.signature.clone(),
                    validity_result: Some("valid".to_string()),
                    missing_elements: None,
                    counter_notice_id: None,
                    restoration_due_at: None,
                    created_by: None,
                })
                .execute(&mut conn)
                .map_err(|e| e.to_string())?;

            diesel::insert_into(content_moderation_actions::table)
                .values(NewContentModerationAction {
                    case_id,
                    action_type: action_type::CONTENT_DISABLED.to_string(),
                    entity_type: entity_type.as_db_str().to_string(),
                    entity_id,
                    world_id,
                    account_id,
                    claimant_name: input.claimant_name.clone(),
                    claimant_contact: input.claimant_contact.clone(),
                    copyrighted_work_description: input.copyrighted_work_description.clone(),
                    infringing_material_location: input.infringing_material_location.clone(),
                    good_faith_statement: input.good_faith_statement,
                    accuracy_statement: input.accuracy_statement,
                    signature: input.signature.clone(),
                    validity_result: Some("valid".to_string()),
                    missing_elements: None,
                    counter_notice_id: None,
                    restoration_due_at: None,
                    created_by: None,
                })
                .execute(&mut conn)
                .map_err(|e| e.to_string())?;

            load_case_events(&mut conn, case_id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    // Spec 034 FR-040: the content is disabled and that is done. What follows
    // is about a mirror the platform does not control, and **it cannot fail
    // this call** (FR-040d) — the takedown has already happened, and a
    // repository being unreachable is not a reason to un-happen it.
    //
    // Deliberately after the transaction, and deliberately not a `?`. The hook
    // returns a report rather than a Result precisely so there is nothing here
    // to propagate; the worst it can do is say it could not.
    if entity_type == ModerationEntityType::WorldLoreEntry
        && let Some(disabled) = events
            .iter()
            .find(|e| e.action_type == action_type::CONTENT_DISABLED)
    {
        // The disabling action is what the notice is recorded against. A fresh
        // id here would orphan the record from the takedown that caused it, and
        // "which takedown was this for" is the first question anyone asks of
        // the table a year later.
        let response = crate::lore_sync::takedown_hook::on_content_disabled(
            state,
            disabled.world_id,
            disabled.id,
            chrono::Utc::now().date_naive(),
        )
        .await;

        if let crate::lore_sync::takedown_hook::MirrorResponse::Attempted(outcome) = &response {
            eprintln!("[LoreSync] takedown mirror response: {outcome:?}");
        }
    }

    to_graphql_case(events)
}

/// Testable core of `ModerationMutation::submit_counter_notice`. Requires
/// the caller to be the owning GM/account for the case's world (reuses
/// `is_dm_of_world`, matching every other GM-scoped mutation in this
/// codebase).
pub async fn submit_counter_notice_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: SubmitCounterNoticeInput,
) -> GraphQLResult<GraphQLModerationCase> {
    let missing = validate_counter_notice(&CounterNoticeFields {
        removed_material_description: &input.removed_material_description,
        good_faith_mistake_statement: input.good_faith_mistake_statement,
        consent_to_jurisdiction: input.consent_to_jurisdiction,
        contact_information: &input.contact_information,
        signature: &input.signature,
    });
    if !missing.is_empty() {
        return Err(Error::new(format!(
            "Counter-notice missing required elements: {}",
            missing.join(", ")
        )));
    }

    let case_id = input.case_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (world_id, entity_type, entity_id) =
        tokio::task::spawn_blocking(move || -> Result<(Uuid, String, Uuid), String> {
            let last = content_moderation_actions::table
                .filter(content_moderation_actions::case_id.eq(case_id))
                .order(content_moderation_actions::created_at.desc())
                .select(ContentModerationAction::as_select())
                .first::<ContentModerationAction>(&mut conn)
                .map_err(|_| "Case not found".to_string())?;
            Ok((last.world_id, last.entity_type, last.entity_id))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the content's owning GM may submit a counter-notice for this case",
        ));
    }

    let waiting_days = counter_notice_waiting_period_days();
    let restoration_due_at = chrono::Utc::now() + chrono::Duration::days(waiting_days);

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let events =
        tokio::task::spawn_blocking(move || -> Result<Vec<ContentModerationAction>, String> {
            diesel::insert_into(content_moderation_actions::table)
                .values(NewContentModerationAction {
                    case_id,
                    action_type: action_type::COUNTER_NOTICE_RECEIVED.to_string(),
                    entity_type: entity_type.clone(),
                    entity_id,
                    world_id,
                    account_id: Some(user_id),
                    claimant_name: String::new(),
                    claimant_contact: String::new(),
                    copyrighted_work_description: String::new(),
                    infringing_material_location: input.removed_material_description.clone(),
                    good_faith_statement: input.good_faith_mistake_statement,
                    accuracy_statement: input.consent_to_jurisdiction,
                    signature: input.signature.clone(),
                    validity_result: Some("valid".to_string()),
                    missing_elements: None,
                    counter_notice_id: Some(case_id),
                    restoration_due_at: None,
                    created_by: Some(user_id),
                })
                .execute(&mut conn)
                .map_err(|e| e.to_string())?;

            diesel::insert_into(content_moderation_actions::table)
                .values(NewContentModerationAction {
                    case_id,
                    action_type: action_type::COUNTER_NOTICE_FORWARDED.to_string(),
                    entity_type,
                    entity_id,
                    world_id,
                    account_id: Some(user_id),
                    claimant_name: String::new(),
                    claimant_contact: input.contact_information.clone(),
                    copyrighted_work_description: String::new(),
                    infringing_material_location: String::new(),
                    good_faith_statement: true,
                    accuracy_statement: true,
                    signature: String::new(),
                    validity_result: None,
                    missing_elements: None,
                    counter_notice_id: Some(case_id),
                    restoration_due_at: Some(restoration_due_at),
                    created_by: Some(user_id),
                })
                .execute(&mut conn)
                .map_err(|e| e.to_string())?;

            load_case_events(&mut conn, case_id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    to_graphql_case(events)
}

/// Testable core of `ModerationMutation::resolve_moderation_case`.
/// Compliance-staff-only (`is_admin`) — manually resolves a case outside
/// the automatic restoration-timer path (e.g. claimant filed further
/// legal action before the waiting period elapsed).
pub async fn resolve_moderation_case_impl(
    state: &AppState,
    is_admin: bool,
    case_id: Uuid,
    resolution: ModerationActionType,
) -> GraphQLResult<GraphQLModerationCase> {
    if !is_admin {
        return Err(Error::new(
            "Only compliance staff may resolve a moderation case",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let events =
        tokio::task::spawn_blocking(move || -> Result<Vec<ContentModerationAction>, String> {
            let last = content_moderation_actions::table
                .filter(content_moderation_actions::case_id.eq(case_id))
                .order(content_moderation_actions::created_at.desc())
                .select(ContentModerationAction::as_select())
                .first::<ContentModerationAction>(&mut conn)
                .map_err(|_| "Case not found".to_string())?;

            diesel::insert_into(content_moderation_actions::table)
                .values(NewContentModerationAction {
                    case_id,
                    action_type: resolution.as_db_str().to_string(),
                    entity_type: last.entity_type,
                    entity_id: last.entity_id,
                    world_id: last.world_id,
                    account_id: last.account_id,
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
                .map_err(|e| e.to_string())?;

            load_case_events(&mut conn, case_id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    to_graphql_case(events)
}

#[derive(Default)]
pub struct ModerationMutation;

#[async_graphql::Object]
impl ModerationMutation {
    async fn submit_takedown_notice(
        &self,
        ctx: &Context<'_>,
        input: SubmitTakedownNoticeInput,
    ) -> GraphQLResult<GraphQLModerationCase> {
        let state = app_state(ctx)?;
        submit_takedown_notice_impl(state, input).await
    }

    async fn submit_counter_notice(
        &self,
        ctx: &Context<'_>,
        input: SubmitCounterNoticeInput,
    ) -> GraphQLResult<GraphQLModerationCase> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        submit_counter_notice_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn resolve_moderation_case(
        &self,
        ctx: &Context<'_>,
        case_id: Uuid,
        resolution: ModerationActionType,
    ) -> GraphQLResult<GraphQLModerationCase> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        resolve_moderation_case_impl(state, auth_user.is_admin, case_id, resolution).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    fn valid_notice_input(
        entity_type: ModerationEntityType,
        entity_id: Uuid,
    ) -> SubmitTakedownNoticeInput {
        SubmitTakedownNoticeInput {
            entity_type,
            entity_id,
            claimant_name: "Acme Corp".to_string(),
            claimant_contact: "legal@acme.example".to_string(),
            copyrighted_work_description: "Acme Sourcebook Vol. 1".to_string(),
            infringing_material_location: entity_id.to_string(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Jane Claimant".to_string(),
        }
    }

    /// **FR-040d, and the one that matters most in this file.**
    ///
    /// A takedown is a legal obligation with a committed response window. The
    /// mirror hook that runs after it talks to a repository the platform does
    /// not control — a host that is down, a grant that was revoked, a
    /// repository that was deleted. None of that may reverse or block the
    /// disabling, and the structural guarantee is that the hook returns a
    /// report rather than a Result. This is the test that the guarantee holds
    /// in the path that actually runs.
    ///
    /// The connection here points at a repository that cannot resolve, so the
    /// hook genuinely fails rather than being skipped.
    #[tokio::test]
    async fn a_takedown_succeeds_even_when_the_worlds_mirror_cannot_be_reached() {
        use crate::models::LoreRepositoryConnection;
        use crate::schema::lore_repository_connections as c;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = crate::test_support::insert_test_lore_entry(&mut conn, world_id, owner_id);

        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(c::table)
            .values(LoreRepositoryConnection {
                id: Uuid::now_v7(),
                world_id,
                host_kind: "test".into(),
                // Not a number, so the withdrawal fails while parsing the
                // installation reference — before any HTTP. The test then
                // needs no network and fails the same way on every machine,
                // where an unreachable *host* would make it depend on whether
                // this one is online.
                installation_ref: "not-an-installation".into(),
                // Unique per run: FR-033's constraint is instance-wide, so a
                // fixed name here makes the second run of this test fail on
                // the previous run's row rather than on anything it asserts.
                repository_ref: format!("no-such-owner/no-such-repository-{}", Uuid::now_v7()),
                branch: "main".into(),
                directory: "lore".into(),
                incoming_enabled: false,
                notice_acknowledged_at: Some(now),
                state: "working".into(),
                state_reason: None,
                // Public, so the hook tries to lodge and fails against a
                // repository that does not exist — the failure is real rather
                // than short-circuited by the private skip.
                repository_is_public: Some(true),
                visibility_checked_at: Some(now),
                deactivated_at: None,
                deactivated_reason: None,
                last_synced_at: None,
                last_written_commit: None,
                created_by: owner_id,
                updated_by: owner_id,
                created_at: now,
                updated_at: now,
            })
            .execute(&mut conn)
            .unwrap();

        let case = submit_takedown_notice_impl(
            &state,
            valid_notice_input(ModerationEntityType::WorldLoreEntry, entry_id),
        )
        .await
        .expect("a takedown must not fail because a repository is unreachable");

        assert!(
            case.events
                .iter()
                .any(|e| matches!(e.action_type, ModerationActionType::ContentDisabled)),
            "the content was not disabled",
        );

        // And the obligation was recorded as unmet rather than silently
        // dropped — FR-040d requires an administrator to be able to find it.
        let unmet = crate::lore_sync::disassociate::failed_notices(&mut conn)
            .expect("the failed notices are queryable");
        assert!(
            !unmet.is_empty(),
            "a failed withdrawal left no record for an administrator",
        );
    }

    #[tokio::test]
    async fn valid_notice_disables_the_target_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        use crate::schema::world_actors;
        let actor_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(actor_id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("npc"),
                world_actors::game_system_id.eq("dnd5e"),
                world_actors::label.eq("Infringing NPC"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(true),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let case = submit_takedown_notice_impl(
            &state,
            valid_notice_input(ModerationEntityType::WorldActor, actor_id),
        )
        .await
        .expect("valid notice should succeed");

        assert_eq!(case.current_status, ModerationActionType::ContentDisabled);

        let status = crate::moderation::effective_status(&state, "world_actor", actor_id)
            .await
            .expect("status query should not error");
        assert_eq!(
            status.as_deref(),
            Some(crate::moderation::action_type::CONTENT_DISABLED)
        );
    }

    #[tokio::test]
    async fn incomplete_notice_is_rejected_without_disabling_content() {
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
                name: "Totally Original Sword".to_string(),
                description: None,
            },
        )
        .await
        .expect("item creation should succeed")
        .id;

        let mut input = valid_notice_input(ModerationEntityType::WorldItem, item_id);
        input.accuracy_statement = false;

        let case = submit_takedown_notice_impl(&state, input)
            .await
            .expect("incomplete notice should still be logged, not error");
        assert_eq!(
            case.current_status,
            ModerationActionType::NoticeRejectedIncomplete
        );

        let status = crate::moderation::effective_status(&state, "world_item", item_id)
            .await
            .expect("status query should not error");
        assert!(
            status.is_none(),
            "an incomplete notice must not disable the entity"
        );
    }

    fn valid_counter_notice_input(case_id: Uuid) -> SubmitCounterNoticeInput {
        SubmitCounterNoticeInput {
            case_id,
            removed_material_description: "My homebrew NPC, entirely SRD-derived".to_string(),
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "gm@example.com".to_string(),
            signature: "GM Name".to_string(),
        }
    }

    /// US2: only the content's owning GM may submit a counter-notice, and
    /// doing so forwards the case without yet restoring it.
    #[tokio::test]
    async fn counter_notice_requires_owner_and_forwards_without_restoring() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let item = crate::graphql::mutations_items::create_item_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_items::CreateItemInput {
                world_id,
                name: "Disputed Item".to_string(),
                description: None,
            },
        )
        .await
        .unwrap();

        let case = submit_takedown_notice_impl(
            &state,
            valid_notice_input(ModerationEntityType::WorldItem, item.id),
        )
        .await
        .unwrap();

        let denied = submit_counter_notice_impl(
            &state,
            outsider_id,
            false,
            valid_counter_notice_input(case.case_id),
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-owner must not be able to counter-notice"
        );

        let forwarded = submit_counter_notice_impl(
            &state,
            owner_id,
            false,
            valid_counter_notice_input(case.case_id),
        )
        .await
        .expect("owner should be able to counter-notice");
        assert_eq!(
            forwarded.current_status,
            ModerationActionType::CounterNoticeForwarded
        );

        // Not yet restored — waiting period hasn't elapsed.
        let status = crate::moderation::effective_status(&state, "world_item", item.id)
            .await
            .unwrap();
        assert_eq!(
            status.as_deref(),
            Some(crate::moderation::action_type::CONTENT_DISABLED)
        );
    }

    /// US2 Scenario 4: compliance staff can block restoration before the
    /// waiting period elapses.
    #[tokio::test]
    async fn staff_can_block_restoration_before_waiting_period_elapses() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = crate::graphql::mutations_items::create_item_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_items::CreateItemInput {
                world_id,
                name: "Disputed Item 2".to_string(),
                description: None,
            },
        )
        .await
        .unwrap();

        let case = submit_takedown_notice_impl(
            &state,
            valid_notice_input(ModerationEntityType::WorldItem, item.id),
        )
        .await
        .unwrap();

        submit_counter_notice_impl(
            &state,
            owner_id,
            false,
            valid_counter_notice_input(case.case_id),
        )
        .await
        .unwrap();

        let denied_for_player = resolve_moderation_case_impl(
            &state,
            false,
            case.case_id,
            ModerationActionType::ContentRemainsDisabled,
        )
        .await;
        assert!(
            denied_for_player.is_err(),
            "a non-admin must not resolve a case"
        );

        let resolved = resolve_moderation_case_impl(
            &state,
            true,
            case.case_id,
            ModerationActionType::ContentRemainsDisabled,
        )
        .await
        .expect("compliance staff should be able to resolve the case");
        assert_eq!(
            resolved.current_status,
            ModerationActionType::ContentRemainsDisabled
        );

        let status = crate::moderation::effective_status(&state, "world_item", item.id)
            .await
            .unwrap();
        assert_eq!(
            status.as_deref(),
            Some(crate::moderation::action_type::CONTENT_REMAINS_DISABLED)
        );
    }
}
