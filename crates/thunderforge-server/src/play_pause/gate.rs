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
use crate::state::AppState;

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

/// A refusal carried out of a closure whose error type is diesel's.
///
/// Most resolvers authorise inside a `spawn_blocking` closure that returns
/// `QueryResult`, and the entity lookup that says which world a wall or a
/// token is in happens there too. Converting a [`GateError`] into a diesel
/// error lets the gate be one `?` beside that lookup, and aborts a surrounding
/// transaction as any other error would. [`refusal_or`] takes it back out on
/// the far side, where the resolver would otherwise have flattened every
/// error into one message and lost the code.
#[derive(Debug)]
pub struct CarriedRefusal(pub GateError);

impl std::fmt::Display for CarriedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("refused by the play-pause gate")
    }
}

impl std::error::Error for CarriedRefusal {}

impl From<GateError> for diesel::result::Error {
    fn from(e: GateError) -> Self {
        diesel::result::Error::QueryBuilderError(Box::new(CarriedRefusal(e)))
    }
}

/// The gate's refusal, if `e` carries one.
pub fn carried(e: &diesel::result::Error) -> Option<GateError> {
    match e {
        diesel::result::Error::QueryBuilderError(inner) => inner
            .downcast_ref::<CarriedRefusal>()
            .map(|carried| carried.0.clone()),
        _ => None,
    }
}

/// The gate's refusal if `e` carries one, and `message` otherwise: the
/// `map_err` a resolver that flattens its errors uses instead of `|_|`.
pub fn refusal_or(e: diesel::result::Error, message: &str) -> Error {
    match carried(&e) {
        Some(refusal) => refusal.into(),
        None => Error::new(message),
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

/// [`refuse_if_paused`], for async code that holds a world id but no
/// connection. Takes one from the pool; a pool that gives none refuses, as an
/// unreadable answer does.
pub async fn refuse_world_if_paused(state: &AppState, world_id: Uuid) -> Result<(), GateError> {
    let Ok(mut conn) = state.db_pool.get() else {
        return Err(GateError::Unreadable("no database connection".into()));
    };
    tokio::task::spawn_blocking(move || refuse_if_paused(&mut conn, world_id))
        .await
        .map_err(|e| GateError::Unreadable(e.to_string()))?
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
