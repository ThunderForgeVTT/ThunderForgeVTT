//! Which messages a tick should try, how often, and how long it waits after a
//! refusal.
//!
//! # Why the selection is its own function
//!
//! `lore_sync/schedule.rs` states the reason and it applies here verbatim: "a
//! promise enforced by an `if` inside a loop inside a spawned task is a
//! promise nothing can test." The promises here are FR-015's — that a message
//! is never sent twice, that a message waiting on the backoff curve is left
//! alone, and that a `blocked` message is not attempted against a mail server
//! that does not exist. Pulled out, each of those is one assertion against a
//! database instead of an argument about a loop.
//!
//! # Why the same backoff array as lore sync
//!
//! Because there should be one retry curve in this product to reason about,
//! not two that drift. It ends rather than growing forever for the same reason
//! that file gives: a message refused nine times over about two hours is being
//! refused for a reason retrying will not fix, and the operator surface has
//! had it in front of them the whole time.
//!
//! # The three states a tick can leave a message in
//!
//! `sent`, `queued` with a later `next_attempt_at`, or `failed`. There is no
//! fourth, and in particular a tick never deletes a row: an operator's answer
//! to "what happened to the message" must not be "there is no record".

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::outbox::{self, OutboxState};
use crate::schema::mail_outbox;
use crate::state::AppState;

/// How often the task wakes. Half of lore sync's thirty seconds, because a
/// person pressing "send a test message" is watching, and because a tick costs
/// one indexed query when the outbox is empty, which it usually is.
pub const TICK_SECONDS: u64 = 15;

/// The backoff schedule, in seconds, indexed by attempts already made. The
/// same array `lore_sync/schedule.rs` uses, deliberately.
const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600];

/// How many attempts the curve describes. `outbox::MAX_ATTEMPTS` is this, so
/// the number of tries and the length of the curve cannot disagree.
pub const BACKOFF_LENGTH: usize = BACKOFF_SECONDS.len();

/// When a message that has failed this many times may next be tried.
pub fn next_attempt_after(last_attempt: NaiveDateTime, attempts: i32) -> NaiveDateTime {
    let index = (attempts.max(1) as usize - 1).min(BACKOFF_SECONDS.len() - 1);
    last_attempt + Duration::seconds(BACKOFF_SECONDS[index])
}

/// One message the task should try now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub id: Uuid,
}

/// Messages due for an attempt now, oldest first.
///
/// Ordered by when they were written so a busy instance starves nobody:
/// without an order the database is free to return the same few rows every
/// tick.
///
/// `sending` is excluded, which is the single-flight guard — a row is claimed
/// by moving it to `sending` in one conditional statement, so a second sender
/// finds nothing to claim. `blocked` is excluded because there is nowhere to
/// send it; [`outbox::release_blocked`] is what moves those back, and it runs
/// on the tick where mail became available rather than on an edit.
pub fn due_now(conn: &mut PgConnection, now: NaiveDateTime) -> Result<Vec<Due>, String> {
    let ids: Vec<Uuid> = mail_outbox::table
        .filter(mail_outbox::state.eq(OutboxState::Queued.as_str()))
        .filter(
            mail_outbox::next_attempt_at
                .is_null()
                .or(mail_outbox::next_attempt_at.le(now)),
        )
        .order(mail_outbox::created_at.asc())
        .limit(BATCH)
        .select(mail_outbox::id)
        .load(conn)
        .map_err(|e| format!("Failed to select messages: {e}"))?;

    Ok(ids.into_iter().map(|id| Due { id }).collect())
}

/// How many messages one tick will attempt. A bound rather than "everything",
/// so an instance that has been offline for a day does not open a hundred SMTP
/// conversations in the same second when it comes back.
const BATCH: i64 = 20;

/// Try one message, whatever its outcome, and record what happened.
///
/// Returns the message's state afterwards. Public because `sendTestMail` calls
/// it directly: FR-013 wants an operator to press a button and be told the
/// outcome, and waiting up to fifteen seconds for a tick would be a worse
/// answer to the same question. It is the *same* path either way — the code
/// under test is the code in production (contracts/mail.md rule 2).
pub async fn attempt(state: &AppState, id: Uuid) -> Result<OutboxState, String> {
    let settings = crate::settings::resolver::resolve_all(state).await?;
    let transport = state.mail.transport(&settings);
    let availability = transport.availability();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    if !availability.is_ready() {
        outbox::block(&mut conn, id, &availability)?;
        return Ok(OutboxState::Blocked);
    }

    let now = Utc::now().naive_utc();
    if !outbox::mark_sending(&mut conn, id, now)? {
        // Somebody else has it, or it is not in a state that may be sent. Not
        // an error: the honest answer is whatever the row says now.
        let row = outbox::load(&mut conn, id)?
            .ok_or_else(|| "That message is no longer in the outbox.".to_string())?;
        return Ok(row.state().unwrap_or(OutboxState::Failed));
    }

    let row = outbox::load(&mut conn, id)?
        .ok_or_else(|| "That message is no longer in the outbox.".to_string())?;
    let message = match row.decrypt(state) {
        Ok(message) => message,
        // The stored ciphertext will not read back — `THUNDERFORGE_SECRET` was
        // rotated. Nothing sends and nothing crashes; the row says so in prose
        // an operator can act on, which is the edge case contracts/mail.md
        // lists and the behaviour `settings::resolver` already takes for a
        // secret it cannot decrypt.
        Err(_) => {
            let failure = super::DeliveryFailure::permanent(
                "The stored copy of this message could not be read back, which \
                 happens when the instance secret has been rotated. It cannot \
                 be sent; whatever asked for it will need to ask again.",
            );
            outbox::record_failure(&mut conn, &row, &failure, now)?;
            return Ok(OutboxState::Failed);
        }
    };

    match transport.send(&message).await {
        Ok(()) => {
            outbox::record_success(&mut conn, id, Utc::now().naive_utc())?;
            Ok(OutboxState::Sent)
        }
        Err(failure) => {
            let outcome = outbox::record_failure(&mut conn, &row, &failure, now)?;
            Ok(outcome.state)
        }
    }
}

/// Start the background sender.
///
/// The shape `main.rs` already uses five times — a `spawn_*_task` in the
/// library, called from the binary, owning its own schedule and staying off
/// every hot path. No new infrastructure, deliberately: `lore_sync` rejected a
/// broker for this exact loop, and a self-hosted VTT should not gain one to
/// send a password reset.
pub fn spawn_mail_task(state: AppState) {
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
    let Ok(settings) = crate::settings::resolver::resolve_all(state).await else {
        return;
    };
    let available = state.mail.transport(&settings).availability().is_ready();

    // A tick that cannot read the database is not an error worth shouting
    // about every fifteen seconds — the rest of the server is already failing
    // loudly if the pool is gone.
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };

    if available {
        match outbox::release_blocked(&mut conn) {
            Ok(0) => {}
            Ok(released) => {
                eprintln!("[Mail] mail is configured; {released} held message(s) released")
            }
            Err(e) => eprintln!("[Mail] ⚠️  could not release held messages: {e}"),
        }
    }

    let now = Utc::now().naive_utc();
    let due = match due_now(&mut conn, now) {
        Ok(due) => due,
        Err(e) => {
            eprintln!("[Mail] ⚠️  could not select messages: {e}");
            return;
        }
    };
    drop(conn);

    for message in due {
        if let Err(e) = attempt(state, message.id).await {
            // The id, never the recipient and never the subject: a log line is
            // one of the surfaces FR-016 is about.
            eprintln!(
                "[Mail] ⚠️  attempt on {} could not be recorded: {e}",
                message.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> NaiveDateTime {
        chrono::DateTime::from_timestamp(seconds, 0)
            .expect("a valid timestamp")
            .naive_utc()
    }

    /// The curve is the one lore sync uses, and it is indexed by attempts
    /// already made rather than by attempts remaining — an off-by-one here
    /// would either retry immediately forever or skip the first wait.
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

    /// It terminates rather than growing without bound, and a nonsensical
    /// attempt count is clamped rather than panicking on an index.
    #[test]
    fn the_curve_ends_at_an_hour_and_survives_a_nonsense_attempt_count() {
        assert_eq!(next_attempt_after(at(0), 50), at(3600));
        assert_eq!(next_attempt_after(at(0), 0), at(30));
        assert_eq!(next_attempt_after(at(0), -3), at(30));
    }

    /// The curve's length and the number of attempts allowed are one number.
    /// Two would drift, and the drift is invisible until a message that should
    /// have been given up on is retried at an index that does not exist.
    #[test]
    fn the_attempt_limit_is_the_length_of_the_curve() {
        assert_eq!(BACKOFF_LENGTH, BACKOFF_SECONDS.len());
        assert_eq!(outbox::MAX_ATTEMPTS as usize, BACKOFF_SECONDS.len());
    }
}
