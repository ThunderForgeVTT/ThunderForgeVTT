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
// Spec 012: World Lore Wiki
// ============================================================================

use crate::models::{LoreEntry, LoreImageAsset, LorePermission, LoreRevision};

/// A world-scoped wiki page (FR-001..FR-021). `myPermissionLevel` and
/// `renderedHtml` are per-request-computed complex fields: `content` is
/// re-rendered (GFM parse + link resolution + sanitize) on every read
/// rather than cached, keeping `renderedHtml` always consistent with the
/// current `content`/live `world_lore_links` state (research.md 1, 2).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLLoreEntry {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub current_revision_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[async_graphql::ComplexObject]
impl GraphQLLoreEntry {
    /// Effective Viewer/Editor/Owner level the calling user holds on this
    /// entry: DM of the entry's world always resolves to Owner;
    /// otherwise the caller's explicit `world_lore_permissions` row, else
    /// Viewer (FR-003).
    async fn my_permission_level(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<ActorPermissionLevel> {
        let state = crate::graphql::app_state(ctx)?;
        let auth_user = crate::graphql::authenticated_user(ctx)?;
        crate::auth::lore_permissions::effective_lore_permission(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            self.id,
        )
        .await
    }

    /// Server-rendered, sanitized GFM HTML for `content`, with resolved
    /// in-text links substituted in as real anchors/broken-link spans
    /// (FR-004, FR-005, FR-007).
    async fn rendered_html(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<String> {
        crate::graphql::queries::lore::render_lore_content(ctx, self.world_id, &self.content).await
    }

    /// Every lore entry whose body currently contains a resolved in-text
    /// link to this entry (FR-006).
    async fn linked_from(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<GraphQLLoreEntry>> {
        crate::graphql::queries::lore::lore_entries_linking_to(ctx, self.id).await
    }
}

impl From<LoreEntry> for GraphQLLoreEntry {
    fn from(row: LoreEntry) -> Self {
        Self {
            id: row.id,
            world_id: row.world_id,
            title: row.title,
            slug: row.slug,
            content: row.content,
            current_revision_id: row.current_revision_id,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// An immutable snapshot of a lore entry's Markdown content at one point
/// in save time (FR-016/017/018).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLLoreRevision {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub content_markdown: String,
    pub author_id: uuid::Uuid,
    pub restored_from_revision_id: Option<uuid::Uuid>,
    pub created_at: NaiveDateTime,
}

#[async_graphql::ComplexObject]
impl GraphQLLoreRevision {
    /// Re-rendered on read for this specific historical revision
    /// (contracts/lore-revisions.md) - resolves in-text links against
    /// the world's current entries/actors (a past revision's links are
    /// not themselves versioned; only its Markdown text is).
    async fn rendered_html(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<String> {
        let world_id = crate::graphql::queries::lore::world_id_for_lore_entry(ctx, self.lore_entry_id).await?;
        crate::graphql::queries::lore::render_lore_content(ctx, world_id, &self.content_markdown).await
    }
}

impl From<LoreRevision> for GraphQLLoreRevision {
    fn from(row: LoreRevision) -> Self {
        Self {
            id: row.id,
            lore_entry_id: row.lore_entry_id,
            content_markdown: row.content_markdown,
            author_id: row.author_id,
            restored_from_revision_id: row.restored_from_revision_id,
            created_at: row.created_at,
        }
    }
}

/// A lore entry's ownership-block entry: one explicit (lore entry,
/// world member, permission level) grant. Direct structural mirror of
/// `GraphQLActorPermission` (spec 010), generalized to lore entries.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLorePermission {
    pub lore_entry_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: ActorPermissionLevel,
    pub updated_at: NaiveDateTime,
}

impl From<LorePermission> for GraphQLLorePermission {
    fn from(row: LorePermission) -> Self {
        Self {
            lore_entry_id: row.lore_entry_id,
            user_id: row.world_member_user_id,
            level: ActorPermissionLevel::from_db_str(&row.level).unwrap_or(ActorPermissionLevel::Viewer),
            updated_at: row.updated_at,
        }
    }
}

/// An uploaded/pasted image attached to a lore entry (FR-008/009).
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLoreImageAsset {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub url: String,
    pub thumbnail_url: String,
    pub byte_size: i32,
    pub created_at: NaiveDateTime,
}

impl From<LoreImageAsset> for GraphQLLoreImageAsset {
    fn from(row: LoreImageAsset) -> Self {
        Self {
            id: row.id,
            lore_entry_id: row.lore_entry_id,
            url: format!("/lore-assets/{}", row.id),
            thumbnail_url: format!("/lore-assets/{}/thumb", row.id),
            byte_size: row.byte_size as i32,
            created_at: row.created_at,
        }
    }
}
