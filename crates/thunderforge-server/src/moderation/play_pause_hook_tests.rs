//! Spec 051 T042: a takedown asks for a pause, and a restoration never
//! touches one.
//!
//! Every notice goes through `submit_takedown_notice_impl`, as `scene_tests`
//! does: the claim is that the programme raises the request, not that a
//! request can be written. Committed rather than rolled back for the same
//! reason, in worlds of their own.
//!
//! Each test holds `play_pause::test_lock` for its whole length, across
//! awaits, because the hook writes the tables `play_pause::migration_tests`
//! drops and recreates under exclusive locks. Holding a std mutex over an
//! await is harmless here: each `#[tokio::test]` has a runtime of its own and
//! nothing else on it waits for the lock.
#![allow(clippy::await_holding_lock)]

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::mutations_moderation::{
    SubmitCounterNoticeInput, SubmitTakedownNoticeInput, resolve_moderation_case_impl,
    submit_counter_notice_impl, submit_takedown_notice_impl,
};
use crate::graphql::types::{ModerationActionType, ModerationEntityType};
use crate::models::{ContentModerationAction, NewContentModerationAction};
use crate::moderation::reach::record_adoption_sync;
use crate::moderation::standing::{
    AppealDecision, Ladder, file_appeal_sync, open_termination_sync, resolve_appeal_sync,
};
use crate::moderation::{action_type, effective_status_sync};
use crate::play_pause::gate::refuse_if_paused;
use crate::play_pause::live_play::mark_live;
use crate::play_pause::models::{PauseRequest, PauseRequestState, PauseTrigger, PlayPause};
use crate::play_pause::requests::{FAIL_RAISES_FOR, ask_for_pauses, triggers_of_request};
use crate::play_pause::{TriggerDetail, pause_world, test_lock};
use crate::schema::{content_moderation_actions, world_play_pause_requests, world_play_pauses};
use crate::state::AppState;
use crate::test_support::{
    insert_test_actor, insert_test_item, insert_test_lore_entry, insert_test_scene,
    insert_test_scene_named, insert_test_user, insert_test_world, test_app_state,
};

fn notice(entity_type: ModerationEntityType, entity_id: Uuid) -> SubmitTakedownNoticeInput {
    SubmitTakedownNoticeInput {
        entity_type,
        entity_id,
        claimant_name: "Pause Hook Claimant".into(),
        claimant_contact: "claimant@example.test".into(),
        copyrighted_work_description: "A work the claimant owns".into(),
        infringing_material_location: "Content of a world being played".into(),
        good_faith_statement: true,
        accuracy_statement: true,
        signature: "Pause Hook Claimant".into(),
    }
}

fn pending_of(state: &AppState, world: Uuid) -> Vec<PauseRequest> {
    let mut conn = state.db_pool.get().unwrap();
    world_play_pause_requests::table
        .filter(world_play_pause_requests::world_id.eq(world))
        .filter(world_play_pause_requests::state.eq(PauseRequestState::Pending))
        .select(PauseRequest::as_select())
        .load(&mut conn)
        .unwrap()
}

fn triggers(state: &AppState, request: Uuid) -> Vec<PauseTrigger> {
    let mut conn = state.db_pool.get().unwrap();
    triggers_of_request(&mut conn, request).unwrap()
}

fn case_events(state: &AppState, case_id: Uuid) -> Vec<ContentModerationAction> {
    let mut conn = state.db_pool.get().unwrap();
    content_moderation_actions::table
        .filter(content_moderation_actions::case_id.eq(case_id))
        .select(ContentModerationAction::as_select())
        .load(&mut conn)
        .unwrap()
}

/// FR-030, FR-033: a takedown on a scene, an actor and a lore entry of a world
/// being played each raise, onto the one request for that world. A world
/// nobody is playing, beside it, raises nothing (US3 AS4).
#[tokio::test]
async fn takedowns_on_a_live_worlds_scene_actor_and_lore_each_raise() {
    let _lock = test_lock();
    let state = test_app_state();
    let (live, idle, scene, actor, lore, idle_scene) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let live = insert_test_world(&mut conn, owner);
        let idle = insert_test_world(&mut conn, owner);
        let scene = insert_test_scene(&mut conn, live, owner);
        let actor_scene = insert_test_scene_named(&mut conn, live, owner, "Actor Stage");
        let actor = insert_test_actor(&mut conn, live, actor_scene, owner);
        let lore = insert_test_lore_entry(&mut conn, live, owner);
        let idle_scene = insert_test_scene(&mut conn, idle, owner);
        assert!(mark_live(&mut conn, live).unwrap());
        (live, idle, scene, actor, lore, idle_scene)
    };

    let mut disabled_actions = Vec::new();
    for (entity_type, entity_id, db_type) in [
        (ModerationEntityType::Scene, scene, "scene"),
        (ModerationEntityType::WorldActor, actor, "world_actor"),
        (
            ModerationEntityType::WorldLoreEntry,
            lore,
            "world_lore_entry",
        ),
    ] {
        let case = submit_takedown_notice_impl(&state, notice(entity_type, entity_id))
            .await
            .expect("the takedown is filed");
        let disabled = case_events(&state, case.case_id)
            .into_iter()
            .find(|e| e.action_type == action_type::CONTENT_DISABLED)
            .expect("the content is disabled");
        disabled_actions.push((disabled.id, db_type, entity_id));
    }

    let [request] = pending_of(&state, live).try_into().unwrap();
    let recorded: Vec<_> = triggers(&state, request.id)
        .into_iter()
        .map(|t| {
            (
                t.moderation_action_id.unwrap(),
                t.entity_type.unwrap(),
                t.entity_id.unwrap(),
            )
        })
        .collect();
    let expected: Vec<_> = disabled_actions
        .into_iter()
        .map(|(action, db_type, id)| (action, db_type.to_string(), id))
        .collect();
    assert_eq!(recorded, expected, "one request, carrying every takedown");

    submit_takedown_notice_impl(&state, notice(ModerationEntityType::Scene, idle_scene))
        .await
        .expect("the takedown is filed");
    assert!(
        pending_of(&state, idle).is_empty(),
        "nobody is playing it, so nobody is asked"
    );
}

/// R3: a copy disabled by `fan_out_disable` in a world being played asks for
/// that world, though the source's world is idle.
#[tokio::test]
async fn a_takedown_reaching_a_copy_in_a_live_world_raises_for_that_world() {
    let _lock = test_lock();
    let state = test_app_state();
    let (source_world, adopter_world, source, copy) = {
        let mut conn = state.db_pool.get().unwrap();
        let sharer = insert_test_user(&mut conn);
        let adopter = insert_test_user(&mut conn);
        let source_world = insert_test_world(&mut conn, sharer);
        let adopter_world = insert_test_world(&mut conn, adopter);
        let source = insert_test_item(&mut conn, source_world, sharer);
        let copy = insert_test_item(&mut conn, adopter_world, adopter);
        record_adoption_sync(
            &mut conn,
            "world_item",
            source,
            copy,
            adopter_world,
            adopter,
        )
        .unwrap();
        assert!(mark_live(&mut conn, adopter_world).unwrap());
        (source_world, adopter_world, source, copy)
    };

    submit_takedown_notice_impl(&state, notice(ModerationEntityType::WorldItem, source))
        .await
        .expect("the takedown is filed");

    assert!(
        pending_of(&state, source_world).is_empty(),
        "the source's world is idle"
    );
    let [request] = pending_of(&state, adopter_world).try_into().unwrap();
    let [trigger] = triggers(&state, request.id).try_into().unwrap();
    assert_eq!(trigger.entity_id, Some(copy));

    let mut conn = state.db_pool.get().unwrap();
    let copy_action: (Uuid, String) = content_moderation_actions::table
        .filter(content_moderation_actions::entity_id.eq(copy))
        .select((
            content_moderation_actions::id,
            content_moderation_actions::action_type,
        ))
        .first(&mut conn)
        .unwrap();
    assert_eq!(trigger.moderation_action_id, Some(copy_action.0));
    assert_eq!(copy_action.1, action_type::CONTENT_DISABLED_AS_COPY);
}

/// R4: a hook that cannot write its request leaves the takedown in effect,
/// reports success to the claimant, and logs the moderation action id.
#[tokio::test]
async fn a_failing_hook_leaves_the_takedown_in_effect_and_logs_the_action() {
    let _lock = test_lock();
    let state = test_app_state();
    let (world, scene) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let scene = insert_test_scene(&mut conn, world, owner);
        assert!(mark_live(&mut conn, world).unwrap());
        (world, scene)
    };
    FAIL_RAISES_FOR.lock().unwrap().insert(world);

    let filed =
        submit_takedown_notice_impl(&state, notice(ModerationEntityType::Scene, scene)).await;

    let case = filed.expect("the takedown does not fail because the hook did");
    assert_eq!(case.current_status, ModerationActionType::ContentDisabled);
    {
        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            effective_status_sync(&mut conn, "scene", scene)
                .unwrap()
                .as_deref(),
            Some(action_type::CONTENT_DISABLED),
            "the scene is taken down"
        );
    }
    assert!(
        pending_of(&state, world).is_empty(),
        "no request was written"
    );

    let events = case_events(&state, case.case_id);
    let disabled = events
        .iter()
        .find(|e| e.action_type == action_type::CONTENT_DISABLED)
        .unwrap()
        .id;
    let logged = ask_for_pauses(&state, &events, &[]).await;
    FAIL_RAISES_FOR.lock().unwrap().remove(&world);

    let [line] = logged.as_slice() else {
        panic!("one failure logged: {logged:?}");
    };
    assert!(
        line.contains(&disabled.to_string()),
        "the log names the moderation action, so it can be raised by hand: {line}"
    );
}

// ----- FR-042: restoring content never lifts a pause -----------------------

/// A world, paused, with a scene taken down in it. Returns the owner, the
/// world, the scene and its case.
async fn a_paused_world_with_a_takedown(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
    let (owner, world, scene) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let scene = insert_test_scene(&mut conn, world, owner);
        pause_world(
            &mut conn,
            owner,
            world,
            "Paused.",
            TriggerDetail::operator(),
        )
        .unwrap();
        (owner, world, scene)
    };
    let case = submit_takedown_notice_impl(state, notice(ModerationEntityType::Scene, scene))
        .await
        .expect("a takedown in a paused world");
    (owner, world, scene, case.case_id)
}

fn assert_still_paused(state: &AppState, world: Uuid) {
    let mut conn = state.db_pool.get().unwrap();
    let pauses: Vec<PlayPause> = world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world))
        .select(PlayPause::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(pauses.len(), 1);
    assert!(pauses[0].is_active(), "restoring content lifted the pause");
    assert!(
        refuse_if_paused(&mut conn, world).is_err(),
        "the gate still refuses"
    );
}

#[tokio::test]
async fn a_case_resolved_as_restored_leaves_the_pause_active() {
    let _lock = test_lock();
    let state = test_app_state();
    let (_, world, scene, case_id) = a_paused_world_with_a_takedown(&state).await;

    resolve_moderation_case_impl(&state, true, case_id, ModerationActionType::ContentRestored)
        .await
        .expect("restored");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        effective_status_sync(&mut conn, "scene", scene).unwrap(),
        None
    );
    drop(conn);
    assert_still_paused(&state, world);
}

#[tokio::test]
async fn a_counter_notice_elapsing_leaves_the_pause_active() {
    let _lock = test_lock();
    let state = test_app_state();
    let (owner, world, scene, case_id) = a_paused_world_with_a_takedown(&state).await;

    submit_counter_notice_impl(
        &state,
        owner,
        false,
        SubmitCounterNoticeInput {
            case_id,
            removed_material_description: "My own map".into(),
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "owner@example.test".into(),
            signature: "The Owner".into(),
        },
    )
    .await
    .expect("forwarded");

    let mut conn = state.db_pool.get().unwrap();
    diesel::update(
        content_moderation_actions::table
            .filter(content_moderation_actions::case_id.eq(case_id))
            .filter(
                content_moderation_actions::action_type.eq(action_type::COUNTER_NOTICE_FORWARDED),
            ),
    )
    .set(
        content_moderation_actions::restoration_due_at
            .eq(Some(Utc::now() - chrono::Duration::days(1))),
    )
    .execute(&mut conn)
    .unwrap();

    assert_eq!(
        effective_status_sync(&mut conn, "scene", scene).unwrap(),
        None,
        "the lazy restoration ran"
    );
    drop(conn);
    assert_still_paused(&state, world);
}

#[tokio::test]
async fn an_upheld_appeal_restoring_a_case_leaves_the_pause_active() {
    let _lock = test_lock();
    let state = test_app_state();
    let ladder = Ladder {
        warn_at: 1,
        suspend_publishing_at: 2,
        threshold: 3,
        lookback_days: 365,
        window_days: 30,
        requires_human: true,
    };

    let mut conn = state.db_pool.get().unwrap();
    let account = insert_test_user(&mut conn);
    let admin = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, account);
    pause_world(
        &mut conn,
        admin,
        world,
        "Paused.",
        TriggerDetail::operator(),
    )
    .unwrap();

    let mut cases = Vec::new();
    for _ in 0..3 {
        let case_id = Uuid::now_v7();
        let item = insert_test_item(&mut conn, world, account);
        diesel::insert_into(content_moderation_actions::table)
            .values(NewContentModerationAction {
                case_id,
                action_type: action_type::CONTENT_DISABLED.to_string(),
                entity_type: "world_item".to_string(),
                entity_id: item,
                world_id: world,
                account_id: Some(account),
                claimant_name: "Claimant".to_string(),
                claimant_contact: "claimant@example.test".to_string(),
                copyrighted_work_description: "A work.".to_string(),
                infringing_material_location: "An item.".to_string(),
                good_faith_statement: true,
                accuracy_statement: true,
                signature: "Claimant".to_string(),
                validity_result: Some("valid".to_string()),
                missing_elements: None,
                counter_notice_id: None,
                restoration_due_at: None,
                created_by: None,
            })
            .execute(&mut conn)
            .unwrap();
        cases.push((case_id, item));
    }
    open_termination_sync(&mut conn, account, ladder, Utc::now())
        .unwrap()
        .expect("three strikes open a window");
    file_appeal_sync(&mut conn, account, "Not mine.", Utc::now()).unwrap();
    resolve_appeal_sync(
        &mut conn,
        account,
        admin,
        &AppealDecision {
            upheld: true,
            note: None,
            overturned_case: Some(cases[0].0),
        },
        ladder,
        Utc::now(),
    )
    .expect("upheld");

    assert_eq!(
        effective_status_sync(&mut conn, "world_item", cases[0].1).unwrap(),
        None,
        "restore_case_sync ran"
    );
    drop(conn);
    assert_still_paused(&state, world);
}
