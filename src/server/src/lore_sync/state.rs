//! A connection's state, and the only transitions that produce one.
//!
//! # Why the transitions live here rather than at each call site
//!
//! Because they were scattered, and scattered transitions are how a state
//! machine stops being one. A pass set `working`, a failure set
//! `needs_attention`, an enforcement action set `deactivated`, and each did it
//! with its own `diesel::update` — so "which states can follow which" was not
//! written anywhere and could only be reconstructed by reading every writer.
//!
//! The rule that matters is not obvious from any single call site, which is
//! exactly why it needs a home: **`deactivated` is a one-way door for
//! everyone except an administrator** (FR-041a, FR-041c). A pass that
//! succeeded must not lift it, a retry must not lift it, and a Game Master
//! resolving a divergence must not lift it. Enforced in one place, that is one
//! `if`. Enforced at four call sites, it is four chances to forget.
//!
//! # `needs_attention` always carries a reason
//!
//! FR-029 says a state must name the remedy, and the type makes that
//! unavoidable: the transition to `needs_attention` takes the reason as an
//! argument, so there is no way to enter that state silently. A state that
//! says something is wrong without saying what sends a Game Master to a
//! support channel.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::lore_repository_connections as c;

/// The four states, in FR-029's own words plus FR-041c's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    NeverConfigured,
    Working,
    NeedsAttention,
    Deactivated,
}

impl State {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::NeverConfigured => "never_configured",
            Self::Working => "working",
            Self::NeedsAttention => "needs_attention",
            Self::Deactivated => "deactivated",
        }
    }

    /// An unrecognised stored value resolves towards attention, never towards
    /// `working`. A build that does not understand what it read must not
    /// report a connection as healthy on the strength of not understanding it.
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "never_configured" => Self::NeverConfigured,
            "working" => Self::Working,
            "deactivated" => Self::Deactivated,
            _ => Self::NeedsAttention,
        }
    }

    /// Whether a pass may run for a connection in this state.
    pub fn admits_a_pass(self) -> bool {
        matches!(self, Self::Working | Self::NeedsAttention)
    }
}

/// Why a transition was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// An enforcement deactivation is not lifted by anything a pass or an
    /// owner can do (FR-041a).
    Deactivated,
    Database(String),
}

/// Record that a pass succeeded.
///
/// Refuses to lift a deactivation. This is the transition most likely to be
/// wrong: a pass that somehow ran against a deactivated connection and then
/// marked it healthy would undo an enforcement action through the back door,
/// and it would look like ordinary success in the log.
pub fn to_working(conn: &mut PgConnection, world_id: Uuid) -> Result<(), Refused> {
    guard_not_deactivated(conn, world_id)?;
    set(conn, world_id, State::Working, None)
}

/// Record that a pass failed, and why in words a Game Master can act on.
///
/// The reason is a parameter rather than optional, so FR-029's "names the
/// remedy" cannot be satisfied by an empty string that compiled.
pub fn to_needs_attention(
    conn: &mut PgConnection,
    world_id: Uuid,
    remedy: &str,
) -> Result<(), Refused> {
    guard_not_deactivated(conn, world_id)?;
    set(conn, world_id, State::NeedsAttention, Some(remedy))
}

/// The current state, or `None` where there is no connection.
pub fn current(conn: &mut PgConnection, world_id: Uuid) -> Result<Option<State>, Refused> {
    c::table
        .filter(c::world_id.eq(world_id))
        .select(c::state)
        .first::<String>(conn)
        .optional()
        .map(|v| v.map(|s| State::from_db_str(&s)))
        .map_err(|e| Refused::Database(e.to_string()))
}

fn guard_not_deactivated(conn: &mut PgConnection, world_id: Uuid) -> Result<(), Refused> {
    match current(conn, world_id)? {
        Some(State::Deactivated) => Err(Refused::Deactivated),
        _ => Ok(()),
    }
}

fn set(
    conn: &mut PgConnection,
    world_id: Uuid,
    state: State,
    reason: Option<&str>,
) -> Result<(), Refused> {
    diesel::update(c::table.filter(c::world_id.eq(world_id)))
        .set((
            c::state.eq(state.as_db_str()),
            c::state_reason.eq(reason.map(str::to_string)),
            c::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(conn)
        .map(|_| ())
        .map_err(|e| Refused::Database(e.to_string()))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
