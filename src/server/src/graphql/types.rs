//! Phase 4.9.Z Step 1: Core GraphQL Entity Types
//!
//! This module contains the GraphQL object type definitions for core entities:
//! - GraphQLUser (identity)
//! - GraphQLGameSystem (game system metadata)
//! - GraphQLWorld (world/session container)
//! - GraphQLWorldToken (game token/character)
//! - GraphQLWorldEvent (change log entry)
//!
//! These types are foundational and referenced by queries/mutations throughout.

use async_graphql::SimpleObject;
use chrono::NaiveDateTime;
use crate::models::{GameSystem, User, World, WorldEvent, WorldToken};
use async_graphql::Json;

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
// Game System Metadata
// ============================================================================

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLGameSystem {
    pub id: uuid::Uuid,
    pub slug: String,
    pub title: String,
    pub manifest_url: String,
    pub version: String,
    pub installed_by: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<GameSystem> for GraphQLGameSystem {
    fn from(system: GameSystem) -> Self {
        Self {
            id: system.id,
            slug: system.slug,
            title: system.title,
            manifest_url: system.manifest_url,
            version: system.version,
            installed_by: system.installed_by,
            created_at: system.created_at,
            updated_at: system.updated_at,
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
        }
    }
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
// Spec 010: Actor Ownership / Sharing
// ============================================================================

use crate::models::{ActorPermission, ActorShare};

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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorPermission {
    pub actor_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<ActorPermission> for GraphQLActorPermission {
    fn from(row: ActorPermission) -> Self {
        Self {
            actor_id: row.actor_id,
            user_id: row.user_id,
            level: ActorPermissionLevel::from_db_str(&row.level)
                .unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorShareLink {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub share_code: String,
    pub revoked: bool,
    pub created_at: NaiveDateTime,
}

impl From<ActorShare> for GraphQLActorShareLink {
    fn from(row: ActorShare) -> Self {
        Self {
            id: row.id,
            actor_id: row.actor_id,
            share_code: row.share_code,
            revoked: row.revoked,
            created_at: row.created_at,
        }
    }
}

/// Read-only, world-identity-scrubbed projection of a shared actor
/// (research.md §9) — deliberately excludes id/worldId/sceneId/createdBy/
/// ownedBy so an arbitrary logged-in viewer can't learn the source world.
#[derive(SimpleObject, Debug, Clone)]
pub struct SharedActorPreview {
    pub label: String,
    pub actor_type: String,
    pub is_npc: bool,
    pub game_system_id: Option<String>,
    pub system_data: Option<crate::graphql::GraphQLActorSystemData>,
}

// ============================================================================
// Spec 013: Items & Inventory
// ============================================================================

use crate::models::{ActorInventoryEntry, ItemEffect, ItemPermission, ItemShare, WorldItem};

/// Kind of an Item Effect. `Modifier` covers both stat boosts and
/// detriments via a signed formula (e.g. `-1d4`) — no separate buff/debuff
/// variant (research.md §1). Extensible by adding new variants only.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ItemEffectType {
    Heal,
    Damage,
    Modifier,
    AttackRoll,
}

impl ItemEffectType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ItemEffectType::Heal => "heal",
            ItemEffectType::Damage => "damage",
            ItemEffectType::Modifier => "modifier",
            ItemEffectType::AttackRoll => "attack_roll",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "heal" => Some(ItemEffectType::Heal),
            "damage" => Some(ItemEffectType::Damage),
            "modifier" => Some(ItemEffectType::Modifier),
            "attack_roll" => Some(ItemEffectType::AttackRoll),
            _ => None,
        }
    }
}

/// Scaffolded per FR-004a — not evaluated/enforced by any code path in
/// this pass; exists so a future dice-roller spec can add real triggering
/// without redesigning `world_item_effects`.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ItemEffectTrigger {
    OnUse,
    Passive,
}

impl ItemEffectTrigger {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ItemEffectTrigger::OnUse => "on_use",
            ItemEffectTrigger::Passive => "passive",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "on_use" => Some(ItemEffectTrigger::OnUse),
            "passive" => Some(ItemEffectTrigger::Passive),
            _ => None,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemEffect {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub effect_type: ItemEffectType,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<ItemEffectTrigger>,
    pub sort_order: i32,
}

impl From<ItemEffect> for GraphQLItemEffect {
    fn from(row: ItemEffect) -> Self {
        Self {
            id: row.id,
            item_id: row.item_id,
            effect_type: ItemEffectType::from_db_str(&row.effect_type)
                .unwrap_or(ItemEffectType::Modifier),
            formula: row.formula,
            target: row.target,
            trigger_kind: row
                .trigger_kind
                .as_deref()
                .and_then(ItemEffectTrigger::from_db_str),
            sort_order: row.sort_order,
        }
    }
}

/// An Item's own GraphQL projection. `effects` is resolved separately by
/// the owning query/mutation (not a field resolver here) since every
/// current call site already has both rows in hand after a join/second
/// query — keeps this type a plain `SimpleObject` with no async fetch.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItem {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub effects: Vec<GraphQLItemEffect>,
    pub my_permission_level: ActorPermissionLevel,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl GraphQLItem {
    pub fn from_row(
        row: WorldItem,
        effects: Vec<ItemEffect>,
        my_permission_level: ActorPermissionLevel,
    ) -> Self {
        Self {
            id: row.id,
            world_id: row.world_id,
            name: row.name,
            description: row.description,
            icon_asset_id: row.icon_asset_id,
            effects: effects.into_iter().map(GraphQLItemEffect::from).collect(),
            my_permission_level,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemPermission {
    pub item_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<ItemPermission> for GraphQLItemPermission {
    fn from(row: ItemPermission) -> Self {
        Self {
            item_id: row.item_id,
            user_id: row.user_id,
            level: ActorPermissionLevel::from_db_str(&row.level)
                .unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLItemShareLink {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub share_code: String,
    pub revoked: bool,
    pub created_at: NaiveDateTime,
}

impl From<ItemShare> for GraphQLItemShareLink {
    fn from(row: ItemShare) -> Self {
        Self {
            id: row.id,
            item_id: row.item_id,
            share_code: row.share_code,
            revoked: row.revoked,
            created_at: row.created_at,
        }
    }
}

/// Read-only, world-identity-scrubbed projection of a shared item
/// (mirrors `SharedActorPreview` — excludes id/worldId/createdBy/
/// ownership block so an arbitrary logged-in viewer can't learn the
/// source world, per contracts/item-share.md).
#[derive(SimpleObject, Debug, Clone)]
pub struct SharedItemPreview {
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub effects: Vec<GraphQLItemEffect>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLInventoryEntry {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub item_id: Option<uuid::Uuid>,
    pub item_name: String,
    pub quantity: i32,
}

impl From<ActorInventoryEntry> for GraphQLInventoryEntry {
    fn from(row: ActorInventoryEntry) -> Self {
        Self {
            id: row.id,
            actor_id: row.actor_id,
            item_id: row.item_id,
            item_name: row.item_name_snapshot,
            quantity: row.quantity,
        }
    }
}
