//! Phase 4.9.Z Step 1: Core GraphQL Entity Types
//!
//! This module contains the GraphQL object type definitions for core entities:
//! - GraphQLUser (identity)
//! - GraphQLWorld (world/session container)
//! - GraphQLWorldToken (game token/character)
//! - GraphQLWorldEvent (change log entry)
//!
//! These types are foundational and referenced by queries/mutations throughout.

use crate::models::{User, World, WorldEvent, WorldToken};
use async_graphql::Json;
use async_graphql::SimpleObject;
use chrono::NaiveDateTime;

// ============================================================================
// User Entity
// ============================================================================

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLUser {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub is_admin: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<User> for GraphQLUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: if user.is_admin {
                "admin".to_string()
            } else {
                "user".to_string()
            },
            is_admin: user.is_admin,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

// ============================================================================
// World Container
// ============================================================================

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorld {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub game_system_id: Option<String>,
    pub interface_pack_id: Option<String>,
    pub scenes: Vec<String>,
    pub actors: Vec<String>,
    pub tokens: Vec<String>,
    pub events: Vec<String>,
    pub game_system: Option<String>,
    pub interface_pack: Option<String>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub session_notes: Option<String>,
    /// Spec 017 (FR-007): gates the Actor Selection screen's "create your
    /// own character" option. GM-controlled, defaults to false.
    pub allow_player_created_actors: bool,
    /// Spec 020 (FR-003): when true, Genie Session Resource holdings
    /// carry over into the next session instead of resetting to 0.
    pub genie_resource_carryover_enabled: bool,
    /// Spec 022 (FR-014/FR-015): GM-controlled default grid type
    /// ("square" | "hex" | "gridless") applied to a newly created scene
    /// when its own `gridType` isn't explicitly set.
    pub default_scene_grid_type: String,
    /// Spec 022 (FR-002a/FR-002b, ADR-046): the world's server-authoritative
    /// currently-launched scene for Play. `None` = nothing launched yet.
    pub active_scene_id: Option<uuid::Uuid>,
}

impl From<World> for GraphQLWorld {
    fn from(world: World) -> Self {
        Self {
            id: world.id,
            name: world.name,
            description: world.description,
            game_system_id: world.game_system_id,
            interface_pack_id: world.interface_pack_id,
            scenes: Vec::new(),
            actors: Vec::new(),
            tokens: Vec::new(),
            events: Vec::new(),
            game_system: None,
            interface_pack: None,
            created_by: world.created_by,
            updated_by: world.updated_by,
            created_at: world.created_at,
            updated_at: world.updated_at,
            session_notes: world.session_notes,
            allow_player_created_actors: world.allow_player_created_actors,
            genie_resource_carryover_enabled: world.genie_resource_carryover_enabled,
            default_scene_grid_type: world.default_scene_grid_type,
            active_scene_id: world.active_scene_id,
        }
    }
}

/// One entry in `myWorldsWithRole` (Welcome page hub): a world the caller
/// owns or is an accepted member of, paired with their role in it. `role`
/// is the raw `world_members`-style string ("Owner"/"GM"/"Player") — the
/// frontend collapses Owner/GM to a single "Game Master" badge and Player
/// to "Player", matching this app's existing DM = Owner-or-GM convention
/// (spec 010) rather than introducing a third badge label here.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLMyWorldEntry {
    pub world: GraphQLWorld,
    pub role: String,
}

// ============================================================================
// World Token (Game Token/Character)
// ============================================================================

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorldToken {
    pub id: String,
    pub world_id: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub label: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub schema_version: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<WorldToken> for GraphQLWorldToken {
    fn from(token: WorldToken) -> Self {
        Self {
            id: token.id,
            world_id: token.world_id,
            x: token.x,
            y: token.y,
            z: token.z,
            label: token.label,
            health: token.health,
            max_health: token.max_health,
            created_by: token.created_by,
            updated_by: token.updated_by,
            schema_version: token.schema_version,
            created_at: token.created_at,
            updated_at: token.updated_at,
        }
    }
}

// ============================================================================
// World Event (Change Log Entry)
// ============================================================================

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorldEvent {
    pub id: i64,
    pub world_id: uuid::Uuid,
    pub event_code: i32,
    pub token_event: Option<Json<serde_json::Value>>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub schema_version: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<WorldEvent> for GraphQLWorldEvent {
    fn from(event: WorldEvent) -> Self {
        Self {
            id: event.id,
            world_id: event.world_id,
            event_code: event.event_code,
            token_event: event.token_event.map(Json),
            created_by: event.created_by,
            updated_by: event.updated_by,
            schema_version: event.schema_version,
            created_at: event.created_at,
            updated_at: event.updated_at,
        }
    }
}

// ============================================================================
// ============================================================================
// The content permission ladder
// ============================================================================

// Named for actors because spec 010 introduced it there, but it is the ladder
// for every permissioned content type — items, lore and abilities all resolve
// against it. It lives at the root rather than in `types_actors.rs` for that
// reason: three of its four callers are not about actors at all.
/// Effective permission level a caller holds on one actor. `Owner` is
/// always implicit for the world's DM (Owner/GM role) regardless of any
/// explicit `world_actor_permissions` row (spec 010 FR-017).
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ActorPermissionLevel {
    Viewer,
    Editor,
    Owner,
}

impl ActorPermissionLevel {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ActorPermissionLevel::Viewer => "Viewer",
            ActorPermissionLevel::Editor => "Editor",
            ActorPermissionLevel::Owner => "Owner",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "Viewer" => Some(ActorPermissionLevel::Viewer),
            "Editor" => Some(ActorPermissionLevel::Editor),
            "Owner" => Some(ActorPermissionLevel::Owner),
            _ => None,
        }
    }

    /// Ordering for "at least X" checks — Viewer < Editor < Owner.
    pub fn rank(self) -> u8 {
        match self {
            ActorPermissionLevel::Viewer => 0,
            ActorPermissionLevel::Editor => 1,
            ActorPermissionLevel::Owner => 2,
        }
    }
}

#[path = "types_actors.rs"]
pub mod types_actors;
pub use types_actors::*;

#[path = "types_items.rs"]
pub mod types_items;
pub use types_items::*;

#[path = "types_lore.rs"]
pub mod types_lore;
pub use types_lore::*;

#[path = "types_moderation.rs"]
pub mod types_moderation;
pub use types_moderation::*;

#[path = "types_dice.rs"]
pub mod types_dice;
pub use types_dice::*;

#[path = "types_abilities.rs"]
pub mod types_abilities;
pub use types_abilities::*;
