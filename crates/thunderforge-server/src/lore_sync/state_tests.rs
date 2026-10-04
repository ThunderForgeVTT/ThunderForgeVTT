//! The transitions, and the one that must never happen.

use uuid::Uuid;

use super::*;
use crate::models::LoreRepositoryConnection;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn connected() -> (crate::AppState, Uuid) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let now = chrono::Utc::now().naive_utc();

    diesel::insert_into(c::table)
        .values(LoreRepositoryConnection {
            id: Uuid::now_v7(),
            world_id: world,
            host_kind: "test".into(),
            installation_ref: "1".into(),
            repository_ref: format!("owner/{}", Uuid::now_v7()),
            branch: "main".into(),
            directory: "lore".into(),
            incoming_enabled: false,
            notice_acknowledged_at: Some(now),
            state: "working".into(),
            state_reason: None,
            repository_is_public: None,
            visibility_checked_at: None,
            deactivated_at: None,
            deactivated_reason: None,
            last_synced_at: None,
            last_written_commit: None,
            created_by: owner,
            updated_by: owner,
            created_at: now,
            updated_at: now,
        })
        .execute(&mut conn)
        .expect("insert connection");

    (state, world)
}

#[test]
fn a_failure_records_the_remedy_with_the_state() {
    let (state, world) = connected();
    let mut conn = state.db_pool.get().expect("a connection");

    to_needs_attention(&mut conn, world, "Re-grant access at your repository host.")
        .expect("transition");

    let (stored, reason) = c::table
        .filter(c::world_id.eq(world))
        .select((c::state, c::state_reason))
        .first::<(String, Option<String>)>(&mut conn)
        .expect("loaded");

    assert_eq!(State::from_db_str(&stored), State::NeedsAttention);
    assert_eq!(
        reason.as_deref(),
        Some("Re-grant access at your repository host."),
        "a state that says something is wrong without saying what",
    );
}

#[test]
fn recovering_clears_the_reason() {
    let (state, world) = connected();
    let mut conn = state.db_pool.get().expect("a connection");

    to_needs_attention(&mut conn, world, "The host was unreachable.").expect("failed");
    to_working(&mut conn, world).expect("recovered");

    let reason = c::table
        .filter(c::world_id.eq(world))
        .select(c::state_reason)
        .first::<Option<String>>(&mut conn)
        .expect("loaded");

    assert_eq!(reason, None, "a stale remedy survived recovery");
}

/// **FR-041a, and the reason this module exists.**
///
/// A pass that ran against a deactivated connection and then marked it healthy
/// would undo an enforcement action through the back door — and it would look
/// like ordinary success in the log, which is the worst way for it to happen.
#[test]
fn a_successful_pass_cannot_lift_a_deactivation() {
    let (state, world) = connected();
    let mut conn = state.db_pool.get().expect("a connection");

    diesel::update(c::table.filter(c::world_id.eq(world)))
        .set(c::state.eq("deactivated"))
        .execute(&mut conn)
        .expect("deactivate");

    assert_eq!(to_working(&mut conn, world), Err(Refused::Deactivated));
    assert_eq!(
        current(&mut conn, world).expect("loaded"),
        Some(State::Deactivated),
        "the deactivation was lifted",
    );
}

#[test]
fn a_failure_cannot_move_a_deactivation_either() {
    let (state, world) = connected();
    let mut conn = state.db_pool.get().expect("a connection");

    diesel::update(c::table.filter(c::world_id.eq(world)))
        .set(c::state.eq("deactivated"))
        .execute(&mut conn)
        .expect("deactivate");

    assert_eq!(
        to_needs_attention(&mut conn, world, "anything"),
        Err(Refused::Deactivated),
    );
}

/// A build that does not understand what it read must not call a connection
/// healthy on the strength of not understanding it.
#[test]
fn an_unknown_stored_state_resolves_towards_attention() {
    assert_eq!(
        State::from_db_str("something-a-later-migration-added"),
        State::NeedsAttention,
    );
    for s in [
        State::NeverConfigured,
        State::Working,
        State::NeedsAttention,
        State::Deactivated,
    ] {
        assert_eq!(State::from_db_str(s.as_db_str()), s);
    }
}

#[test]
fn only_a_live_connection_admits_a_pass() {
    assert!(State::Working.admits_a_pass());
    assert!(
        State::NeedsAttention.admits_a_pass(),
        "a failure must be retryable"
    );
    assert!(!State::Deactivated.admits_a_pass());
    assert!(!State::NeverConfigured.admits_a_pass());
}
