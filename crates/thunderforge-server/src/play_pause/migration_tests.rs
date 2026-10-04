//! Spec 051 T006: what the migration promises, asked of the database directly.
//!
//! Every rule here is also kept by [`super::pause_world`] and
//! [`super::lift_pause`]. These tests go around them on purpose, in raw SQL:
//! the claim is that the record holds against a route that never calls this
//! module, and a test through the module would prove only the module.
//!
//! Each test runs inside a transaction it rolls back, so the rows written to
//! prove a record cannot be deleted do not stay behind as rows that cannot be
//! deleted. A refusal aborts the statement, so each attempt runs in its own
//! savepoint ([`refused`]) and the transaction carries on.

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

const UP: &str =
    include_str!("../../migrations/2026-09-13-170000-0000_pausing_a_worlds_play/up.sql");
const DOWN: &str =
    include_str!("../../migrations/2026-09-13-170000-0000_pausing_a_worlds_play/down.sql");

/// Whether the database refused `sql`. The attempt is rolled back to a
/// savepoint either way, so a refusal does not poison the test's transaction
/// and an unexpected success does not leak into the next assertion.
fn refused(conn: &mut PgConnection, sql: &str) -> bool {
    let outcome = conn.transaction::<(), diesel::result::Error, _>(|conn| {
        conn.batch_execute(sql)?;
        // Undo a success too: the question is only whether it was allowed.
        Err(diesel::result::Error::RollbackTransaction)
    });
    !matches!(outcome, Err(diesel::result::Error::RollbackTransaction))
}

fn allowed(conn: &mut PgConnection, sql: &str) -> bool {
    !refused(conn, sql)
}

fn pause_sql(id: Uuid, world: Uuid, operator: Uuid, grounds: &str) -> String {
    format!(
        "INSERT INTO world_play_pauses \
           (id, world_id, world_name, paused_by, paused_by_name, grounds, created_by, updated_by) \
         VALUES ('{id}', '{world}', 'A World', '{operator}', 'operator', '{grounds}', '{operator}', '{operator}')"
    )
}

fn request_sql(id: Uuid, world: Uuid) -> String {
    format!(
        "INSERT INTO world_play_pause_requests (id, world_id, world_name) \
         VALUES ('{id}', '{world}', 'A World')"
    )
}

fn lift_sql(id: Uuid, operator: Uuid) -> String {
    format!(
        "UPDATE world_play_pauses SET lifted_by = '{operator}', lifted_by_name = 'operator', \
           lifted_at = now(), lift_grounds = 'resolved' WHERE id = '{id}'"
    )
}

struct Fixture {
    operator: Uuid,
    world: Uuid,
}

fn fixture(conn: &mut PgConnection) -> Fixture {
    let operator = insert_test_user(conn);
    let world = insert_test_world(conn, operator);
    Fixture { operator, world }
}

fn count(conn: &mut PgConnection, sql: &str) -> i64 {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    diesel::sql_query(sql)
        .get_result::<Count>(conn)
        .expect("count")
        .n
}

/// One active pause per world, and pausing again after a lift is a new row.
#[test]
fn a_second_active_pause_for_a_world_is_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let first = Uuid::now_v7();
        conn.batch_execute(&pause_sql(first, f.world, f.operator, "abuse"))
            .unwrap();

        assert!(
            refused(
                conn,
                &pause_sql(Uuid::now_v7(), f.world, f.operator, "again")
            ),
            "two active pauses for one world"
        );

        conn.batch_execute(&lift_sql(first, f.operator)).unwrap();
        assert!(
            allowed(
                conn,
                &pause_sql(Uuid::now_v7(), f.world, f.operator, "again")
            ),
            "a lifted pause does not stop the world being paused again"
        );
        Ok(())
    });
}

/// FR-033.
#[test]
fn a_second_pending_request_for_a_world_is_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        conn.batch_execute(&request_sql(Uuid::now_v7(), f.world))
            .unwrap();
        assert!(refused(conn, &request_sql(Uuid::now_v7(), f.world)));
        Ok(())
    });
}

/// FR-004, including grounds that are only whitespace.
#[test]
fn blank_grounds_are_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        assert!(refused(
            conn,
            &pause_sql(Uuid::now_v7(), f.world, f.operator, "")
        ));
        assert!(refused(
            conn,
            &pause_sql(Uuid::now_v7(), f.world, f.operator, "   ")
        ));

        let pause = Uuid::now_v7();
        conn.batch_execute(&pause_sql(pause, f.world, f.operator, "abuse"))
            .unwrap();
        assert!(
            refused(
                conn,
                &format!(
                    "UPDATE world_play_pauses SET lifted_by = '{}', lifted_by_name = 'operator', \
                       lifted_at = now(), lift_grounds = '  ' WHERE id = '{pause}'",
                    f.operator
                )
            ),
            "a lift on blank grounds"
        );
        assert!(
            refused(
                conn,
                &format!("UPDATE world_play_pauses SET lifted_at = now() WHERE id = '{pause}'")
            ),
            "a lift that names nobody"
        );
        Ok(())
    });
}

/// A lift happens once, changes only the lift, and cannot be undone.
#[test]
fn a_lifted_pause_cannot_be_relifted_or_unlifted() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let pause = Uuid::now_v7();
        conn.batch_execute(&pause_sql(pause, f.world, f.operator, "abuse"))
            .unwrap();

        assert!(
            refused(
                conn,
                &format!("UPDATE world_play_pauses SET grounds = 'rewritten' WHERE id = '{pause}'")
            ),
            "an active pause's grounds are not editable"
        );

        conn.batch_execute(&lift_sql(pause, f.operator)).unwrap();

        assert!(refused(conn, &lift_sql(pause, f.operator)), "a second lift");
        assert!(
            refused(
                conn,
                &format!(
                    "UPDATE world_play_pauses SET lifted_by = NULL, lifted_by_name = NULL, \
                       lifted_at = NULL, lift_grounds = NULL WHERE id = '{pause}'"
                )
            ),
            "un-lifting"
        );
        Ok(())
    });
}

/// FR-035: a decision is final.
#[test]
fn a_decided_request_cannot_change() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let request = Uuid::now_v7();
        conn.batch_execute(&request_sql(request, f.world)).unwrap();

        assert!(
            refused(
                conn,
                &format!(
                    "UPDATE world_play_pause_requests SET state = 'Declined' WHERE id = '{request}'"
                )
            ),
            "a decision with no decider, time or note"
        );

        conn.batch_execute(&format!(
            "UPDATE world_play_pause_requests SET state = 'Declined', decided_by = '{op}', \
               decided_by_name = 'operator', decided_at = now(), decision_note = 'not needed' \
             WHERE id = '{request}'",
            op = f.operator
        ))
        .unwrap();

        assert!(refused(
            conn,
            &format!(
                "UPDATE world_play_pause_requests SET state = 'Approved', \
                   decision_note = 'changed my mind' WHERE id = '{request}'"
            )
        ));
        assert!(refused(
            conn,
            &format!(
                "UPDATE world_play_pause_requests SET decision_note = 'reworded' WHERE id = '{request}'"
            )
        ));
        Ok(())
    });
}

/// FR-053: the record is never deleted, whichever of the three tables.
#[test]
fn deleting_from_any_record_table_is_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let pause = Uuid::now_v7();
        let request = Uuid::now_v7();
        conn.batch_execute(&pause_sql(pause, f.world, f.operator, "abuse"))
            .unwrap();
        conn.batch_execute(&request_sql(request, f.world)).unwrap();
        conn.batch_execute(&format!(
            "INSERT INTO world_play_pause_triggers (id, pause_id, kind) \
             VALUES ('{}', '{pause}', 'Operator')",
            Uuid::now_v7()
        ))
        .unwrap();

        assert!(refused(
            conn,
            &format!("DELETE FROM world_play_pauses WHERE id = '{pause}'")
        ));
        assert!(refused(
            conn,
            &format!("DELETE FROM world_play_pause_requests WHERE id = '{request}'")
        ));
        assert!(refused(
            conn,
            &format!("DELETE FROM world_play_pause_triggers WHERE pause_id = '{pause}'")
        ));
        assert!(
            refused(
                conn,
                &format!(
                    "UPDATE world_play_pause_triggers SET note = 'rewritten' WHERE pause_id = '{pause}'"
                )
            ),
            "a trigger is not edited either"
        );
        Ok(())
    });
}

/// A trigger belongs to exactly one of a request and a pause, and a takedown
/// names its action.
#[test]
fn a_trigger_with_both_or_neither_owner_is_refused() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let pause = Uuid::now_v7();
        let request = Uuid::now_v7();
        conn.batch_execute(&pause_sql(pause, f.world, f.operator, "abuse"))
            .unwrap();
        conn.batch_execute(&request_sql(request, f.world)).unwrap();

        assert!(refused(
            conn,
            &format!(
                "INSERT INTO world_play_pause_triggers (id, request_id, pause_id, kind) \
                 VALUES ('{}', '{request}', '{pause}', 'Operator')",
                Uuid::now_v7()
            )
        ));
        assert!(refused(
            conn,
            &format!(
                "INSERT INTO world_play_pause_triggers (id, kind) VALUES ('{}', 'Operator')",
                Uuid::now_v7()
            )
        ));
        assert!(
            refused(
                conn,
                &format!(
                    "INSERT INTO world_play_pause_triggers (id, request_id, kind) \
                     VALUES ('{}', '{request}', 'Takedown')",
                    Uuid::now_v7()
                )
            ),
            "a takedown that names no moderation action"
        );

        // And the idempotence a retried takedown hook relies on.
        let action = Uuid::now_v7();
        let takedown = |id: Uuid| {
            format!(
                "INSERT INTO world_play_pause_triggers (id, request_id, kind, moderation_action_id) \
                 VALUES ('{id}', '{request}', 'Takedown', '{action}')"
            )
        };
        conn.batch_execute(&takedown(Uuid::now_v7())).unwrap();
        assert!(refused(conn, &takedown(Uuid::now_v7())));
        Ok(())
    });
}

/// FR-053 and research R8: the record outlives the world; the live-play mark
/// does not.
#[test]
fn deleting_a_world_keeps_its_pauses_and_takes_its_live_play_mark() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let f = fixture(conn);
        let pause = Uuid::now_v7();
        let request = Uuid::now_v7();
        conn.batch_execute(&pause_sql(pause, f.world, f.operator, "abuse"))
            .unwrap();
        conn.batch_execute(&request_sql(request, f.world)).unwrap();
        conn.batch_execute(&format!(
            "INSERT INTO world_live_play (world_id, last_beat_at) VALUES ('{}', now())",
            f.world
        ))
        .unwrap();

        conn.batch_execute(&format!("DELETE FROM worlds WHERE id = '{}'", f.world))
            .expect("a paused world can still be deleted");

        let world = f.world;
        assert_eq!(
            count(
                conn,
                &format!("SELECT count(*) AS n FROM world_play_pauses WHERE world_id = '{world}'")
            ),
            1
        );
        assert_eq!(
            count(
                conn,
                &format!(
                    "SELECT count(*) AS n FROM world_play_pause_requests WHERE world_id = '{world}'"
                )
            ),
            1
        );
        assert_eq!(
            count(
                conn,
                &format!("SELECT count(*) AS n FROM world_live_play WHERE world_id = '{world}'")
            ),
            0
        );
        Ok(())
    });
}

/// `down.sql` drops every table, enum and function `up.sql` made, and `up.sql`
/// runs again after it. Rolled back, as `settings_migration_tests` is.
#[test]
fn down_leaves_nothing_behind() {
    let _lock = super::test_lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();

    const TABLES: &str = "'world_play_pauses', 'world_play_pause_requests', \
                          'world_play_pause_triggers', 'world_live_play'";
    const TYPES: &str = "'PauseRequestState', 'PauseTriggerKind'";
    const FUNCTIONS: &str = "'world_play_pauses_only_lift', \
                             'world_play_pause_requests_decided_is_final', \
                             'world_play_pause_triggers_append_only'";

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        // Take the locks the cycle needs in one order before it needs them:
        // `worlds` first (dropping `world_live_play`'s foreign key locks it),
        // then this migration's tables. A test elsewhere deleting a world
        // takes `worlds` before `world_live_play`, so the same order cannot
        // deadlock against it.
        conn.batch_execute(
            "LOCK TABLE worlds IN SHARE ROW EXCLUSIVE MODE; \
             LOCK TABLE world_play_pauses, world_play_pause_triggers, \
                        world_play_pause_requests, world_live_play IN ACCESS EXCLUSIVE MODE;",
        )
        .expect("lock");

        let objects = |conn: &mut PgConnection| {
            (
                count(
                    conn,
                    &format!(
                        "SELECT count(*) AS n FROM pg_tables \
                         WHERE schemaname = 'public' AND tablename IN ({TABLES})"
                    ),
                ),
                count(
                    conn,
                    &format!("SELECT count(*) AS n FROM pg_type WHERE typname IN ({TYPES})"),
                ),
                count(
                    conn,
                    &format!("SELECT count(*) AS n FROM pg_proc WHERE proname IN ({FUNCTIONS})"),
                ),
            )
        };

        assert_eq!(
            objects(conn),
            (4, 2, 3),
            "the migration has not run; this test would pass by not running"
        );

        conn.batch_execute(DOWN).expect("down.sql runs");
        assert_eq!(objects(conn), (0, 0, 0), "down.sql left something behind");
        assert!(
            count(
                conn,
                "SELECT count(*) AS n FROM pg_tables WHERE schemaname = 'public' AND tablename = 'worlds'"
            ) == 1,
            "down.sql took a table it did not create"
        );

        conn.batch_execute(UP).expect("up.sql runs after down.sql");
        assert_eq!(objects(conn), (4, 2, 3));
        Ok(())
    });
}
