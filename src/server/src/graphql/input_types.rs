use async_graphql::{Enum, InputObject, Json, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db_types::PolicyEffectEnum;
// use crate::models::Policy; // Disabled - Policy table not implemented

use super::{GraphQLUser, GraphQLWorld, GraphQLWorldEvent, GraphQLWorldToken};

// ========== World & Scene Creation/Update ==========

/// Input for creating a new world
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateWorldInput {
    pub name: String,
    pub description: Option<String>,
    pub game_system_id: Option<String>,
    pub interface_pack_id: Option<String>,
}

/// Input for creating a new scene
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateSceneInput {
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[graphql(name = "type")]
    pub type_: Option<String>,
    pub grid_size: Option<i32>,
    pub grid_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: Option<Json<serde_json::Value>>,
}

/// Input for updating scene properties
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateSceneInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub grid_size: Option<i32>,
    pub grid_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: Option<Json<serde_json::Value>>,
    /// Spec 022 (FR-005/FR-006): GM-authored Markdown source for the
    /// scene's player-facing summary. `Some(_)` (including `Some("")`)
    /// re-renders `summaryRenderedHtml`; `None` leaves the summary
    /// untouched.
    pub summary_markdown: Option<String>,
}

// ========== Walls (Phase 6; door semantics: native canvas authoring FR-017) ==========

/// Door state for a wall segment. "None" means the wall is not a door at
/// all (its blocks_vision/blocks_movement flags always apply). "Open"
/// means the door does not block vision/movement regardless of those
/// flags. "Closed" means the flags apply as stored.
#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLDoorState {
    None,
    Open,
    Closed,
}

impl GraphQLDoorState {
    pub fn as_db_str(self) -> &'static str {
        match self {
            GraphQLDoorState::None => "none",
            GraphQLDoorState::Open => "open",
            GraphQLDoorState::Closed => "closed",
        }
    }

    pub fn from_db_str(value: &str) -> Self {
        match value {
            "open" => GraphQLDoorState::Open,
            "closed" => GraphQLDoorState::Closed,
            _ => GraphQLDoorState::None,
        }
    }
}

/// Input for creating a new wall
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateWallInput {
    pub scene_id: Uuid,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub blocks_vision: Option<bool>,
    pub blocks_movement: Option<bool>,
    pub door_state: Option<GraphQLDoorState>,
    pub metadata: Option<Json<serde_json::Value>>,
}

/// Input for updating wall properties
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateWallInput {
    pub x1: Option<f64>,
    pub y1: Option<f64>,
    pub x2: Option<f64>,
    pub y2: Option<f64>,
    pub blocks_vision: Option<bool>,
    pub blocks_movement: Option<bool>,
    pub door_state: Option<GraphQLDoorState>,
    pub metadata: Option<Json<serde_json::Value>>,
}

// ========== Light Sources (native canvas authoring) ==========

/// Input for creating a new light source
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateLightSourceInput {
    pub scene_id: Uuid,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub intensity: Option<f64>,
    pub color: Option<String>,
    pub attached_token_id: Option<Uuid>,
    pub casts_shadows: Option<bool>,
    pub metadata: Option<Json<serde_json::Value>>,
}

/// Input for updating light source properties
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateLightSourceInput {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub radius: Option<f64>,
    pub intensity: Option<f64>,
    pub color: Option<String>,
    pub attached_token_id: Option<Uuid>,
    pub casts_shadows: Option<bool>,
    pub metadata: Option<Json<serde_json::Value>>,
}

// ========== Shapes (native canvas authoring) ==========

/// Kind of freehand/drawn shape on a scene's canvas (native canvas
/// authoring: stroke, rect, ellipse, line, text).
#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLShapeKind {
    Stroke,
    Rect,
    Ellipse,
    Line,
    Text,
}

impl GraphQLShapeKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            GraphQLShapeKind::Stroke => "stroke",
            GraphQLShapeKind::Rect => "rect",
            GraphQLShapeKind::Ellipse => "ellipse",
            GraphQLShapeKind::Line => "line",
            GraphQLShapeKind::Text => "text",
        }
    }

    pub fn from_db_str(value: &str) -> Self {
        match value {
            "rect" => GraphQLShapeKind::Rect,
            "ellipse" => GraphQLShapeKind::Ellipse,
            "line" => GraphQLShapeKind::Line,
            "text" => GraphQLShapeKind::Text,
            _ => GraphQLShapeKind::Stroke,
        }
    }
}

/// Input for creating a new shape
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateShapeInput {
    pub scene_id: Uuid,
    pub kind: GraphQLShapeKind,
    pub geometry: Json<serde_json::Value>,
    pub text: Option<String>,
    pub style: Option<Json<serde_json::Value>>,
    pub visible_to_players: Option<bool>,
    pub metadata: Option<Json<serde_json::Value>>,
}

/// Input for updating shape properties
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateShapeInput {
    pub geometry: Option<Json<serde_json::Value>>,
    pub text: Option<String>,
    pub style: Option<Json<serde_json::Value>>,
    pub visible_to_players: Option<bool>,
    pub metadata: Option<Json<serde_json::Value>>,
}

// ========== Player Presence (Phase 4.9.B.3) ==========

/// Player presence data in world/scene
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPlayerPresence {
    pub player_id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
    pub idle_duration_secs: i32,
}

/// List of players online in a world
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPlayersOnlineList {
    pub world_id: Uuid,
    pub players: Vec<GraphQLPlayerPresence>,
}

// ========== Policies & Permissions ==========

/// Policy effect (Allow or Deny)
#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLPolicyEffect {
    Allow,
    Deny,
}

impl From<PolicyEffectEnum> for GraphQLPolicyEffect {
    fn from(effect: PolicyEffectEnum) -> Self {
        match effect {
            PolicyEffectEnum::Allow => Self::Allow,
            PolicyEffectEnum::Deny => Self::Deny,
        }
    }
}

/// Access control policy
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPolicy {
    pub id: Uuid,
    pub effect: GraphQLPolicyEffect,
    pub resources: Vec<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// impl From<Policy> for GraphQLPolicy {
//     fn from(policy: Policy) -> Self {
//         Self {
//             id: policy.id,
//             effect: policy.effect.into(),
//             resources: policy.resources.into_iter().flatten().collect(),
//             created_by: policy.created_by,
//             updated_by: policy.updated_by,
//             created_at: policy.created_at,
//             updated_at: policy.updated_at,
//         }
//     }
// }

// ========== Placeholder Domain Objects ==========

/// Placeholder for domain objects with minimal metadata
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPlaceholderDomainObject {
    pub schema_version: String,
    pub status: String,
}

// ========== Data Export (GDPR Compliance) ==========

/// Manifest for exported user data
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLExportManifest {
    pub schema_version: String,
    pub exported_at: DateTime<Utc>,
    pub worlds: i32,
    pub world_tokens: i32,
    pub world_events: i32,
    pub policies: i32,
}

/// Complete user data export payload
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLExportMyDataPayload {
    pub manifest: GraphQLExportManifest,
    pub user: GraphQLUser,
    pub worlds: Vec<GraphQLWorld>,
    pub world_tokens: Vec<GraphQLWorldToken>,
    pub world_events: Vec<GraphQLWorldEvent>,
    pub policies: Vec<GraphQLPolicy>,
    pub scenes: Vec<GraphQLPlaceholderDomainObject>,
    pub actors: Vec<GraphQLPlaceholderDomainObject>,
    pub asset_packs: Vec<GraphQLPlaceholderDomainObject>,
    pub game_systems: Vec<GraphQLPlaceholderDomainObject>,
}

/// Result of deleting user data (GDPR deleteMe request)
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDeleteMyDataPayload {
    pub status: String,
    pub message: String,
    pub worlds_deleted: i64,
    pub world_tokens_deleted: i64,
    pub world_events_deleted: i64,
    pub policies_deleted: i64,
    pub oauth_links_deleted: i64,
    pub sessions_deleted: i64,
    pub login_challenges_deleted: i64,
    pub oauth_link_challenges_deleted: i64,
    pub users_deleted: i64,
}

/// Result of deleting a world
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDeleteWorldPayload {
    pub id: Uuid,
    pub status: String,
    pub message: String,
}

// ========== Token Management ==========

/// Input for creating a new world token
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateWorldTokenInput {
    pub world_id: Uuid,
    pub label: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

/// Input for upserting (create or update) a world token
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpsertWorldTokenInput {
    pub world_id: Uuid,
    pub token_id: Option<String>,
    pub label: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

/// Input for moving a token to new coordinates
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLMoveTokenInput {
    pub token_id: String,
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
}

// ========== Scene-Scoped Tokens (native canvas authoring) ==========

/// Input for creating a new token on a scene
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateTokenInput {
    pub scene_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub x: f64,
    pub y: f64,
    pub rotation: Option<f64>,
    pub scale: Option<f64>,
    pub metadata: Option<Json<serde_json::Value>>,
}

/// Input for updating an existing token's position/properties
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateTokenInput {
    pub actor_id: Option<Uuid>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    pub scale: Option<f64>,
    pub metadata: Option<Json<serde_json::Value>>,
    pub owner_user_id: Option<Uuid>,
    pub is_primary: Option<bool>,
    pub photo_url: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

// ========== Fog of War Management ==========

/// Input for updating fog of war mask
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateFogMaskInput {
    pub scene_id: Uuid,
    pub bitmap_data_base64: String,
    pub width: i32,
    pub height: i32,
}

// ========== Campaign & Invite Management (Phase 4.10) ==========
//
// The actual invite/membership input and response types are defined in
// mutations_invites.rs (GenerateInviteCodeInput, JoinWorldInput,
// UpdateMemberRoleInput, WorldInvitePayload, WorldMembershipPayload) — the
// GraphQL*-prefixed versions once here were an orphaned earlier draft,
// never wired into any resolver, and have been removed.
