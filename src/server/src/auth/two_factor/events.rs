//! What happened to a second factor, and who did it (FR-015, FR-025).
//!
//! # What a row says, and what it must never say
//!
//! The act, never the person. No address, no user agent, no email, no code and
//! no fragment of one — the same rule spec 035 wrote into `instance_access_events`,
//! for the same reason. A `recovery_code_used` row records that a code was
//! used; it never records which, because knowing which one would make the log
//! itself worth stealing.
//!
//! # Why two user columns
//!
//! FR-025 asks who did it *and* for whom, and one column cannot answer both.
//! For a self-service enrolment they are the same account. For an operator
//! reset they are not, and that is the row somebody will eventually need to
//! read. `actor` is nullable and `ON DELETE SET NULL`, so an operator who
//! later leaves does not take the record of their reset with them.
//!
//! # Written in the transaction it describes
//!
//! Every caller records inside the transaction that performed the act, which
//! is why [`record_sync`] is the only writer. A removal that was not recorded
//! did not happen, and a factor cleared whose row rolled back would be a
//! security history with a hole in exactly the place somebody would look.
//!
//! An async best-effort form existed here briefly, for callers whose work is
//! already committed by the time they know the outcome. Nothing needed it —
//! every path this log cares about is holding a transaction when it decides —
//! so it went, rather than sit here as a second way to do this that nobody
//! maintains.

use diesel::prelude::*;

use crate::schema::two_factor_events;
use crate::state::AppState;

/// The vocabulary, mirrored by a CHECK constraint in the spec-041 migration.
///
/// Constants rather than strings at the call sites: a value added here without
/// adding it to the constraint fails loudly at the insert, and a typo cannot
/// silently create an eighth kind of event that nothing queries.
pub(crate) mod event_type {
    pub(crate) const ENROLLED: &str = "enrolled";
    pub(crate) const REMOVED: &str = "removed";
    pub(crate) const RECOVERY_CODE_USED: &str = "recovery_code_used";
    pub(crate) const RECOVERY_CODES_ISSUED: &str = "recovery_codes_issued";
    /// Written by the operator-reset path, which is the one case where
    /// `actor` and `subject` differ and the whole reason they are two columns.
    #[allow(dead_code)]
    pub(crate) const RESET_BY_OPERATOR: &str = "reset_by_operator";
    pub(crate) const REQUIREMENT_SET: &str = "requirement_set";
    pub(crate) const REQUIREMENT_CLEARED: &str = "requirement_cleared";
}

/// Record an event on a connection the caller already holds.
///
/// The form to prefer: inside the transaction that performed the act, so a
/// removal that was not recorded did not happen.
pub(crate) fn record_sync(
    conn: &mut PgConnection,
    subject_user_id: uuid::Uuid,
    actor_user_id: Option<uuid::Uuid>,
    event_type: &str,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(two_factor_events::table)
        .values((
            two_factor_events::id.eq(uuid::Uuid::now_v7()),
            two_factor_events::subject_user_id.eq(subject_user_id),
            two_factor_events::actor_user_id.eq(actor_user_id),
            two_factor_events::event_type.eq(event_type),
        ))
        .execute(conn)?;
    Ok(())
}

/// One event, as its subject reads it in their own security settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TwoFactorEvent {
    pub(crate) occurred_at: chrono::NaiveDateTime,
    pub(crate) event_type: String,
    /// True when somebody other than the account holder did this — which is
    /// the single fact a person most needs from this list.
    pub(crate) by_someone_else: bool,
}

/// This account's own second-factor history, most recent first.
///
/// Scoped to `subject_user_id`, so one account cannot read another's. The
/// acting account's *identity* is deliberately not returned: "an administrator
/// did this" is what the account holder needs and can act on; which
/// administrator is an operator's question, asked of an operator's surface.
pub(crate) async fn events_for(
    state: &AppState,
    subject_user_id: uuid::Uuid,
    limit: i64,
) -> Result<Vec<TwoFactorEvent>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let rows = tokio::task::spawn_blocking(move || {
        two_factor_events::table
            .filter(two_factor_events::subject_user_id.eq(subject_user_id))
            .order(two_factor_events::occurred_at.desc())
            .limit(limit)
            .select((
                two_factor_events::occurred_at,
                two_factor_events::event_type,
                two_factor_events::actor_user_id,
            ))
            .load::<(chrono::NaiveDateTime, String, Option<uuid::Uuid>)>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to read the second-factor history".to_string())?;

    Ok(rows
        .into_iter()
        .map(|(occurred_at, event_type, actor)| TwoFactorEvent {
            occurred_at,
            event_type,
            // An absent actor is an operator whose account has since been
            // deleted (`ON DELETE SET NULL`), which is still not the subject.
            by_someone_else: actor != Some(subject_user_id),
        })
        .collect())
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod events_tests;
