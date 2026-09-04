//! FR-038, FR-041a and FR-030, asserted against real rows.
//!
//! Selection is the whole of what this module decides, and each exclusion is a
//! promise the product makes rather than an implementation detail.

use diesel::prelude::*;
use uuid::Uuid;

use super::*;
use crate::models::LoreRepositoryConnection;
use crate::schema::{lore_repository_connections, lore_sync_runs};
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn row(world_id: Uuid, owner: Uuid, state: &str, acknowledged: bool) -> LoreRepositoryConnection {
    let now = chrono::Utc::now().naive_utc();
    LoreRepositoryConnection {
        id: Uuid::now_v7(),
        world_id,
        host_kind: "test".into(),
        installation_ref: "inst".into(),
        repository_ref: format!("owner/{}", Uuid::now_v7()),
        branch: "main".into(),
        directory: "lore".into(),
        incoming_enabled: false,
        notice_acknowledged_at: acknowledged.then_some(now),
        state: state.into(),
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
    }
}

fn insert(conn: &mut PgConnection, r: &LoreRepositoryConnection) -> Uuid {
    diesel::insert_into(lore_repository_connections::table)
        .values(r.clone())
        .execute(conn)
        .expect("insert connection");
    r.id
}

fn insert_run(
    conn: &mut PgConnection,
    connection_id: Uuid,
    started_at: chrono::NaiveDateTime,
    outcome: Option<&str>,
    attempt: i32,
) {
    diesel::insert_into(lore_sync_runs::table)
        .values((
            lore_sync_runs::id.eq(Uuid::now_v7()),
            lore_sync_runs::connection_id.eq(connection_id),
            lore_sync_runs::started_at.eq(started_at),
            lore_sync_runs::outcome.eq(outcome.map(str::to_string)),
            lore_sync_runs::attempt.eq(attempt),
        ))
        .execute(conn)
        .expect("insert run");
}

fn fresh() -> (
    crate::AppState,
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>,
    Uuid,
    Uuid,
) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    (state, conn, world, owner)
}

fn is_due(conn: &mut PgConnection, id: Uuid) -> bool {
    due_now(conn, chrono::Utc::now().naive_utc())
        .expect("selection")
        .iter()
        .any(|d| d.connection_id == id)
}

/// **FR-038.** The acknowledgement is a gate, not a display preference. A
/// world whose Game Master has not agreed to what leaving the platform means
/// must not have left it.
#[test]
fn an_unacknowledged_connection_never_runs() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "never_configured", false));

    assert!(
        !is_due(&mut conn, id),
        "a world synchronised without consent"
    );
}

#[test]
fn an_acknowledged_connection_that_has_never_run_goes_first() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "working", true));

    assert!(is_due(&mut conn, id));
}

/// **FR-041a.** Deactivation is an enforcement action, and a pass that resumed
/// one would undo a decision the platform made deliberately.
#[test]
fn a_deactivated_connection_never_runs() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "deactivated", true));

    assert!(!is_due(&mut conn, id), "an enforcement action was undone");
}

/// Two passes over one connection would have two processes writing one working
/// clone.
#[test]
fn a_connection_with_a_run_still_in_flight_is_not_started_again() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "working", true));
    insert_run(&mut conn, id, chrono::Utc::now().naive_utc(), None, 1);

    assert!(!is_due(&mut conn, id));
}

/// **FR-030.** Selecting a connection that just failed would hammer a host
/// already telling us it is unhappy.
#[test]
fn a_recently_failed_connection_waits_before_retrying() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "needs_attention", true));
    insert_run(
        &mut conn,
        id,
        chrono::Utc::now().naive_utc(),
        Some("failed"),
        1,
    );

    assert!(!is_due(&mut conn, id), "a failure was retried immediately");
}

#[test]
fn a_connection_whose_backoff_has_elapsed_runs_again() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "needs_attention", true));
    let long_ago = chrono::Utc::now().naive_utc() - chrono::Duration::hours(4);
    insert_run(&mut conn, id, long_ago, Some("failed"), 3);

    assert!(
        is_due(&mut conn, id),
        "a recovered connection never retried"
    );
}

#[test]
fn a_succeeded_run_does_not_hold_the_next_pass_back() {
    let (_state, mut conn, world, owner) = fresh();
    let id = insert(&mut conn, &row(world, owner, "working", true));
    insert_run(
        &mut conn,
        id,
        chrono::Utc::now().naive_utc(),
        Some("succeeded"),
        1,
    );

    assert!(is_due(&mut conn, id));
}

/// The intervals lengthen and then stop lengthening. A schedule that grew
/// forever would eventually never retry, which is indistinguishable from
/// giving up without saying so.
#[test]
fn the_backoff_lengthens_and_then_settles() {
    let t0 = chrono::Utc::now().naive_utc();
    let gap = |attempt| (next_attempt_after(t0, attempt) - t0).num_seconds();

    assert_eq!(gap(1), 30);
    assert!(gap(2) > gap(1));
    assert!(gap(5) > gap(3));
    assert_eq!(gap(9), 3600);
    assert_eq!(gap(50), 3600, "the schedule grew without bound");
    // A nonsensical attempt count must not panic or produce a negative wait.
    assert_eq!(gap(0), 30);
    assert_eq!(gap(-3), 30);
}
