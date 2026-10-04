//! Whether a world is *in live play* (research R3).
//!
//! A world is in live play when a member's heartbeat reached the server in the
//! last [`LIVE_WITHIN_SECONDS`] seconds. The heartbeat writes that down in `world_live_play`,
//! durably, because a takedown deciding whether to ask for a pause must get
//! the same answer whichever process filed it — and the presence registry
//! lives in one process's memory.
//!
//! # One write per world, not per beat
//!
//! Every client beats every five seconds. Writing each beat would be the cost
//! the heartbeat was moved into memory to avoid. So a process skips the write
//! when it marked the world under [`MARK_EVERY`] ago, and the write itself is
//! conditional on the stored mark being older than that — which covers the
//! other processes, so a busy world costs one row update every 30 seconds
//! however many clients and processes it has.
//!
//! 30 seconds of throttle inside a 45-second window leaves one missed mark's
//! worth of slack: a world beaten at t=0 and marked, then at t=29 (skipped),
//! is marked again by t=35 at the latest, well inside the window.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use diesel::dsl::{IntervalDsl as _, now};
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::world_live_play;

/// A beat this recent means the world is being played.
pub const LIVE_WITHIN_SECONDS: i32 = 45;
/// A process writes a world's mark at most this often.
pub const MARK_EVERY: Duration = Duration::from_secs(MARK_EVERY_SECONDS as u64);
const MARK_EVERY_SECONDS: i32 = 30;

/// When this process last wrote each world's mark.
static MARKED: LazyLock<Mutex<HashMap<Uuid, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Past this many worlds, forget the ones whose marks this process could
/// write again anyway. Keeps a long-lived process from holding every world it
/// ever saw.
const PRUNE_ABOVE: usize = 4096;

fn recently_marked(world_id: Uuid, at: Instant) -> bool {
    let marked = MARKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    marked
        .get(&world_id)
        .is_some_and(|last| at.saturating_duration_since(*last) < MARK_EVERY)
}

fn remember_mark(world_id: Uuid, at: Instant) {
    let mut marked = MARKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if marked.len() > PRUNE_ABOVE {
        marked.retain(|_, last| at.saturating_duration_since(*last) < MARK_EVERY);
    }
    marked.insert(world_id, at);
}

/// Record that `world_id` is being played, unless this process already did
/// under [`MARK_EVERY`] ago. Returns whether it wrote.
///
/// The in-process note is taken only after the write succeeds, so a failed
/// write is tried again on the next beat rather than 30 seconds later.
pub fn mark_live(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<bool> {
    let at = Instant::now();
    if recently_marked(world_id, at) {
        return Ok(false);
    }

    // Written as SQL: Diesel's upsert has no `WHERE` on `DO UPDATE`. The
    // condition is the other processes' throttle — a mark written by anyone
    // under 30 seconds ago serves.
    diesel::sql_query(
        "INSERT INTO world_live_play (world_id, last_beat_at) VALUES ($1, now()) \
         ON CONFLICT (world_id) DO UPDATE SET last_beat_at = excluded.last_beat_at \
         WHERE world_live_play.last_beat_at < excluded.last_beat_at - make_interval(secs => $2)",
    )
    .bind::<diesel::sql_types::Uuid, _>(world_id)
    .bind::<diesel::sql_types::Integer, _>(MARK_EVERY_SECONDS)
    .execute(conn)?;

    remember_mark(world_id, at);
    Ok(true)
}

/// Whether `world_id` had a beat within [`LIVE_WITHIN_SECONDS`].
pub fn in_live_play(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<bool> {
    Ok(live_among(conn, &[world_id])?.contains(&world_id))
}

/// Which of `world_ids` are in live play, in one query — for a page of
/// requests or pauses, each of which says whether it is played now (FR-031).
pub fn live_among(conn: &mut PgConnection, world_ids: &[Uuid]) -> QueryResult<HashSet<Uuid>> {
    if world_ids.is_empty() {
        return Ok(HashSet::new());
    }
    Ok(world_live_play::table
        .filter(world_live_play::world_id.eq_any(world_ids))
        .filter(world_live_play::last_beat_at.gt(now - LIVE_WITHIN_SECONDS.seconds()))
        .select(world_live_play::world_id)
        .load::<Uuid>(conn)?
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    fn last_beat(conn: &mut PgConnection, world: Uuid) -> Option<chrono::NaiveDateTime> {
        world_live_play::table
            .filter(world_live_play::world_id.eq(world))
            .select(world_live_play::last_beat_at)
            .first(conn)
            .optional()
            .unwrap()
    }

    /// A beat marks the world live; a second beat from this process inside
    /// 30 seconds writes nothing.
    #[test]
    fn a_beat_marks_the_world_and_the_next_is_throttled() {
        let _lock = super::super::test_lock();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let owner = insert_test_user(conn);
            let world = insert_test_world(conn, owner);
            assert!(!in_live_play(conn, world)?, "never played");

            assert!(mark_live(conn, world)?, "the first beat writes");
            assert!(in_live_play(conn, world)?);
            let first = last_beat(conn, world);

            assert!(!mark_live(conn, world)?, "the second is skipped in-process");
            assert_eq!(last_beat(conn, world), first);
            Ok(())
        });
    }

    /// Another process's recent mark stands: the conditional upsert does not
    /// move it. A stale one is moved.
    #[test]
    fn the_upsert_moves_only_a_mark_older_than_thirty_seconds() {
        let _lock = super::super::test_lock();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let owner = insert_test_user(conn);
            let recent = insert_test_world(conn, owner);
            let stale = insert_test_world(conn, owner);
            let at = |seconds_ago: i64| {
                chrono::Utc::now().naive_utc() - chrono::Duration::seconds(seconds_ago)
            };
            diesel::insert_into(world_live_play::table)
                .values(&vec![
                    (
                        world_live_play::world_id.eq(recent),
                        world_live_play::last_beat_at.eq(at(10)),
                    ),
                    (
                        world_live_play::world_id.eq(stale),
                        world_live_play::last_beat_at.eq(at(120)),
                    ),
                ])
                .execute(conn)?;
            assert!(in_live_play(conn, recent)?);
            assert!(!in_live_play(conn, stale)?, "two minutes is not live");

            let before = last_beat(conn, recent);
            mark_live(conn, recent)?;
            assert_eq!(last_beat(conn, recent), before, "a fresh mark stands");

            mark_live(conn, stale)?;
            assert!(in_live_play(conn, stale)?, "a stale mark is moved");
            assert_eq!(live_among(conn, &[recent, stale])?.len(), 2);
            Ok(())
        });
    }
}
