//! Which connections a pass should run for, and how often.
//!
//! # Why the selection is its own function
//!
//! Because it is where FR-038 lives, and FR-038 is a promise: **no world
//! synchronises until its Game Master has acknowledged what leaving the
//! platform means.** A promise enforced by an `if` inside a loop inside a
//! spawned task is a promise nothing can test. Pulled out, "an unacknowledged
//! connection is never selected" is one assertion against a database.
//!
//! The same is true of the other two exclusions, and each is a different kind
//! of "not now":
//!
//! - **Unacknowledged** — the Game Master has not agreed yet. Not an error,
//!   and not something to retry; it waits for a person.
//! - **Deactivated** — an enforcement action (FR-041a). The only state a Game
//!   Master cannot leave by fixing something, and a pass that resumed one
//!   would undo a decision the platform made deliberately.
//! - **Backing off** — a run failed recently and FR-030 requires progressively
//!   longer intervals. Selecting it anyway would hammer a host that is already
//!   telling us it is unhappy.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

/// How often the task wakes. SC-003 allows sixty seconds from an edit to the
/// repository, so a shorter tick buys nothing a Game Master would notice and
/// costs a fetch per connection.
pub const TICK_SECONDS: u64 = 30;

/// The backoff schedule, in seconds, indexed by consecutive failures.
///
/// It ends rather than growing forever: a connection that has failed nine
/// times is broken in a way retrying will not fix, and hourly is the honest
/// floor — often enough to recover on its own when a host comes back, rare
/// enough not to be noise. FR-029 has already told the Game Master by then.
const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600];

/// When a connection with this many consecutive failures may next be tried.
pub fn next_attempt_after(last_attempt: NaiveDateTime, consecutive_failures: i32) -> NaiveDateTime {
    let index = (consecutive_failures.max(1) as usize - 1).min(BACKOFF_SECONDS.len() - 1);
    last_attempt + Duration::seconds(BACKOFF_SECONDS[index])
}

/// One connection the task should run a pass for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub connection_id: Uuid,
    pub world_id: Uuid,
}

/// Connections due for a pass now.
///
/// Ordered by how long they have waited, so a busy instance starves nobody:
/// without an order, the same few worlds would be served every tick because
/// the database is free to return rows in whatever order suits it.
pub fn due_now(conn: &mut PgConnection, now: NaiveDateTime) -> Result<Vec<Due>, String> {
    use crate::schema::{lore_repository_connections as c, lore_sync_runs as r};

    let candidates: Vec<(Uuid, Uuid, Option<NaiveDateTime>)> = c::table
        // FR-038. The acknowledgement is a gate, not a display preference.
        .filter(c::notice_acknowledged_at.is_not_null())
        // FR-041a. An enforcement deactivation is not something a pass lifts.
        .filter(c::state.ne("deactivated"))
        .select((c::id, c::world_id, c::last_synced_at))
        .order(c::last_synced_at.asc().nulls_first())
        .load(conn)
        .map_err(|e| format!("Failed to load connections: {e}"))?;

    let mut due = Vec::new();
    for (connection_id, world_id, _) in candidates {
        let latest: Option<(NaiveDateTime, Option<String>, i32)> = r::table
            .filter(r::connection_id.eq(connection_id))
            .order(r::started_at.desc())
            .select((r::started_at, r::outcome, r::attempt))
            .first(conn)
            .optional()
            .map_err(|e| format!("Failed to load runs: {e}"))?;

        let ready = match latest {
            // Never run. A newly acknowledged connection goes first, which is
            // also what makes SC-001's five minutes achievable.
            None => true,
            // A run that has not finished is still in flight; starting a
            // second pass for one connection would have two processes writing
            // one clone.
            Some((_, None, _)) => false,
            Some((started_at, Some(outcome), attempt)) => match outcome.as_str() {
                "succeeded" => true,
                _ => now >= next_attempt_after(started_at, attempt),
            },
        };

        if ready {
            due.push(Due {
                connection_id,
                world_id,
            });
        }
    }

    Ok(due)
}

/// Start the background task.
///
/// The shape `main.rs` already uses four times — a `spawn_*_task` in the
/// library, called from the binary, owning its own schedule and staying off
/// every hot path. No new infrastructure, deliberately: a queue or an external
/// scheduler would be a new deployment component for a loop the process can
/// hold itself.
pub fn spawn_lore_sync_task(state: crate::AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECONDS));
        loop {
            interval.tick().await;

            // A tick that cannot read the database is not an error worth
            // shouting about every thirty seconds — the rest of the server is
            // already failing loudly if the pool is gone.
            let Ok(mut conn) = state.db_pool.get() else {
                continue;
            };
            let now = Utc::now().naive_utc();
            match due_now(&mut conn, now) {
                Ok(due) if !due.is_empty() => {
                    eprintln!("[LoreSync] {} connection(s) due", due.len());
                }
                Ok(_) => {}
                Err(e) => eprintln!("[LoreSync] ⚠️  could not select connections: {e}"),
            }
        }
    });
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod schedule_tests;
