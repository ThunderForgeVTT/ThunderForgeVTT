//! Spec 040 T009 (FR-028): `up.sql` → `down.sql` → `up.sql` leaves the schema
//! exactly as it was.
//!
//! # Why this is worth a test at all
//!
//! A `down.sql` is only ever run by somebody having a bad day — a downgrade, a
//! rolled-back deploy, a developer rewinding to bisect. It is therefore the
//! least-exercised SQL in the repository and the most consequential when it is
//! wrong. The two failures it is written against are a `down` that drops less
//! than `up` created, which makes the next `up` fail on an object that already
//! exists, and a `down` that drops *more*, which takes an unrelated table with
//! it.
//!
//! # Why it runs in a transaction it rolls back
//!
//! Postgres makes DDL transactional, so the whole cycle can be executed
//! against the ordinary test database and then undone. The alternative —
//! a scratch database — would need the entire migration chain replayed into
//! it, because `instance_settings.updated_by` references `users(id)`, and
//! replaying forty migrations to test one is a cost with no matching answer.
//!
//! It takes `test_env::lock()` for the same reason every other test in this
//! feature does: it is holding an exclusive lock on the two tables those tests
//! read and write, and the ledger's own notes record what unserialised
//! settings tests cost once already.

use diesel::connection::SimpleConnection;
use diesel::prelude::*;

use crate::settings::test_env;
use crate::test_support::test_app_state;

const UP: &str = include_str!("../../migrations/2026-09-07-120000-0000_instance_settings/up.sql");
const DOWN: &str =
    include_str!("../../migrations/2026-09-07-120000-0000_instance_settings/down.sql");

/// Every column the resolver and the change writer actually read, by table.
/// Named here rather than counted, so a migration that adds a column does not
/// fail this test and a migration that drops one does.
const EXPECTED: &[(&str, &[&str])] = &[
    (
        "instance_settings",
        &[
            "key",
            "value",
            "updated_by",
            "updated_at",
            "created_by",
            "created_at",
        ],
    ),
    (
        "instance_setting_changes",
        &[
            "id",
            "key",
            "previous_value",
            "new_value",
            "redacted",
            "changed_by",
            "changed_at",
            "source",
        ],
    ),
];

#[derive(QueryableByName)]
struct Name {
    #[diesel(sql_type = diesel::sql_types::Text)]
    name: String,
}

fn columns(conn: &mut PgConnection, table: &str) -> Vec<String> {
    diesel::sql_query(
        "SELECT column_name AS name FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 ORDER BY column_name",
    )
    .bind::<diesel::sql_types::Text, _>(table)
    .load::<Name>(conn)
    .expect("read the columns")
    .into_iter()
    .map(|row| row.name)
    .collect()
}

fn indexes(conn: &mut PgConnection, table: &str) -> Vec<String> {
    diesel::sql_query(
        "SELECT indexname AS name FROM pg_indexes \
         WHERE schemaname = 'public' AND tablename = $1 ORDER BY indexname",
    )
    .bind::<diesel::sql_types::Text, _>(table)
    .load::<Name>(conn)
    .expect("read the indexes")
    .into_iter()
    .map(|row| row.name)
    .collect()
}

fn snapshot(conn: &mut PgConnection) -> Vec<(String, Vec<String>, Vec<String>)> {
    EXPECTED
        .iter()
        .map(|(table, _)| {
            (
                (*table).to_string(),
                columns(conn, table),
                indexes(conn, table),
            )
        })
        .collect()
}

#[test]
fn down_then_up_leaves_the_schema_it_started_with() {
    let _guard = test_env::lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");

    // Every assertion below happens inside this transaction, and the
    // transaction is never committed. `test_transaction` rolls back whatever
    // the closure did, including the DDL.
    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        let before = snapshot(conn);

        // The tables must exist to begin with, or this test would pass
        // trivially against a database where the migration never ran — the
        // "passed by not running" shape the ledger has already paid for once.
        for (table, expected) in EXPECTED {
            let found = columns(conn, table);
            assert!(
                !found.is_empty(),
                "`{table}` does not exist before the cycle; the test database is not migrated"
            );
            for column in *expected {
                assert!(
                    found.iter().any(|c| c == column),
                    "`{table}` is missing `{column}` before the cycle"
                );
            }
        }

        conn.batch_execute(DOWN).expect("down.sql runs");

        for (table, _) in EXPECTED {
            assert!(
                columns(conn, table).is_empty(),
                "`{table}` survived down.sql — the next up.sql would fail on an object that already exists"
            );
        }

        conn.batch_execute(UP).expect("up.sql runs after down.sql");

        assert_eq!(
            snapshot(conn),
            before,
            "the schema after down → up is not the schema before it"
        );

        Ok(())
    });
}

#[test]
fn down_drops_only_what_up_created() {
    let _guard = test_env::lock();
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        // `instance_settings.updated_by` references `users`, and
        // `instance_access_settings` is spec 035's table sitting beside these
        // two. A `DROP ... CASCADE` in the wrong place takes one of them, and
        // the symptom would be a downgrade that quietly destroys accounts.
        let neighbours = ["users", "instance_access_settings"];
        let before: Vec<Vec<String>> = neighbours.iter().map(|t| columns(conn, t)).collect();

        conn.batch_execute(DOWN).expect("down.sql runs");

        for (table, expected) in neighbours.iter().zip(before) {
            assert_eq!(
                columns(conn, table),
                expected,
                "down.sql changed `{table}`, which it did not create"
            );
        }

        Ok(())
    });
}
