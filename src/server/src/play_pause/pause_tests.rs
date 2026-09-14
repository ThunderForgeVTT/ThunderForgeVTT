//! Spec 051 T016: pausing and lifting, through the module.

use diesel::prelude::*;
use uuid::Uuid;

use super::models::{PauseTrigger, PauseTriggerKind, PlayPause};
use super::{PauseError, TriggerDetail, lift_pause, pause_world};
use crate::graphql::mutations_moderation::{
    SubmitTakedownNoticeInput, submit_takedown_notice_impl,
};
use crate::graphql::types::ModerationEntityType;
use crate::moderation::effective_status_sync;
use crate::schema::{
    content_moderation_actions, world_events, world_play_pause_triggers, world_play_pauses,
};
use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, test_app_state};
use crate::world_events::EVENT_CODE_WORLD_PLAY_PAUSED;

const GROUNDS: &str = "Abuse reported at this table.";

fn pauses_of(conn: &mut PgConnection, world: Uuid) -> Vec<PlayPause> {
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world))
        .select(PlayPause::as_select())
        .load(conn)
        .unwrap()
}

fn triggers_of(conn: &mut PgConnection, pause: Uuid) -> Vec<PauseTrigger> {
    world_play_pause_triggers::table
        .filter(world_play_pause_triggers::pause_id.eq(pause))
        .order(world_play_pause_triggers::id)
        .select(PauseTrigger::as_select())
        .load(conn)
        .unwrap()
}

fn pause_events(conn: &mut PgConnection, world: Uuid) -> Vec<Option<serde_json::Value>> {
    world_events::table
        .filter(world_events::world_id.eq(world))
        .filter(world_events::event_code.eq(EVENT_CODE_WORLD_PLAY_PAUSED))
        .select(world_events::token_event)
        .load(conn)
        .unwrap()
}

/// One pause, its `Operator` trigger, and event 28 carrying `pausedAt` only.
#[test]
fn pausing_writes_the_pause_its_trigger_and_the_event() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);

        let outcome = pause_world(
            conn,
            operator,
            world,
            &format!("  {GROUNDS}\n"),
            TriggerDetail::operator(),
        )
        .expect("paused");

        assert!(!outcome.already_paused);
        let pause = &outcome.pause;
        assert!(pause.is_active());
        assert_eq!(pause.world_id, world);
        assert!(
            pause.world_name.starts_with("Test World"),
            "the name is snapshotted"
        );
        assert_eq!(pause.paused_by, operator);
        assert!(pause.paused_by_name.starts_with("test_user_"));
        assert_eq!(pause.grounds, GROUNDS, "stored trimmed");
        assert_eq!((pause.created_by, pause.updated_by), (operator, operator));
        assert_eq!(pauses_of(conn, world).len(), 1);

        let triggers = triggers_of(conn, pause.id);
        let [trigger] = triggers.as_slice() else {
            panic!("one trigger expected: {triggers:?}");
        };
        assert_eq!(trigger.kind, PauseTriggerKind::Operator);
        assert_eq!(trigger.request_id, None);
        assert_eq!(
            trigger.note, None,
            "the grounds are the pause's, not repeated"
        );
        assert_eq!(trigger.created_by, Some(operator));

        let events = pause_events(conn, world);
        let [Some(payload)] = events.as_slice() else {
            panic!("one pause event with a payload expected: {events:?}");
        };
        assert_eq!(
            payload,
            &serde_json::json!({ "pausedAt": pause.paused_at.and_utc().to_rfc3339() }),
            "the event every member receives says when, and nothing else"
        );
        Ok(())
    });
}

#[test]
fn blank_grounds_and_missing_worlds_are_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);

        for blank in ["", "   ", "\n\t"] {
            assert!(matches!(
                pause_world(conn, operator, world, blank, TriggerDetail::operator()),
                Err(PauseError::GroundsRequired)
            ));
        }
        assert!(pauses_of(conn, world).is_empty(), "nothing was written");
        assert!(pause_events(conn, world).is_empty());

        assert!(matches!(
            pause_world(
                conn,
                operator,
                Uuid::now_v7(),
                GROUNDS,
                TriggerDetail::operator()
            ),
            Err(PauseError::WorldNotFound)
        ));

        let error: async_graphql::Error =
            pause_world(conn, operator, world, " ", TriggerDetail::operator())
                .unwrap_err()
                .into();
        let extensions = serde_json::to_value(error.extensions.unwrap()).unwrap();
        assert_eq!(extensions["code"], "GROUNDS_REQUIRED");
        Ok(())
    });
}

/// FR-036: a second operator pausing a paused world adds their grounds to the
/// pause that exists, and makes no second pause and no second event.
#[test]
fn pausing_a_paused_world_adds_a_trigger_not_a_pause() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let first = insert_test_user(conn);
        let second = insert_test_user(conn);
        let world = insert_test_world(conn, first);

        let original = pause_world(conn, first, world, GROUNDS, TriggerDetail::operator()).unwrap();
        let again = pause_world(
            conn,
            second,
            world,
            "Also seen from the reports queue.",
            TriggerDetail::operator(),
        )
        .unwrap();

        assert!(again.already_paused);
        assert_eq!(again.pause.id, original.pause.id);
        assert_eq!(again.pause.grounds, GROUNDS, "the first grounds stand");
        assert_eq!(pauses_of(conn, world).len(), 1);
        assert_eq!(pause_events(conn, world).len(), 1, "no second event");

        let triggers = triggers_of(conn, original.pause.id);
        assert_eq!(triggers.len(), 2);
        let added = &triggers[1];
        assert_eq!(added.kind, PauseTriggerKind::Operator);
        assert_eq!(
            added.note.as_deref(),
            Some("Also seen from the reports queue.")
        );
        assert_eq!(added.created_by, Some(second));
        Ok(())
    });
}

/// A lift sets its four columns once. The second operator to lift is told who
/// got there first, and when.
#[test]
fn a_lift_happens_once_and_a_second_lift_says_who_lifted() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let pauser = insert_test_user(conn);
        let lifter = insert_test_user(conn);
        let late = insert_test_user(conn);
        let world = insert_test_world(conn, pauser);
        let pause = pause_world(conn, pauser, world, GROUNDS, TriggerDetail::operator())
            .unwrap()
            .pause;

        assert!(matches!(
            lift_pause(conn, lifter, pause.id, "  "),
            Err(PauseError::GroundsRequired)
        ));

        let lifted = lift_pause(conn, lifter, pause.id, "Resolved with the table.").unwrap();
        assert!(!lifted.is_active());
        assert_eq!(lifted.lifted_by, Some(lifter));
        assert!(
            lifted
                .lifted_by_name
                .as_deref()
                .unwrap()
                .starts_with("test_user_")
        );
        assert_eq!(
            lifted.lift_grounds.as_deref(),
            Some("Resolved with the table.")
        );
        assert_eq!(lifted.updated_by, lifter);
        assert_eq!(
            (lifted.paused_by, lifted.grounds.as_str(), lifted.paused_at),
            (pauser, GROUNDS, pause.paused_at),
            "the pause itself is unchanged"
        );

        match lift_pause(conn, late, pause.id, "Me too.") {
            Err(PauseError::AlreadyLifted {
                lifted_by,
                lifted_by_name,
                lifted_at,
            }) => {
                assert_eq!(lifted_by, lifter);
                assert_eq!(Some(lifted_by_name), lifted.lifted_by_name);
                assert_eq!(Some(lifted_at), lifted.lifted_at);
            }
            other => panic!("a second lift must report the first: {other:?}"),
        }
        let reread = pauses_of(conn, world);
        assert_eq!(
            reread[0].lifted_by,
            Some(lifter),
            "the late lift changed nothing"
        );

        let error: async_graphql::Error = lift_pause(conn, late, pause.id, "Again.")
            .unwrap_err()
            .into();
        let extensions = serde_json::to_value(error.extensions.unwrap()).unwrap();
        assert_eq!(extensions["code"], "PAUSE_ALREADY_LIFTED");
        assert_eq!(extensions["liftedBy"]["id"], lifter.to_string());

        assert!(matches!(
            lift_pause(conn, lifter, Uuid::now_v7(), "Nothing to lift."),
            Err(PauseError::PauseNotFound)
        ));
        Ok(())
    });
}

/// FR-041, FR-042: lifting restores play, not content. A scene taken down
/// while the world was paused is still taken down after the lift, and the
/// moderation record gains nothing.
///
/// Committed rather than rolled back: the notice goes through
/// `submit_takedown_notice_impl`, which takes its own connections.
#[tokio::test]
async fn lifting_leaves_a_taken_down_scene_taken_down() {
    let state = test_app_state();
    let (operator, world, scene) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, operator);
        let scene = insert_test_scene(&mut conn, world, operator);
        (operator, world, scene)
    };

    let pause = {
        let _lock = super::test_lock();
        let mut conn = state.db_pool.get().unwrap();
        pause_world(
            &mut conn,
            operator,
            world,
            GROUNDS,
            TriggerDetail::operator(),
        )
        .unwrap()
        .pause
    };

    submit_takedown_notice_impl(
        &state,
        SubmitTakedownNoticeInput {
            entity_type: ModerationEntityType::Scene,
            entity_id: scene,
            claimant_name: "Pause Test Claimant".into(),
            claimant_contact: "claimant@example.test".into(),
            copyrighted_work_description: "A published battle map".into(),
            infringing_material_location: "A scene in a paused world".into(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Pause Test Claimant".into(),
        },
    )
    .await
    .expect("a notice can be filed against a scene in a paused world");

    let _lock = super::test_lock();
    let mut conn = state.db_pool.get().unwrap();
    let actions_for = |conn: &mut PgConnection| -> i64 {
        content_moderation_actions::table
            .filter(content_moderation_actions::entity_id.eq(scene))
            .count()
            .get_result(conn)
            .unwrap()
    };
    let status_before = effective_status_sync(&mut conn, "scene", scene).unwrap();
    let actions_before = actions_for(&mut conn);
    assert!(status_before.is_some(), "the scene is taken down");

    lift_pause(&mut conn, operator, pause.id, "Unrelated to the takedown.").unwrap();

    assert_eq!(
        effective_status_sync(&mut conn, "scene", scene).unwrap(),
        status_before
    );
    assert_eq!(
        actions_for(&mut conn),
        actions_before,
        "no moderation row was written"
    );
}
