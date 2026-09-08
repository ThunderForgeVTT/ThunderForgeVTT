//! T051: two people finish setup at the same instant.
//!
//! Split out of `admin_setup.rs` to keep it under the 1000-line gate
//! `scripts/check-file-length.sh` enforces. `use super::*` reaches the
//! handlers, the schema imports and `setup_state_lock`.

use super::*;
use crate::test_support::test_app_state;

/// Puts the singleton setup row into "not finished" for the length of a
/// test and puts it back afterwards — **including when the test panics**,
/// which is why it is a `Drop` and not two statements.
///
/// The first version of this test cleaned up at the end of the function.
/// One failed assertion later, the development instance was sitting there
/// with `setup_completed_at` null, offering its own setup wizard to
/// anybody who loaded it. A test that can leave an instance unconfigured
/// when it fails is worse than the race it is checking for.
struct ReopenedSetup {
    state: AppState,
    restore: Option<AdminBootstrapSetup>,
}

impl ReopenedSetup {
    fn new() -> Self {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        let restore = admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
            .expect("the setup row reads");

        diesel::update(admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)))
            .set((
                admin_bootstrap_setup::setup_completed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                admin_bootstrap_setup::admin_code_hash.eq(Some("not-a-real-hash".to_string())),
            ))
            .execute(&mut conn)
            .expect("the setup row reopens");

        ReopenedSetup { state, restore }
    }
}

impl Drop for ReopenedSetup {
    fn drop(&mut self) {
        let Ok(mut conn) = self.state.db_pool.get() else {
            return;
        };
        match &self.restore {
            Some(row) => {
                let _ = diesel::update(
                    admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)),
                )
                .set((
                    admin_bootstrap_setup::setup_completed_at.eq(row.setup_completed_at),
                    admin_bootstrap_setup::admin_code_hash.eq(row.admin_code_hash.clone()),
                    admin_bootstrap_setup::admin_code_generated_at.eq(row.admin_code_generated_at),
                    admin_bootstrap_setup::updated_at.eq(row.updated_at),
                ))
                .execute(&mut conn);
            }
            None => {
                let _ = diesel::delete(
                    admin_bootstrap_setup::table.filter(admin_bootstrap_setup::id.eq(1)),
                )
                .execute(&mut conn);
            }
        }
    }
}

/// T051 / `contracts/setup.md` rule 6 and its failure table: two people
/// complete setup at the same instant, one succeeds, the other is told
/// `409 setup_complete` — **never** a generic `500` out of a
/// unique-constraint violation.
///
/// Tested at `complete_setup_exclusively` rather than through the axum
/// handler on purpose. The handler's first step is
/// `ensure_admin_setup_code_valid`, and the development database this
/// suite runs against has three hundred administrators and a finished
/// setup row in it, so a handler-level race would be refused before it
/// ever reached the window under test. The race lives in this function;
/// arranging the rest would mean clearing somebody's real data.
#[test]
fn two_concurrent_completions_produce_one_success_and_one_conflict() {
    let _env = crate::settings::test_env::lock();
    let _rows = setup_state_lock();
    let reopened = ReopenedSetup::new();

    let now = Utc::now().naive_utc();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

    let outcomes: Vec<Result<bool, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let pool = reopened.state.db_pool.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    let mut conn = pool.get().expect("a connection");
                    // Both threads hold a connection before either starts,
                    // so the race is over the row and not over the pool.
                    barrier.wait();
                    complete_setup_exclusively(&mut conn, now)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("the thread did not panic"))
            .collect()
    });

    for outcome in &outcomes {
        assert!(
            outcome.is_ok(),
            "a concurrent completion failed outright rather than losing the race: {outcome:?}"
        );
    }

    let winners = outcomes.iter().filter(|o| matches!(o, Ok(true))).count();
    let losers = outcomes.iter().filter(|o| matches!(o, Ok(false))).count();
    assert_eq!(winners, 1, "setup completed {winners} times");
    assert_eq!(losers, 1, "the loser of the race was not told it lost");
}

/// The same function called twice in sequence: completion happens once
/// (FR-006). The race above proves the window is closed; this proves the
/// ordinary path is not merely lucky.
#[test]
fn setup_completes_exactly_once() {
    let _env = crate::settings::test_env::lock();
    let _rows = setup_state_lock();
    let reopened = ReopenedSetup::new();

    let mut conn = reopened.state.db_pool.get().expect("a connection");
    let now = Utc::now().naive_utc();

    assert_eq!(complete_setup_exclusively(&mut conn, now), Ok(true));
    assert_eq!(
        complete_setup_exclusively(&mut conn, now),
        Ok(false),
        "setup completed a second time"
    );
}
