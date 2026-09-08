//! The pass: what is due, how long it waits after a refusal, and the two
//! other jobs that ride the same tick.
//!
//! # Why the selection is its own function
//!
//! `lore_sync/schedule.rs` states the reason and it applies here verbatim: "a
//! promise enforced by an `if` inside a loop inside a spawned task is a
//! promise nothing can test." The promises here are FR-018's and FR-019's —
//! that an in-flight attempt is not started twice, that a backoff is waited
//! out, that a submission whose evidence has expired is not delivered without
//! it, and that a busy instance starves nobody. Pulled out, each is one
//! assertion against a database.
//!
//! # One schedule, three jobs
//!
//! Delivery, the retention sweep (`contracts/attachments.md` § 5) and US5's
//! state refresh (research § R14) all run on this tick, bounded per pass.
//! Three schedules would be three things to reason about for one loop the
//! process already holds.
//!
//! # Spawned unconditionally
//!
//! Even with no destination configured, for the reason `main.rs` already gives
//! about the lore sync task: gating the spawn on configuration would mean an
//! operator who configures the feature has to restart to use it, which is a
//! worse trade than a query that finds no rows.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::{DeliveryState, deliver};
use crate::schema::{feedback_delivery_attempts, feedback_submissions};
use crate::state::AppState;

/// How often the task wakes. Lore sync's thirty seconds, and for the same
/// reason: a tick costs one indexed query when there is nothing to do, which
/// is most ticks.
pub const TICK_SECONDS: u64 = 30;

/// The backoff schedule, in seconds, indexed by attempts already made. The
/// same array `lore_sync/schedule.rs` and `mail/schedule.rs` use, deliberately
/// — there should be one retry curve in this product to reason about, not
/// three that drift.
///
/// It ends at an hour rather than growing forever: a submission that has
/// failed nine times is failing in a way retrying will not fix, and hourly is
/// the honest floor — often enough to recover on its own when a host comes
/// back, rare enough not to be noise. An operator has been able to see it
/// since the first failure (FR-021).
pub const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600];

/// When a submission that has been attempted this many times may next be tried.
pub fn next_attempt_after(last_attempt: NaiveDateTime, attempt: i32) -> NaiveDateTime {
    let index = (attempt.max(1) as usize - 1).min(BACKOFF_SECONDS.len() - 1);
    last_attempt + Duration::seconds(BACKOFF_SECONDS[index])
}

/// One submission the task should try now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub submission_id: Uuid,
}

/// How many submissions one tick will attempt, refresh or sweep.
///
/// A bound rather than "everything", so an instance that has been offline for
/// a day does not open a hundred conversations with the host in the same
/// second when it comes back.
const BATCH: i64 = 20;

/// Submissions due for an attempt now, oldest first.
///
/// A submission is due when **all** of: it is `pending`; its latest attempt
/// has finished (or there is none); the backoff has elapsed; and its
/// attachments have not been purged — delivering a submission whose evidence
/// has expired would produce a report without the thing it was for.
///
/// Ordered by `created_at` so a busy instance starves nobody: without an order
/// the database is free to return the same few rows every tick.
pub fn due_now(conn: &mut PgConnection, now: NaiveDateTime) -> Result<Vec<Due>, String> {
    let candidates: Vec<Uuid> = feedback_submissions::table
        .filter(feedback_submissions::delivery_state.eq(DeliveryState::Pending.as_str()))
        .filter(feedback_submissions::attachments_purged_at.is_null())
        .order(feedback_submissions::created_at.asc())
        .select(feedback_submissions::id)
        .load(conn)
        .map_err(|e| format!("Failed to select submissions: {e}"))?;

    let mut due = Vec::new();
    for submission_id in candidates {
        let latest = deliver::latest_attempt(conn, submission_id)?;
        let ready = match latest.started_at {
            // Never attempted. A new submission goes first, which is also what
            // makes the destination's first issue arrive within a tick.
            None => true,
            // An attempt that has not finished is in flight; starting a second
            // would have two processes creating one issue.
            Some(_) if latest.in_flight => false,
            Some(started_at) => now >= next_attempt_after(started_at, latest.attempt),
        };
        if ready {
            due.push(Due { submission_id });
            if due.len() as i64 >= BATCH {
                break;
            }
        }
    }

    Ok(due)
}

/// Delivered submissions whose issue this instance still believes is open,
/// least-recently-checked first.
///
/// Bounded, and only for submissions inside the retention window: a report
/// from four months ago whose issue somebody closes is not worth a request per
/// tick forever.
pub fn state_refresh_due(
    conn: &mut PgConnection,
    now: NaiveDateTime,
) -> Result<Vec<(Uuid, i32)>, String> {
    feedback_submissions::table
        .filter(feedback_submissions::delivery_state.eq(DeliveryState::Delivered.as_str()))
        .filter(feedback_submissions::issue_number.is_not_null())
        .filter(feedback_submissions::issue_state.eq("open"))
        .filter(feedback_submissions::attachments_expire_at.gt(now))
        .order(
            feedback_submissions::issue_state_checked_at
                .asc()
                .nulls_first(),
        )
        .limit(BATCH)
        .select((
            feedback_submissions::id,
            feedback_submissions::issue_number.assume_not_null(),
        ))
        .load(conn)
        .map_err(|e| format!("Failed to select submissions to refresh: {e}"))
}

/// Submissions whose retention has run out and whose bytes are still there.
///
/// Delivery state is deliberately not a condition: a submission delivered on
/// day one and one still pending on day thirty both lose their bytes on day
/// thirty. The pending one's delivery then fails permanently and says so,
/// which is honest and is why the operator's view exists.
pub fn expired_now(conn: &mut PgConnection, now: NaiveDateTime) -> Result<Vec<Uuid>, String> {
    feedback_submissions::table
        .filter(feedback_submissions::attachments_expire_at.le(now))
        .filter(feedback_submissions::attachments_purged_at.is_null())
        .order(feedback_submissions::attachments_expire_at.asc())
        .limit(BATCH)
        .select(feedback_submissions::id)
        .load(conn)
        .map_err(|e| format!("Failed to select expired submissions: {e}"))
}

/// Delete one submission's stored objects and mark the rows.
///
/// The rows outlive their bytes, so a submission can still say what was
/// attached and a person asking where their screenshot went is told it
/// expired rather than finding a blank.
pub async fn purge(state: &AppState, submission_id: Uuid) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let attachments = super::attachments_of(&mut conn, submission_id)?;
    drop(conn);

    let cfg = crate::storage::rustfs::RustFsConfig::from_env();
    for attachment in attachments.iter().filter(|a| a.purged_at.is_none()) {
        // A key outside `feedback/` is refused inside `delete_object` itself,
        // not here — `storage/dedupe.rs` warns that deleting a shared object
        // "would silently blank the background of every other scene sharing
        // those bytes", and a rule kept by callers is a rule until somebody
        // adds a caller.
        if let Err(e) = crate::storage::rustfs::delete_object(&cfg, &attachment.storage_path).await
        {
            return Err(format!("Failed to delete an attachment: {e}"));
        }
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    super::record_purged(&mut conn, submission_id, Utc::now().naive_utc())
}

/// Start the background pass.
///
/// The shape `main.rs` already uses several times — a `spawn_*_task` in the
/// library, called from the binary, owning its own schedule and staying off
/// every hot path.
pub fn spawn_feedback_delivery_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECONDS));
        loop {
            interval.tick().await;
            tick(&state).await;
        }
    });
}

/// One pass. Separate from the loop so a test can run exactly one.
pub async fn tick(state: &AppState) {
    // A tick that cannot read the database is not an error worth shouting
    // about every thirty seconds — the rest of the server is already failing
    // loudly if the pool is gone.
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };
    let now = Utc::now().naive_utc();

    let due = match due_now(&mut conn, now) {
        Ok(due) => due,
        Err(e) => {
            eprintln!("[Feedback] ⚠️  could not select submissions: {e}");
            Vec::new()
        }
    };
    let refresh = state_refresh_due(&mut conn, now).unwrap_or_default();
    let expired = expired_now(&mut conn, now).unwrap_or_default();
    drop(conn);

    for item in due {
        if let Err(e) = deliver::attempt(state, item.submission_id).await {
            // The submission id, never its message and never the person: a log
            // line is one of the surfaces this feature's privacy rules are
            // about.
            eprintln!(
                "[Feedback] ⚠️  attempt on {} could not be recorded: {e}",
                item.submission_id
            );
        }
    }

    refresh_states(state, &refresh).await;

    for submission_id in expired {
        if let Err(e) = purge(state, submission_id).await {
            eprintln!("[Feedback] ⚠️  could not purge {submission_id}: {e}");
        }
    }
}

/// Read back what a maintainer has done with each issue. `open` and `closed`
/// are the only two states stored, because they are the only two the tracker
/// has and inventing a third would be this product guessing at a maintainer's
/// meaning.
async fn refresh_states(state: &AppState, submissions: &[(Uuid, i32)]) {
    if submissions.is_empty() {
        return;
    }
    let Ok(settings) = crate::settings::resolver::resolve_all(state).await else {
        return;
    };
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };
    let destination = super::destination(&mut conn).ok().flatten();
    let Ok(host) = state.feedback.host(&settings, destination.as_ref()) else {
        return;
    };

    for (submission_id, number) in submissions {
        if let Ok(issue_state) = host.state_of(*number).await
            && let Err(e) = super::record_issue_state(
                &mut conn,
                *submission_id,
                &issue_state,
                Utc::now().naive_utc(),
            )
        {
            eprintln!("[Feedback] ⚠️  could not record an issue state: {e}");
        }
    }
}

/// Update the destination's observed visibility.
///
/// Called from the pass rather than from the notice query, so the answer a
/// person is shown before they submit is one this instance already had rather
/// than a round trip in the middle of opening a form.
pub async fn refresh_visibility(state: &AppState) -> Result<(), String> {
    let settings = crate::settings::resolver::resolve_all(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let Some(destination) = super::destination(&mut conn)? else {
        return Ok(());
    };
    let host = state
        .feedback
        .host(&settings, Some(&destination))
        .map_err(|_| "No feedback application is configured.".to_string())?;
    let is_public = host
        .is_public()
        .await
        .map_err(|_| "The destination's visibility could not be read.".to_string())?;
    super::record_visibility(&mut conn, destination.id, is_public, Utc::now().naive_utc())
}

/// Attempts on one submission, newest first — what the operator's view reads
/// its reason from.
pub fn attempts_of(
    conn: &mut PgConnection,
    submission_id: Uuid,
) -> Result<Vec<(i32, NaiveDateTime, Option<String>)>, String> {
    feedback_delivery_attempts::table
        .filter(feedback_delivery_attempts::submission_id.eq(submission_id))
        .order(feedback_delivery_attempts::started_at.desc())
        .select((
            feedback_delivery_attempts::attempt,
            feedback_delivery_attempts::started_at,
            feedback_delivery_attempts::failure_reason,
        ))
        .load(conn)
        .map_err(|e| format!("Failed to read the delivery attempts: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> NaiveDateTime {
        chrono::DateTime::from_timestamp(seconds, 0)
            .expect("a valid timestamp")
            .naive_utc()
    }

    /// The curve is the one the product already has, and it is indexed by
    /// attempts already made rather than attempts remaining — an off-by-one
    /// here would either retry immediately forever or skip the first wait.
    #[test]
    fn the_backoff_curve_is_the_one_the_product_already_has() {
        assert_eq!(
            BACKOFF_SECONDS,
            [30, 60, 120, 300, 600, 900, 1800, 2700, 3600]
        );
        assert_eq!(next_attempt_after(at(0), 1), at(30));
        assert_eq!(next_attempt_after(at(0), 2), at(60));
        assert_eq!(next_attempt_after(at(0), 9), at(3600));
    }

    /// It ends rather than growing without bound, and a nonsensical attempt
    /// count is clamped rather than panicking on an index.
    #[test]
    fn the_curve_ends_at_an_hour_and_survives_a_nonsense_attempt_count() {
        assert_eq!(next_attempt_after(at(0), 50), at(3600));
        assert_eq!(next_attempt_after(at(0), 0), at(30));
        assert_eq!(next_attempt_after(at(0), -3), at(30));
    }

    /// The first step is thirty seconds and not immediate, and that is a
    /// decision rather than a default: the host's search index is not
    /// immediate either, so an attempt retried in the same second after an
    /// ambiguous failure would miss its own issue and create a second one.
    #[test]
    fn the_first_retry_waits_long_enough_for_the_hosts_index() {
        assert_eq!(BACKOFF_SECONDS[0], 30);
    }
}
