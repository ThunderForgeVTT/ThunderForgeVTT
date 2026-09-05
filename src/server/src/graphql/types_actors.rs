//! Who may see and change an actor, and what an actor looks like.
//!
//! Spec 010's ownership and sharing types, plus spec 031's actor imagery.
//! Grouped by subject rather than by the spec that introduced them: spec 031
//! added one type here and one to `types_items.rs`, and keeping them together
//! would put an item's price in the file about actors.

use super::ActorPermissionLevel;
use crate::models::WorldActorImage;
use async_graphql::SimpleObject;
use chrono::NaiveDateTime;

use crate::models::{ActorPermission, ActorShare};

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

/// One of an actor's images, named by what it is for (FR-036).
///
/// `role` is carried through as the stored string rather than mapped to an
/// enum: the column is open by decision (ADR-057/ADR-054), so a client that
/// does not recognise a role should be able to skip it, which requires being
/// told what it was. `assetId` is exposed alongside the URLs because the
/// engine's asset pipeline addresses images by id.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorImage {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub role: String,
    pub asset_id: uuid::Uuid,
    pub url: String,
    pub thumbnail_url: String,
}

impl From<WorldActorImage> for GraphQLActorImage {
    fn from(row: WorldActorImage) -> Self {
        Self {
            id: row.id,
            actor_id: row.actor_id,
            role: row.role,
            asset_id: row.asset_id,
            url: format!("/api/actor-assets/{}", row.asset_id),
            thumbnail_url: format!("/api/actor-assets/{}/thumb", row.asset_id),
        }
    }
}
