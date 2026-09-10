//! What the counting costs, rung by rung.
//!
//! Spec 039 US5, `contracts/standing-and-termination.md`, data-model.md § 5.
//!
//! # Derived, never stored
//!
//! Strikes, whether an account may publish, and whether it is disabled are
//! computed on every read from `content_moderation_actions` through
//! [`super::strikes_of`] — the same definition `repeatInfringerFlags` uses, so
//! the ladder and the flag cannot disagree about what a strike is (FR-027).
//! Nothing has to notice a strike ageing past the lookback: the calendar moved,
//! the count changed, and publishing comes back without anybody asking
//! (FR-035). Restoration follows the existing process — a case resolved in the
//! person's favour stops counting — rather than a database edit (FR-020).
//!
//! # The ladder is a value, not an ambient read
//!
//! [`Ladder::from_env`] reads the settings once; everything below takes a
//! [`Ladder`]. A test states the ladder it means rather than depending on what
//! the process environment happens to hold, which is the shared-global-state
//! trap this crate has already paid for twice.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::Strike;
use crate::notices;
use crate::state::AppState;

/// Config: the strike at which the person is warned. A notice; publishing is
/// unaffected.
pub fn strike_warn_at() -> i64 {
    std::env::var("MODERATION_STRIKE_WARN_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Config: the strike at which publishing is suspended (FR-018). Play, edit and
/// read are untouched (FR-019).
pub fn strike_suspend_publishing_at() -> i64 {
    std::env::var("MODERATION_STRIKE_SUSPEND_PUBLISHING_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
}

/// Config: days between disablement and deletion (FR-030). Read when a
/// termination opens (US7), and snapshotted there.
pub fn termination_window_days() -> i64 {
    std::env::var("MODERATION_TERMINATION_WINDOW_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

/// Config: whether the end of the window waits for a person rather than a
/// timer. **Defaults to true** — a self-hosted table of six friends should not
/// have a timer that deletes one of them.
pub fn termination_requires_human() -> bool {
    std::env::var("MODERATION_TERMINATION_REQUIRES_HUMAN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true)
}

/// The rungs in force, read once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ladder {
    pub warn_at: i64,
    pub suspend_publishing_at: i64,
    /// The existing `MODERATION_REPEAT_INFRINGER_THRESHOLD`: disablement.
    pub threshold: i64,
    pub lookback_days: i64,
}

impl Ladder {
    pub fn from_env() -> Self {
        Self {
            warn_at: strike_warn_at(),
            suspend_publishing_at: strike_suspend_publishing_at(),
            threshold: super::repeat_infringer_threshold(),
            lookback_days: super::repeat_infringer_lookback_days(),
        }
    }

    /// When a strike recorded at `recorded_at` stops counting — exactly when
    /// the existing lookback stops including it.
    pub fn ages_out_at(&self, recorded_at: DateTime<Utc>) -> DateTime<Utc> {
        recorded_at + chrono::Duration::days(self.lookback_days)
    }
}

/// Where one account stands.
#[derive(Debug, Clone)]
pub struct Standing {
    /// Oldest first.
    pub strikes: Vec<Strike>,
    pub ladder: Ladder,
    pub may_publish: bool,
    /// An open termination exists. Always false until US7 creates
    /// `account_terminations`; the field is here so nothing that reads a
    /// standing has to change when it does.
    pub disabled: bool,
}

impl Standing {
    /// The whole of the ladder's logic, with no database in it.
    ///
    /// Publishing stops at the suspension rung **or** the threshold, whichever
    /// comes first: an operator who sets the suspension rung above the
    /// threshold has not thereby let a disabled account publish.
    pub fn from_strikes(ladder: Ladder, strikes: Vec<Strike>) -> Self {
        let count = strikes.len() as i64;
        Self {
            may_publish: count < ladder.suspend_publishing_at && count < ladder.threshold,
            disabled: false,
            strikes,
            ladder,
        }
    }

    pub fn strike_count(&self) -> i64 {
        self.strikes.len() as i64
    }

    /// At or past the warning rung.
    pub fn warned(&self) -> bool {
        self.strike_count() >= self.ladder.warn_at
    }
}

pub fn standing_of_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    ladder: Ladder,
) -> QueryResult<Standing> {
    let strikes = super::strikes_of(conn, account_id, ladder.lookback_days)?;
    Ok(Standing::from_strikes(ladder, strikes))
}

/// One account's standing under the ladder in force.
pub async fn standing_of(state: &AppState, account_id: Uuid) -> Result<Standing, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let ladder = Ladder::from_env();

    tokio::task::spawn_blocking(move || standing_of_sync(&mut conn, account_id, ladder))
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to read the account's standing".to_string())
}

/// FR-028: a case against `account_id` has just become upheld — tell them.
///
/// Called on the connection that wrote the moderation event, straight after
/// it, and only when that event made the case count — whether it is news is
/// the caller's to know, because only the caller saw the case before. Writes
/// nothing if the case turns out not to count after all.
///
/// The notice says what the strike was for, that it counts, how many remain
/// and when it stops counting. The strike that reaches the suspension rung
/// also gets a second notice saying so — that is the one somebody needs to
/// have read before they next try to share.
pub fn tell_of_strike_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    case_id: Uuid,
    ladder: Ladder,
) -> QueryResult<()> {
    let standing = standing_of_sync(conn, account_id, ladder)?;
    let Some(strike) = standing.strikes.iter().find(|s| s.case_id == case_id) else {
        return Ok(());
    };
    let count = standing.strike_count();
    let subject = json!({
        "caseId": strike.case_id,
        "entityType": strike.entity_type,
        "entityId": strike.entity_id,
        "worldId": strike.world_id,
    });

    notices::record_sync(
        conn,
        account_id,
        notices::kind::STRIKE_RECORDED,
        Some(subject.clone()),
        Some(json!({
            "strikeCount": count,
            "suspendPublishingAt": ladder.suspend_publishing_at,
            "threshold": ladder.threshold,
            "agesOutAt": ladder.ages_out_at(strike.recorded_at).to_rfc3339(),
        })),
    )?;

    if count == ladder.suspend_publishing_at && count < ladder.threshold {
        notices::record_sync(
            conn,
            account_id,
            notices::kind::PUBLISHING_SUSPENDED,
            Some(subject),
            Some(json!({
                "strikeCount": count,
                "threshold": ladder.threshold,
            })),
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "standing_tests.rs"]
mod standing_tests;
