//! Each check, and the attack it rules out.

use super::*;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn setup() -> (crate::AppState, Uuid, Uuid) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    (state, world, owner)
}

#[test]
fn a_hand_off_can_be_completed_once() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");

    let token = begin(&mut conn, world, owner, Some("/world/x/settings/system")).expect("begun");
    let claim = consume(&mut conn, &token, owner).expect("consumed");

    assert_eq!(claim.world_id, world);
    assert_eq!(claim.return_to.as_deref(), Some("/world/x/settings/system"));
}

/// Without single use, a captured callback URL replays and rebinds a world
/// after the fact.
#[test]
fn a_hand_off_cannot_be_replayed() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");

    let token = begin(&mut conn, world, owner, None).expect("begun");
    consume(&mut conn, &token, owner).expect("first");

    assert_eq!(
        consume(&mut conn, &token, owner),
        Err(GrantRefused::NotValid)
    );
}

/// Anyone who obtains a state could otherwise complete someone else's
/// connection, binding a repository they control to a world they do not own.
#[test]
fn another_user_cannot_complete_someone_elses_hand_off() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");
    let stranger = insert_test_user(&mut conn);

    let token = begin(&mut conn, world, owner, None).expect("begun");

    assert_eq!(
        consume(&mut conn, &token, stranger),
        Err(GrantRefused::NotYours)
    );
}

/// **And the session must survive that attempt.** A failed check that burned
/// the session would let anyone who guesses one wrong state invalidate a
/// legitimate user's in-flight connection — denial of service through the
/// front door.
#[test]
fn a_failed_attempt_does_not_burn_the_session() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");
    let stranger = insert_test_user(&mut conn);

    let token = begin(&mut conn, world, owner, None).expect("begun");
    let _ = consume(&mut conn, &token, stranger);

    assert!(
        consume(&mut conn, &token, owner).is_ok(),
        "a stranger's failed attempt destroyed a legitimate hand-off",
    );
}

/// An abandoned session that stays valid is an attack surface that grows on
/// its own.
#[test]
fn an_expired_hand_off_is_refused() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");

    let token = begin(&mut conn, world, owner, None).expect("begun");
    diesel::update(g::table.filter(g::state.eq(&token)))
        .set(g::expires_at.eq((Utc::now() - Duration::hours(1)).naive_utc()))
        .execute(&mut conn)
        .expect("age it");

    assert_eq!(
        consume(&mut conn, &token, owner),
        Err(GrantRefused::NotValid)
    );
}

#[test]
fn a_state_that_never_existed_is_refused() {
    let (state, _world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");

    assert_eq!(
        consume(&mut conn, "not-a-state-anyone-issued", owner),
        Err(GrantRefused::NotValid),
    );
}

/// The state must not front-load a timestamp: it is the only thing between a
/// callback and a world, and a v7 would narrow a guess and leak when the flow
/// began (ADR-049).
#[test]
fn the_state_carries_no_timestamp() {
    let (state, world, owner) = setup();
    let mut conn = state.db_pool.get().expect("a connection");

    let token = begin(&mut conn, world, owner, None).expect("begun");
    let parsed = Uuid::parse_str(&token).expect("a uuid");

    assert_eq!(
        parsed.get_version_num(),
        4,
        "the grant state leaks its creation time"
    );
}
