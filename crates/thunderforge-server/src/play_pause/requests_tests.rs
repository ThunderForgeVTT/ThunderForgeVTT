//! Spec 051 T041: requests to pause, and the decision on one, through the
//! module.

use std::sync::{Arc, Barrier};

use diesel::prelude::*;
use uuid::Uuid;

use super::models::{PauseRequest, PauseRequestState, PauseTrigger, PauseTriggerKind, PlayPause};
use super::requests::{
    Decision, RaiseOutcome, TakedownReach, decide, raise, raise_for_takedown, triggers_of_request,
};
use super::{PauseError, TriggerDetail, pause_world};
use crate::graphql::queries::play_pause::world_play_state_sync;
use crate::schema::{
    world_events, world_live_play, world_play_pause_requests, world_play_pause_triggers,
    world_play_pauses,
};
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};
use crate::world_events::EVENT_CODE_WORLD_PLAY_PAUSED;

fn takedown(action: Uuid) -> TriggerDetail {
    TriggerDetail {
        kind: PauseTriggerKind::Takedown,
        moderation_action_id: Some(action),
        entity_type: Some("scene".into()),
        entity_id: Some(Uuid::now_v7()),
        note: None,
    }
}

fn reach(world: Uuid, action: Uuid) -> TakedownReach {
    TakedownReach {
        world_id: world,
        moderation_action_id: action,
        entity_type: "world_actor".into(),
        entity_id: Uuid::now_v7(),
    }
}

/// A beat just now, written as the heartbeat would.
fn played(conn: &mut PgConnection, world: Uuid) {
    diesel::insert_into(world_live_play::table)
        .values((
            world_live_play::world_id.eq(world),
            world_live_play::last_beat_at.eq(diesel::dsl::now),
        ))
        .execute(conn)
        .unwrap();
}

fn requests_of(conn: &mut PgConnection, world: Uuid) -> Vec<PauseRequest> {
    world_play_pause_requests::table
        .filter(world_play_pause_requests::world_id.eq(world))
        .select(PauseRequest::as_select())
        .load(conn)
        .unwrap()
}

fn pauses_of(conn: &mut PgConnection, world: Uuid) -> Vec<PlayPause> {
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world))
        .select(PlayPause::as_select())
        .load(conn)
        .unwrap()
}

fn pause_triggers(conn: &mut PgConnection, pause: Uuid) -> Vec<PauseTrigger> {
    world_play_pause_triggers::table
        .filter(world_play_pause_triggers::pause_id.eq(pause))
        .order(world_play_pause_triggers::id)
        .select(PauseTrigger::as_select())
        .load(conn)
        .unwrap()
}

fn pause_events(conn: &mut PgConnection, world: Uuid) -> i64 {
    world_events::table
        .filter(world_events::world_id.eq(world))
        .filter(world_events::event_code.eq(EVENT_CODE_WORLD_PLAY_PAUSED))
        .count()
        .get_result(conn)
        .unwrap()
}

fn requested(outcome: RaiseOutcome) -> Uuid {
    match outcome {
        RaiseOutcome::Requested { request_id } => request_id,
        other => panic!("a request was expected: {other:?}"),
    }
}

/// A raise makes one pending request, raised by nobody, carrying its trigger.
/// It pauses nothing (FR-032).
#[test]
fn a_raise_creates_one_pending_request_with_its_trigger() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let owner = insert_test_user(conn);
        let world = insert_test_world(conn, owner);
        let action = Uuid::now_v7();

        let request_id = requested(raise(conn, world, &takedown(action)).unwrap());

        let [request] = requests_of(conn, world).try_into().unwrap();
        assert_eq!(request.id, request_id);
        assert_eq!(request.state, PauseRequestState::Pending);
        assert!(request.world_name.starts_with("Test World"));
        assert_eq!((request.created_by, request.decided_by), (None, None));

        let [trigger] = triggers_of_request(conn, request_id)?.try_into().unwrap();
        assert_eq!(trigger.kind, PauseTriggerKind::Takedown);
        assert_eq!(trigger.moderation_action_id, Some(action));
        assert_eq!(trigger.pause_id, None);

        assert!(
            pauses_of(conn, world).is_empty(),
            "a request pauses nothing"
        );
        assert_eq!(pause_events(conn, world), 0);
        Ok(())
    });
}

/// FR-033: a second trigger for a world with a pending request joins it, and
/// the same moderation action raised again records once.
#[test]
fn a_second_raise_joins_the_pending_request_and_a_repeat_records_once() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let owner = insert_test_user(conn);
        let world = insert_test_world(conn, owner);
        let (first, second) = (Uuid::now_v7(), Uuid::now_v7());

        let request_id = requested(raise(conn, world, &takedown(first)).unwrap());
        assert_eq!(
            requested(raise(conn, world, &takedown(second)).unwrap()),
            request_id
        );
        assert_eq!(
            requested(raise(conn, world, &takedown(first)).unwrap()),
            request_id,
            "a retried hook lands on the same request"
        );

        assert_eq!(requests_of(conn, world).len(), 1, "one request per world");
        let actions: Vec<_> = triggers_of_request(conn, request_id)?
            .into_iter()
            .map(|t| t.moderation_action_id)
            .collect();
        assert_eq!(actions, vec![Some(first), Some(second)], "each action once");
        Ok(())
    });
}

/// FR-036: a trigger for a paused world is added to the pause, and no request
/// is made.
#[test]
fn a_raise_for_a_paused_world_joins_the_pause_and_makes_no_request() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);
        let pause = pause_world(conn, operator, world, "Grounds.", TriggerDetail::operator())
            .unwrap()
            .pause;
        let action = Uuid::now_v7();

        assert_eq!(
            raise(conn, world, &takedown(action)).unwrap(),
            RaiseOutcome::AddedToPause { pause_id: pause.id }
        );
        raise(conn, world, &takedown(action)).unwrap();

        assert!(requests_of(conn, world).is_empty(), "no request");
        let triggers = pause_triggers(conn, pause.id);
        assert_eq!(triggers.len(), 2, "the operator's, and the takedown once");
        assert_eq!(triggers[1].moderation_action_id, Some(action));
        assert_eq!(pauses_of(conn, world).len(), 1, "no second pause");
        assert_eq!(pause_events(conn, world), 1, "no second event");
        Ok(())
    });
}

/// US3 AS4: a takedown on a world nobody is playing raises nothing. A beat
/// two minutes old is nobody playing.
#[test]
fn a_takedown_on_a_world_not_in_live_play_raises_nothing() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let owner = insert_test_user(conn);
        let never = insert_test_world(conn, owner);
        let long_ago = insert_test_world(conn, owner);
        let live = insert_test_world(conn, owner);
        diesel::insert_into(world_live_play::table)
            .values((
                world_live_play::world_id.eq(long_ago),
                world_live_play::last_beat_at
                    .eq(chrono::Utc::now().naive_utc() - chrono::Duration::minutes(2)),
            ))
            .execute(conn)?;
        played(conn, live);

        for world in [never, long_ago] {
            assert_eq!(
                raise_for_takedown(conn, &reach(world, Uuid::now_v7())).unwrap(),
                RaiseOutcome::NotLive
            );
            assert!(requests_of(conn, world).is_empty());
        }
        requested(raise_for_takedown(conn, &reach(live, Uuid::now_v7())).unwrap());
        assert_eq!(requests_of(conn, live).len(), 1);
        Ok(())
    });
}

/// FR-034: approving pauses the world on the note, naming the request, with
/// event 28. The request's triggers stay on the request, and another world's
/// request is untouched.
#[test]
fn approving_pauses_with_the_request_named_and_moves_nothing_else() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);
        let other = insert_test_world(conn, operator);
        let request_id = requested(raise(conn, world, &takedown(Uuid::now_v7())).unwrap());
        let other_request = requested(raise(conn, other, &takedown(Uuid::now_v7())).unwrap());

        assert!(matches!(
            decide(conn, operator, request_id, Decision::Approve, "  "),
            Err(PauseError::GroundsRequired)
        ));
        assert!(matches!(
            decide(conn, operator, Uuid::now_v7(), Decision::Approve, "Note."),
            Err(PauseError::RequestNotFound)
        ));

        let outcome = decide(
            conn,
            operator,
            request_id,
            Decision::Approve,
            " The scene is on the table. ",
        )
        .unwrap();
        assert!(outcome.decided_here);
        assert_eq!(outcome.request.state, PauseRequestState::Approved);
        assert_eq!(outcome.request.decided_by, Some(operator));
        assert_eq!(
            outcome.request.decision_note.as_deref(),
            Some("The scene is on the table.")
        );

        let pause = outcome.pause.expect("approval pauses");
        assert!(pause.is_active());
        assert_eq!(pause.request_id, Some(request_id));
        assert_eq!(pause.grounds, "The scene is on the table.");
        assert_eq!(pauses_of(conn, world).len(), 1);
        assert_eq!(pause_events(conn, world), 1, "event 28, as a direct pause");
        assert!(
            pause_triggers(conn, pause.id).is_empty(),
            "the triggers stay on the request, reached through request_id"
        );
        assert_eq!(triggers_of_request(conn, request_id)?.len(), 1);

        let [untouched] = requests_of(conn, other).try_into().unwrap();
        assert_eq!(untouched.id, other_request);
        assert_eq!(untouched.state, PauseRequestState::Pending);
        assert!(pauses_of(conn, other).is_empty());
        Ok(())
    });
}

/// FR-034: declining records the decision on the request, and nothing a
/// member can reach: no event, no pause, no history.
#[test]
fn declining_records_no_event_and_nothing_a_member_sees() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);
        let request_id = requested(raise(conn, world, &takedown(Uuid::now_v7())).unwrap());

        let outcome = decide(conn, operator, request_id, Decision::Decline, "Not live.").unwrap();
        assert!(outcome.decided_here);
        assert_eq!(outcome.request.state, PauseRequestState::Declined);
        assert!(outcome.pause.is_none());

        assert_eq!(pause_events(conn, world), 0);
        assert!(pauses_of(conn, world).is_empty());
        let seen = world_play_state_sync(conn, world)?;
        assert!(!seen.paused && seen.history.is_empty());

        // A later raise opens a fresh request: the declined one is final.
        let again = requested(raise(conn, world, &takedown(Uuid::now_v7())).unwrap());
        assert_ne!(again, request_id);
        Ok(())
    });
}

/// FR-036 on approval: a world paused while its request waited takes the
/// request's triggers, and the approver's note, onto the pause that exists.
#[test]
fn approving_a_request_for_a_paused_world_adds_to_that_pause() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let operator = insert_test_user(conn);
        let world = insert_test_world(conn, operator);
        let action = Uuid::now_v7();
        let request_id = requested(raise(conn, world, &takedown(action)).unwrap());
        let pause = pause_world(conn, operator, world, "Direct.", TriggerDetail::operator())
            .unwrap()
            .pause;

        let outcome = decide(conn, operator, request_id, Decision::Approve, "Agreed.").unwrap();
        assert!(outcome.decided_here);
        assert_eq!(outcome.pause.map(|p| p.id), Some(pause.id));
        assert_eq!(pauses_of(conn, world).len(), 1, "no second pause");
        assert_eq!(pause_events(conn, world), 1, "no second event");

        let triggers = pause_triggers(conn, pause.id);
        let kinds: Vec<_> = triggers.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PauseTriggerKind::Operator,
                PauseTriggerKind::Operator,
                PauseTriggerKind::Takedown
            ]
        );
        assert_eq!(triggers[1].note.as_deref(), Some("Agreed."));
        assert_eq!(triggers[2].moderation_action_id, Some(action));
        Ok(())
    });
}

/// FR-035, on two connections at once: two operators approve one request
/// together. Exactly one pause results, and the operator who lost is told who
/// decided.
///
/// Committed rather than rolled back — two connections cannot share one test
/// transaction, and sharing one would test nothing. The rows are records the
/// schema forbids deleting, in a world of their own.
#[test]
fn two_concurrent_decisions_make_one_pause_and_the_loser_is_told_who() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let (world, request_id, first, second) = {
        let mut conn = state.db_pool.get().unwrap();
        let first = insert_test_user(&mut conn);
        let second = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, first);
        let request_id = requested(raise(&mut conn, world, &takedown(Uuid::now_v7())).unwrap());
        (world, request_id, first, second)
    };

    let barrier = Arc::new(Barrier::new(2));
    let deciders: Vec<_> = [first, second]
        .into_iter()
        .map(|operator| {
            let mut conn = state.db_pool.get().unwrap();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                decide(
                    &mut conn,
                    operator,
                    request_id,
                    Decision::Approve,
                    "Both of us saw it.",
                )
                .map(|outcome| (operator, outcome))
            })
        })
        .collect();
    let outcomes: Vec<_> = deciders
        .into_iter()
        .map(|decider| decider.join().unwrap().expect("neither decision errors"))
        .collect();

    let winners: Vec<_> = outcomes.iter().filter(|(_, o)| o.decided_here).collect();
    let losers: Vec<_> = outcomes.iter().filter(|(_, o)| !o.decided_here).collect();
    assert_eq!((winners.len(), losers.len()), (1, 1), "one decision wins");
    let (winner, won) = winners[0];
    let (_, lost) = losers[0];

    assert_eq!(lost.request.decided_by, Some(*winner));
    assert_eq!(lost.request.decided_by_name, won.request.decided_by_name);
    assert_eq!(lost.request.decided_at, won.request.decided_at);
    assert_eq!(
        lost.pause.as_ref().map(|p| p.id),
        won.pause.as_ref().map(|p| p.id),
        "the loser is shown the winner's pause"
    );

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(pauses_of(&mut conn, world).len(), 1, "exactly one pause");
    assert_eq!(pause_events(&mut conn, world), 1);
}
