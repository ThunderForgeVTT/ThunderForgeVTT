//! The gate: whether a world-scoped call may go ahead (contracts/live-play-lock.md).
//!
//! # Blind to `is_admin`, by being blind to the caller
//!
//! Neither function takes a user. That is the whole of ADR-100 decision 2 as
//! far as this file is concerned: site admins short-circuit `actor_in_world`
//! and `is_dm_*`, and a gate that took `is_admin` would be one careless `if`
//! away from the same short-circuit. A resolver calls it **beside** its role
//! check, never inside one, and an operator in their own paused world is
//! refused like anyone else.
//!
//! # Fails closed
//!
//! A call that cannot confirm the world is not paused is refused. Every caller
//! is about to read or write the same database, so an unreadable answer here
//! costs a call that was going to fail anyway, and the alternative is a pause
//! that holds only while Postgres is healthy. The refusal is
//! [`GateError::Unreadable`], not [`PlayPaused`]: a database blip must not
//! send a table to a notice saying an operator stopped it.
//!
//! The stream poll in `graphql::session_lifetime` fails the other way, and says
//! why.

use async_graphql::{Error, ErrorExtensions as _};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{scenes, world_play_pauses};

/// The extension code every refusal because a world is paused carries.
pub const WORLD_PLAY_PAUSED: &str = "WORLD_PLAY_PAUSED";

/// This world's play is paused.
///
/// Carries *that and when*, and nothing else. There is deliberately no field
/// for the grounds, the operator or what led to it, so none can be added to a
/// message by accident (FR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayPaused {
    pub world_id: Uuid,
    pub paused_at: NaiveDateTime,
}

impl From<PlayPaused> for Error {
    fn from(paused: PlayPaused) -> Self {
        Error::new("Play in this world has been paused by an operator.").extend_with(|_, ext| {
            ext.set("code", WORLD_PLAY_PAUSED);
            ext.set("worldId", paused.world_id.to_string());
            ext.set("pausedAt", paused.paused_at.and_utc().to_rfc3339());
        })
    }
}

/// Why the gate refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    Paused(PlayPaused),
    /// Whether the world is paused could not be read. Refused all the same.
    Unreadable(String),
}

impl From<PlayPaused> for GateError {
    fn from(paused: PlayPaused) -> Self {
        GateError::Paused(paused)
    }
}

impl From<GateError> for Error {
    fn from(e: GateError) -> Self {
        match e {
            GateError::Paused(paused) => paused.into(),
            GateError::Unreadable(detail) => {
                eprintln!("[play_pause] ⚠️  could not read whether a world is paused: {detail}");
                Error::new("Could not confirm this world is open for play. Try again shortly.")
            }
        }
    }
}

/// Refuse if `world_id`'s play is paused.
///
/// Reads the active pause's partial unique index and nothing else.
pub fn refuse_if_paused(conn: &mut PgConnection, world_id: Uuid) -> Result<(), GateError> {
    match active_pause_at(conn, world_id) {
        Ok(None) => Ok(()),
        Ok(Some(paused_at)) => Err(PlayPaused {
            world_id,
            paused_at,
        }
        .into()),
        Err(e) => Err(GateError::Unreadable(e.to_string())),
    }
}

/// [`refuse_if_paused`], for calls that carry a scene rather than a world.
///
/// A scene that does not exist belongs to no world that could be paused, and
/// passes: the resolver's own lookup is where "no such scene" is said.
pub fn refuse_scene_if_paused(conn: &mut PgConnection, scene_id: Uuid) -> Result<(), GateError> {
    let world_id = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(|e| GateError::Unreadable(e.to_string()))?;
    match world_id {
        Some(world_id) => refuse_if_paused(conn, world_id),
        None => Ok(()),
    }
}

/// When the world's active pause began, if it has one.
pub(crate) fn active_pause_at(
    conn: &mut PgConnection,
    world_id: Uuid,
) -> QueryResult<Option<NaiveDateTime>> {
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world_id))
        .filter(world_play_pauses::lifted_at.is_null())
        .select(world_play_pauses::paused_at)
        .first(conn)
        .optional()
}
