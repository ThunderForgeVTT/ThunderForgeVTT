//! Shared PostgreSQL LISTEN/NOTIFY backplane helper for real-time sync.
//!
//! Records an audit-trail row in `world_events` and triggers `pg_notify`
//! on the `world_events_channel`, mirroring the pattern already used by
//! token upserts (src/server/src/graphql.rs) and invite/membership
//! mutations (src/server/src/graphql/mutations_invites.rs). Centralized
//! here because the canvas-authoring mutations (walls, light sources,
//! shapes, map import) are the first callers outside those two modules.

use async_graphql::{Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use uuid::Uuid;

use crate::schema::world_events;

/// Event codes for the `world_events` audit trail / NOTIFY payload.
/// 1-5 are already used by token sync and invite/membership mutations
/// (src/server/src/graphql.rs, src/server/src/graphql/mutations_invites.rs).
/// 10 (wall), 11 (light), 12 (shape), 13 (map import), 14 (token) are
/// documented in `apps/web/src/engine/world/sync/tokens.ts`'s doc comment;
/// 15 (spec 018, Genie session state) is documented there too.
pub const EVENT_CODE_WALL_CHANGED: i32 = 10;
pub const EVENT_CODE_LIGHT_SOURCE_CHANGED: i32 = 11;
pub const EVENT_CODE_SHAPE_CHANGED: i32 = 12;
pub const EVENT_CODE_MAP_IMPORTED: i32 = 13;
pub const EVENT_CODE_TOKEN_CHANGED: i32 = 14;
/// Spec 018 (User Story 7): Genie session-loop state changes — the
/// Session Wish Pool, Doom Clock, Puzzle Clocks, and Session Resource
/// trades (data-model.md `world_events`). Reuses this existing generic
/// broadcast mechanism rather than a dedicated subscription (research.md
/// R7); consumed by the same `worldEventsCreated(worldId)` subscription
/// every world-member client already holds open. Payload shape
/// (`token_event` JSON column):
/// `{ "kind": "wish_pool" | "doom_clock" | "puzzle_clock" | "resource_trade" | "resource_grant" | "purchase" | "clock_reward", "session_id": "...", ...kind-specific fields }`
/// (contracts/genie-session-loop.md; the last three kinds added by spec
/// 020, contracts/genie-economy.md).
pub const EVENT_CODE_GENIE_SESSION_STATE: i32 = 15;

/// Record a world event to the audit trail and trigger NOTIFY for real-time sync.
pub fn record_world_event(
    conn: &mut PgConnection,
    world_id: Uuid,
    event_code: i32,
    event_payload: Option<serde_json::Value>,
    user_id: Uuid,
) -> GraphQLResult<i64> {
    let now = Utc::now().naive_utc();

    let event_id = diesel::insert_into(world_events::table)
        .values((
            world_events::world_id.eq(world_id),
            world_events::event_code.eq(event_code),
            world_events::token_event.eq(event_payload),
            world_events::schema_version.eq(1),
            world_events::created_at.eq(now),
            world_events::updated_at.eq(now),
            world_events::created_by.eq(user_id),
            world_events::updated_by.eq(user_id),
        ))
        .returning(world_events::id)
        .get_result::<i64>(conn)
        .map_err(|e| Error::new(format!("Failed to record event: {}", e)))?;

    diesel::sql_query("SELECT pg_notify('world_events_channel', $1)")
        .bind::<diesel::sql_types::Text, _>(event_id.to_string())
        .execute(conn)
        .map_err(|e| Error::new(format!("Failed to notify: {}", e)))?;

    Ok(event_id)
}

/// Look up the `world_id` that owns a scene, for callers (walls/lights/
/// shapes/import) that only carry `scene_id`.
pub fn world_id_for_scene(conn: &mut PgConnection, scene_id: Uuid) -> GraphQLResult<Uuid> {
    use crate::schema::scenes;

    scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .map_err(|e| Error::new(format!("Failed to resolve scene's world: {}", e)))
}
