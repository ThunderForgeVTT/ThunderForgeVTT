//! What a pass costs at the size the success criteria name.
//!
//! # Why this is a test rather than a benchmark
//!
//! SC-001 gives a Game Master five minutes to connect a repository and see a
//! 200-entry world in it, and SC-003 puts an edit in the repository within
//! sixty seconds. Both are budgets, and both are dominated by one thing this
//! code controls: how long it takes to turn a world into a plan. Everything
//! else in the pass is network.
//!
//! So the useful thing to know is not a precise number but whether planning is
//! a rounding error against those budgets or a threat to them. A test with a
//! generous ceiling answers that and keeps answering it — a benchmark nobody
//! runs answers it once.
//!
//! The ceiling is deliberately loose. This runs on whatever machine happens to
//! be building, beside every other test, against a shared database. A tight
//! bound would fail for reasons that have nothing to do with the code, and a
//! test that fails for unrelated reasons is a test people learn to ignore.

use diesel::prelude::*;
use uuid::Uuid;

use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

/// SC-001's world size.
const ENTRIES: usize = 200;

/// Generous by design — see the module header. Planning that took even a
/// second here would still be invisible against SC-003's sixty, but it would
/// be worth knowing about.
const CEILING: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn planning_a_two_hundred_entry_world_is_not_the_expensive_part() {
    use crate::schema::world_lore_entries as e;

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);

    let now = chrono::Utc::now().naive_utc();
    let mut parents: Vec<Uuid> = Vec::new();

    for i in 0..ENTRIES {
        let id = Uuid::now_v7();
        // A tree rather than a flat list: path assignment walks ancestors, so
        // a flat world would measure the cheap shape and miss the one SC-001
        // actually describes.
        let parent = if i % 5 == 0 {
            None
        } else {
            parents.last().copied()
        };
        diesel::insert_into(e::table)
            .values((
                e::id.eq(id),
                e::world_id.eq(world),
                e::title.eq(format!("Entry {i}")),
                e::slug.eq(format!("s-{}", id.simple())),
                e::content.eq(format!(
                    "Body of entry {i}.\n\nWith a link to [[Entry {}]].",
                    i.saturating_sub(1)
                )),
                e::created_by.eq(owner),
                e::created_at.eq(now),
                e::updated_at.eq(now),
                e::parent_id.eq(parent),
            ))
            .execute(&mut conn)
            .expect("insert entry");
        if i % 5 == 0 {
            parents.push(id);
        }
    }

    let started = std::time::Instant::now();
    let plan = crate::lore_sync::plan::plan_world(&state, world)
        .await
        .expect("a plan");
    let elapsed = started.elapsed();

    assert_eq!(plan.files.len(), ENTRIES, "not every entry was planned");
    assert!(
        elapsed < CEILING,
        "planning {ENTRIES} entries took {elapsed:?}, which threatens SC-001's five \
         minutes and SC-003's sixty seconds — the budget is supposed to be spent on \
         the network, not here",
    );

    eprintln!("[scale] planned {ENTRIES} entries in {elapsed:?}");
}
