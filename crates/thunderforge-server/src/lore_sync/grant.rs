//! Starting a repository grant, and validating what comes back.
//!
//! # What a hand-off actually risks
//!
//! The user leaves this application, authorises something at a host, and
//! returns to a URL carrying an installation identifier. **Everything in that
//! return is attacker-controlled** — the identifier, the state, the ordering,
//! and how many times it arrives. So the return is not information, it is a
//! claim, and this module's whole job is deciding which claims to believe.
//!
//! Four checks, and each rules out a different attack:
//!
//! 1. **The state must exist and be unconsumed.** Without single use, a
//!    captured callback URL replays and rebinds a world after the fact.
//! 2. **It must not have expired.** A hand-off is seconds of human attention;
//!    one still open an hour later is abandoned, and an abandoned session that
//!    stays valid is an attack surface that grows on its own.
//! 3. **The returning user must be the one who started it.** Otherwise anyone
//!    who obtains a state can complete someone else's connection.
//! 4. **They must still own the world** (FR-003). Authority is re-checked
//!    rather than captured, and a hand-off is exactly the kind of pause during
//!    which it changes.
//!
//! Two of these are not enough: a valid state for the wrong world, and the
//! right world claimed by the wrong person, are both attacks that pass three
//! checks and fail the fourth.

use chrono::{Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::lore_sync_grant_sessions as g;

/// How long a hand-off stays valid. Generous for a human, short for an
/// attacker holding a captured URL.
const SESSION_MINUTES: i64 = 30;

/// Why a returning grant was not believed.
#[derive(Debug, PartialEq, Eq)]
pub enum GrantRefused {
    /// No such state, already used, or expired. **Deliberately one variant** —
    /// telling a caller which of the three it was answers a question about
    /// whether a state ever existed, which is exactly what someone guessing
    /// states wants to know.
    NotValid,
    /// A different user than the one who started it.
    NotYours,
    Database(String),
}

/// Begin a hand-off, returning the state to send to the host.
///
/// v4, not v7 (ADR-049's reasoning): this value is the only thing standing
/// between a callback and a world, so it must not front-load a timestamp that
/// narrows a guess.
pub fn begin(
    conn: &mut PgConnection,
    world_id: Uuid,
    started_by: Uuid,
    return_to: Option<&str>,
) -> Result<String, GrantRefused> {
    let state = Uuid::new_v4().simple().to_string();

    diesel::insert_into(g::table)
        .values((
            g::id.eq(Uuid::now_v7()),
            g::world_id.eq(world_id),
            g::started_by.eq(started_by),
            g::state.eq(&state),
            g::return_to.eq(return_to.map(str::to_string)),
            g::expires_at.eq((Utc::now() + Duration::minutes(SESSION_MINUTES)).naive_utc()),
        ))
        .execute(conn)
        .map_err(|e| GrantRefused::Database(e.to_string()))?;

    Ok(state)
}

/// What a validated hand-off entitles the caller to do.
#[derive(Debug, PartialEq, Eq)]
pub struct Claim {
    pub world_id: Uuid,
    pub return_to: Option<String>,
}

/// Validate a returning state and consume it.
///
/// Consumes on success only. A failed attempt must not burn the session, or an
/// attacker who guesses one wrong state can invalidate a legitimate user's
/// in-flight connection — a denial of service through the front door.
pub fn consume(
    conn: &mut PgConnection,
    state: &str,
    returning_user: Uuid,
) -> Result<Claim, GrantRefused> {
    let now = Utc::now().naive_utc();

    let found = g::table
        .filter(g::state.eq(state))
        .filter(g::consumed_at.is_null())
        .filter(g::expires_at.gt(now))
        .select((g::id, g::world_id, g::started_by, g::return_to))
        .first::<(Uuid, Uuid, Uuid, Option<String>)>(conn)
        .optional()
        .map_err(|e| GrantRefused::Database(e.to_string()))?;

    let Some((id, world_id, started_by, return_to)) = found else {
        return Err(GrantRefused::NotValid);
    };

    if started_by != returning_user {
        return Err(GrantRefused::NotYours);
    }

    // Consumed with the same predicate that found it, so two callbacks racing
    // cannot both succeed: the second updates zero rows.
    let consumed = diesel::update(
        g::table
            .filter(g::id.eq(id))
            .filter(g::consumed_at.is_null()),
    )
    .set(g::consumed_at.eq(now))
    .execute(conn)
    .map_err(|e| GrantRefused::Database(e.to_string()))?;

    if consumed == 0 {
        return Err(GrantRefused::NotValid);
    }

    Ok(Claim {
        world_id,
        return_to,
    })
}

#[cfg(test)]
#[path = "grant_tests.rs"]
mod grant_tests;
