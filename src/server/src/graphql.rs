use async_graphql::{
    Context, Error, InputObject, Json, MergedObject, Result as GraphQLResult, Schema, SimpleObject,
    Subscription,
};
use base64::Engine;
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use futures_util::Stream;
use std::time::Duration;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::admin::{
    load_admin_stats, recalculate_disk_usage as calculate_disk_usage,
    update_manifest_key as persist_manifest_key, update_oauth_provider as persist_oauth_provider,
    update_two_factor_policy as persist_two_factor_policy,
};
use crate::auth::world_membership::require_world_member;
use crate::models::{
    NewGenieSession,
    World,
    WorldActor,
    // Policy - disabled pending schema
};
use crate::schema::{world_actors, world_genie_sessions, worlds}; // policies disabled
use crate::state::AppState;
use crate::users::{UserDataDeleteSummary, UserDataExport, delete_user_data_owned};
// Phase 4.8.1: dnd5e_server will be loaded at runtime via game system registry

// Phase 4.9.Z Step 1: Core entity types extracted to separate module
pub mod types;
pub use types::{
    GraphQLGameSystem, GraphQLMyWorldEntry, GraphQLUser, GraphQLWorld, GraphQLWorldEvent,
    GraphQLWorldToken,
};

// Phase 4.9.Z Step 2: Admin types extracted to separate module
pub mod admin_types;
pub use admin_types::{
    GraphQLAdminBootstrapSettings, GraphQLAdminStats, GraphQLAdminWelcomeSummary,
    GraphQLAuthSecuritySettings, GraphQLOAuthProvider, GraphQLOAuthProviderConfigInput,
    GraphQLSystemManifest,
};

// Phase 4.9.Z Step 3: Input & utility types extracted to separate module
pub mod input_types;
pub use input_types::{
    GraphQLCreateLightSourceInput, GraphQLCreateSceneInput, GraphQLCreateShapeInput,
    GraphQLCreateTokenInput, GraphQLCreateWallInput, GraphQLCreateWorldInput,
    GraphQLCreateWorldTokenInput, GraphQLDeleteMyDataPayload, GraphQLDeleteWorldPayload,
    GraphQLDoorState, GraphQLExportManifest, GraphQLExportMyDataPayload, GraphQLMoveTokenInput,
    GraphQLPlaceholderDomainObject, GraphQLPlayerPresence, GraphQLPlayersOnlineList,
    GraphQLShapeKind, GraphQLUpdateFogMaskInput, GraphQLUpdateLightSourceInput,
    GraphQLUpdateSceneInput, GraphQLUpdateShapeInput, GraphQLUpdateTokenInput,
    GraphQLUpdateWallInput, GraphQLUpsertWorldTokenInput,
};

// Phase 4.9.Z Step 4a: Helper functions extracted to separate module
pub mod helpers;
pub use helpers::{
    admin_user, app_state, authenticated_user, get_world_id_from_scene, load_all_worlds,
    load_game_systems, load_owned_world_event_by_id, load_owned_world_events,
    load_owned_world_token_by_id, load_owned_world_tokens, load_owned_worlds,
    load_visible_world_by_id, normalize_world_name, prepare_world_input, require_visible_world,
    validate_world_name, world_write_error,
};

// Phase 4.9.Z Step 5: Query extraction into separate modules
pub mod queries;
pub use queries::{
    AbilityQuery, ActorQuery, AdminQuery, GenieSessionQuery, HealthcheckQuery, InventoryQuery,
    InviteQuery, ItemQuery, LoreQuery, ModerationQuery, RollQuery, SceneQuery, UserQuery,
    WorldEventsSinceQuery, WorldSyncPlanQuery,
};

// Phase 4.10.B: Invite & Membership mutations for multiplayer campaigns
pub mod mutations_invites;
pub mod share_codes;
pub use mutations_invites::InviteMutation;

// Phase 6: Wall mutations (vision-blocking scene geometry)
pub mod mutations_interactives; // Spec 030: interactive elements
pub mod mutations_walls;
pub use mutations_walls::WallMutation;

// Native canvas authoring: light source mutations
pub mod mutations_lighting;
pub use mutations_lighting::LightSourceMutation;

// Native canvas authoring: shape (stroke/rect/ellipse/line/text) mutations
pub mod mutations_shapes;
pub use mutations_shapes::ShapeMutation;

// Native canvas authoring: scene-scoped token mutations
pub mod mutations_heartbeat;
pub mod mutations_reconcile;
pub mod mutations_tokens;
pub use mutations_heartbeat::{HeartbeatMutation, PresenceQuery};
pub use mutations_reconcile::ReconcileMutation;
pub use mutations_tokens::TokenMutation;

// Spec 002: canvas image asset storage (RustFS)
pub mod mutations_assets;
pub use mutations_assets::{AssetMutation, AssetQuery};

// Spec 010: actor creation/field-editing mutations
pub mod mutations_actors;
pub use mutations_actors::ActorMutation;

// Spec 010: the actor "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_actor_permissions;
pub use mutations_actor_permissions::{ActorPermissionMutation, ActorPermissionQuery};

// Spec 010: actor sharing and cross-world deep copy
pub mod mutations_actor_images; // Spec 031: portrait/token imagery, rows keyed by role
pub mod mutations_actor_shares;
pub use mutations_actor_shares::{ActorShareMutation, ActorShareQuery};

// Spec 012: lore entry creation/editing/deletion/restore mutations
pub mod mutations_lore;
pub use mutations_lore::LoreMutation;

// Spec 012: the lore entry "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_lore_permissions;
pub use mutations_lore_permissions::{LorePermissionMutation, LorePermissionQuery};

// Spec 031 (FR-038): the lore tree and its tags — move, tag, untag
pub mod mutations_lore_tree;

// Spec 012: paste/drop image upload for lore entries
pub mod mutations_lore_images;
pub use mutations_lore_images::LoreImageMutation;

// Spec 013: item creation/field-editing/deletion and effect CRUD
pub mod mutations_abilities;
pub mod mutations_ability_permissions;
pub mod mutations_ability_shares;
pub mod mutations_actor_abilities;
pub mod mutations_items;
pub use mutations_abilities::AbilityMutation;
pub use mutations_ability_permissions::{AbilityPermissionMutation, AbilityPermissionQuery};
pub use mutations_ability_shares::{AbilityShareMutation, AbilityShareQuery};
pub use mutations_actor_abilities::{ActorAbilityMutation, ActorAbilityQuery};
pub use mutations_items::ItemMutation;

// Spec 013: the item "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_item_permissions;
pub mod mutations_item_prices; // Spec 031: the GM's presentational price note
pub use mutations_item_permissions::{ItemPermissionMutation, ItemPermissionQuery};

// Spec 013: item sharing and cross-world deep copy
pub mod mutations_item_shares;
pub use mutations_item_shares::{ItemShareMutation, ItemShareQuery};

// Spec 013: actor inventory (Item + quantity, permissioned via the actor)
pub mod mutations_inventory;
pub use mutations_inventory::InventoryMutation;

// Spec 031: taking a placed item off the map into an inventory — one
// transaction, exactly one winner.
pub mod mutations_pickup;
pub use mutations_pickup::PickupMutation;

// Spec 031 (T055, FR-019): `bringPartyToScene` — the party's characters get a
// token in the destination, and no character gets a second one.
pub mod mutations_party;
pub use mutations_party::PartyMutation;

// Spec 015: DMCA notice-and-takedown moderation mutations
pub mod mutations_moderation;
pub use mutations_moderation::ModerationMutation;

pub mod mutations_roll;
pub use mutations_roll::RollMutation;

// Spec 018 (User Story 7): the Genie session loop — Session Wish Pool,
// Doom Clock, Puzzle Clocks, and Session Resource trades.
pub mod mutations_genie_session;
pub use mutations_genie_session::GenieSessionMutation;

// Play-view Chat + Combat. Both are built on the existing `world_events`
// bus rather than a separate transport — see each module's doc comment.
pub mod mutations_chat;
pub use mutations_chat::{ChatMutation, ChatQuery};
pub mod mutations_combat;
pub use mutations_combat::{CombatMutation, CombatQuery};

// Spec 017: actor "available for claiming" flag, atomic claiming,
// player-created characters, and GM un-claim.
pub mod mutations_actor_claims;
pub use mutations_actor_claims::{ActorClaimMutation, ActorClaimQuery};

// Admin types are now in admin_types.rs module (Phase 4.9.Z Step 2)

impl From<UserDataDeleteSummary> for GraphQLDeleteMyDataPayload {
    fn from(summary: UserDataDeleteSummary) -> Self {
        Self {
            status: "deleted".to_string(),
            message: "User profile and owned data were permanently deleted".to_string(),
            worlds_deleted: summary.worlds_deleted,
            world_tokens_deleted: summary.world_tokens_deleted,
            world_events_deleted: summary.world_events_deleted,
            policies_deleted: summary.policies_deleted,
            oauth_links_deleted: summary.oauth_links_deleted,
            sessions_deleted: summary.sessions_deleted,
            login_challenges_deleted: summary.login_challenges_deleted,
            oauth_link_challenges_deleted: summary.oauth_link_challenges_deleted,
            users_deleted: summary.users_deleted,
        }
    }
}

impl From<UserDataExport> for GraphQLExportMyDataPayload {
    fn from(export: UserDataExport) -> Self {
        Self {
            manifest: GraphQLExportManifest {
                schema_version: export.manifest.schema_version.to_string(),
                exported_at: export.manifest.exported_at,
                worlds: export.manifest.counts.worlds as i32,
                world_tokens: export.manifest.counts.world_tokens as i32,
                world_events: export.manifest.counts.world_events as i32,
                policies: export.manifest.counts.policies as i32,
            },
            user: GraphQLUser {
                id: export.user.id,
                username: export.user.username,
                email: export.user.email,
                role: export.user.role,
                is_admin: export.user.is_admin,
                created_at: export.user.created_at,
                updated_at: export.user.updated_at,
            },
            worlds: export.worlds.into_iter().map(GraphQLWorld::from).collect(),
            world_tokens: export
                .world_tokens
                .into_iter()
                .map(GraphQLWorldToken::from)
                .collect(),
            world_events: export
                .world_events
                .into_iter()
                .map(GraphQLWorldEvent::from)
                .collect(),
            // policies are disabled (module not implemented)
            policies: vec![],
            scenes: export
                .scenes
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            actors: export
                .actors
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            asset_packs: export
                .asset_packs
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            game_systems: export
                .game_systems
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
        }
    }
}

// Constants and struct moved to helpers.rs module (Phase 4.9.Z Step 4a)

// Helper functions moved to helpers.rs module (Phase 4.9.Z Step 4a)

// ========== Phase 3.5: Scene System GraphQL Types ==========

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLScene {
    scene_id: uuid::Uuid,
    world_id: uuid::Uuid,
    name: String,
    description: Option<String>,
    #[graphql(name = "type")]
    type_: String,
    grid_size: i32,
    grid_type: String,
    width: i32,
    height: i32,
    metadata: Option<Json<serde_json::Value>>,
    owner_id: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
    /// Native canvas authoring: set by map import (data-model.md's Scene
    /// section); resolves against the existing `/assets/<path>` static
    /// route. `None` = no background art. Superseded by
    /// `background_asset_id` (spec 002, FR-018).
    background_image_path: Option<String>,
    /// Spec 002 (FR-018): the RustFS-backed `canvas_image_assets` row
    /// for this scene's background, when migrated.
    background_asset_id: Option<uuid::Uuid>,
    /// Bug fix (found while investigating "map import doesn't take the
    /// image"): `background_asset_id` alone is a bare UUID with no
    /// fetchable URL — the frontend's canvas-loading path
    /// (`WorldPage.tsx`) only ever read `background_image_path`, which
    /// dd2vtt import (spec 022/002, `save_background_image`) never sets,
    /// since it writes to RustFS via `background_asset_id` instead. This
    /// computed field is the fetchable URL for whichever mechanism is
    /// actually populated (`background_asset_id` preferred — RustFS via
    /// `canvas_assets_serve.rs`'s existing `GET /canvas-assets/{id}`
    /// route, already used by `AssetPasteTool`; falls back to the legacy
    /// `background_image_path` static-file route for scenes never
    /// migrated to RustFS), mirroring `preview_url`'s existing pattern.
    background_url: Option<String>,
    /// Spec 022: GM-authored Markdown source for the scene's player-facing
    /// summary.
    summary_markdown: Option<String>,
    /// Spec 022: sanitized HTML rendered from `summary_markdown` — render
    /// this, never `summary_markdown` directly.
    summary_rendered_html: Option<String>,
    /// Spec 022 (FR-003, Clarifications): player-facing visibility, hidden
    /// by default on creation.
    hidden: bool,
    /// Spec 022 (FR-011): computed URL for the scene's reduced-size
    /// preview image, `None` until one has been generated.
    preview_url: Option<String>,
}

impl From<crate::models::Scene> for GraphQLScene {
    fn from(scene: crate::models::Scene) -> Self {
        Self {
            scene_id: scene.scene_id,
            world_id: scene.world_id,
            name: scene.name,
            description: scene.description,
            type_: scene.type_,
            grid_size: scene.grid_size,
            grid_type: scene.grid_type,
            width: scene.width,
            height: scene.height,
            metadata: scene.metadata.map(Json),
            owner_id: scene.owner_id,
            created_at: scene.created_at,
            updated_at: scene.updated_at,
            // Bug fix: both these routes are nested under `/api` in
            // main.rs (`.nest("/api", api_router...)`, which merges in
            // `canvas_assets_serve::router()`/`scene_assets_serve::router()`)
            // — their own `Router::new().route("/canvas-assets/{id}", ...)`
            // declarations show only the path *before* that nesting.
            // `preview_url` (pre-existing, spec 022) had this same missing
            // `/api` prefix bug, found while fixing `background_url`
            // (live-verified: without the prefix, dev's Vite proxy only
            // forwards `/api/*`/`/assets/*` to the backend, so the bare
            // path fell through to the SPA's `index.html`, not a 404 —
            // the image request "succeeded" with an HTML body instead of
            // image bytes).
            // The `.webp` suffix is required, not cosmetic: this URL is
            // handed to the engine's `AssetServer`, which resolves an
            // image loader by file extension and gives up (without ever
            // requesting the bytes) on an extensionless path. Every stored
            // object is WebP — see `canvas_assets_serve::parse_asset_id`,
            // which strips the extension back off.
            background_url: scene
                .background_asset_id
                .map(|id| format!("/api/canvas-assets/{id}.webp"))
                .or_else(|| scene.background_image_path.clone()),
            background_image_path: scene.background_image_path,
            background_asset_id: scene.background_asset_id,
            summary_markdown: scene.summary_markdown,
            summary_rendered_html: scene.summary_rendered_html,
            hidden: scene.hidden,
            preview_url: scene
                .preview_asset_id
                .map(|id| format!("/api/scene-assets/{id}/thumb")),
        }
    }
}

// Spec 009 (T001): GM staging page's NPC roster read path.
// Spec 010: `myPermissionLevel` is a per-request-computed field (depends on
// the calling user), so this type is `#[graphql(complex)]` — its resolver
// lives in the `#[ComplexObject]` impl below rather than being a plain
// struct field populated by `From<WorldActor>`.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLWorldActor {
    id: uuid::Uuid,
    world_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    actor_type: String,
    game_system_id: Option<String>,
    label: String,
    description: Option<String>,
    is_public: bool,
    is_npc: bool,
    created_by: uuid::Uuid,
    owned_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
    /// Spec 017 (FR-004): GM-set flag offering this (PC-only) actor to a
    /// joining player on the Actor Selection screen.
    available_for_claim: bool,
}

#[async_graphql::ComplexObject]
impl GraphQLWorldActor {
    /// Effective Viewer/Editor/Owner level the calling user holds on this
    /// actor (data-model.md's "effective actor permission") — DM of the
    /// actor's world always resolves to `Owner` (FR-017); otherwise the
    /// caller's explicit `world_actor_permissions` row, else `Viewer`
    /// (FR-016).
    async fn my_permission_level(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<crate::graphql::types::ActorPermissionLevel> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        crate::auth::actor_permissions::effective_actor_permission(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            self.id,
        )
        .await
    }

    /// Spec 012 (FR-006): every lore entry whose body currently contains
    /// a resolved in-text link to this actor.
    async fn lore_linked_from(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<crate::graphql::types::GraphQLLoreEntry>> {
        let state = app_state(ctx)?;
        crate::graphql::queries::lore::lore_entries_linking_to_actor(state, self.id).await
    }

    /// Spec 031 (FR-036): every image this actor has, keyed by role.
    ///
    /// One list rather than a `portrait`/`token` pair of fields: ADR-057 keeps
    /// the role set open so the deferred presentation images are additive, and
    /// a client that renders a role it knows and skips one it does not needs
    /// no schema change when a new role appears.
    async fn images(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<crate::graphql::types::GraphQLActorImage>> {
        let state = app_state(ctx)?;
        let rows = crate::graphql::mutations_actor_images::actor_images_impl(state, self.id).await?;
        Ok(rows
            .into_iter()
            .map(crate::graphql::types::GraphQLActorImage::from)
            .collect())
    }

    /// Spec 017 (FR-012): who currently has this actor claimed, if anyone.
    /// `None` if unclaimed — independent of `available_for_claim`, since an
    /// actor can be un-flagged without losing its claim (data-model.md).
    async fn claimed_by(&self, ctx: &Context<'_>) -> GraphQLResult<Option<GraphQLWorldMember>> {
        let state = app_state(ctx)?;
        crate::graphql::mutations_actor_claims::claimed_by_impl(state, self.id).await
    }
}

impl From<WorldActor> for GraphQLWorldActor {
    fn from(actor: WorldActor) -> Self {
        Self {
            id: actor.id,
            world_id: actor.world_id,
            scene_id: actor.scene_id,
            actor_type: actor.actor_type,
            game_system_id: actor.game_system_id,
            label: actor.label,
            description: actor.description,
            is_public: actor.is_public,
            is_npc: actor.is_npc,
            created_by: actor.created_by,
            owned_by: actor.owned_by,
            created_at: actor.created_at,
            updated_at: actor.updated_at,
            available_for_claim: actor.available_for_claim,
        }
    }
}

// Spec 017: a world member, exposed for `claimedBy`/`GraphQLActorClaim`.
// Deliberately minimal (no role/joined_at) — nothing downstream needs more
// than "who" yet; extend here rather than introducing a second member type
// if that changes.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorldMember {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub username: String,
}

// Spec 017: the result of `claimActor`/`createAndClaimActor`, and
// `myActorClaim`'s non-null case.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GraphQLActorClaim {
    pub actor_id: uuid::Uuid,
    pub world_member_id: uuid::Uuid,
    pub claimed_by_user_id: uuid::Uuid,
    pub claimed_at: chrono::DateTime<chrono::Utc>,
}

#[async_graphql::ComplexObject]
impl GraphQLActorClaim {
    async fn actor(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        crate::graphql::mutations_actor_claims::load_actor_impl(state, self.actor_id).await
    }
}

// Scene input types moved to input_types.rs (Phase 4.9.Z Step 3)

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWall {
    wall_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    blocks_vision: bool,
    blocks_movement: bool,
    door_state: GraphQLDoorState,
    /// Spec 030: who may change the door's state, not the state itself.
    locked: bool,
    /// Spec 030: not drawn for players until revealed. Presentation only —
    /// the geometry reaches every client, because a door that did not arrive
    /// would also stop blocking vision and movement.
    secret: bool,
    metadata: Option<Json<serde_json::Value>>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::Wall> for GraphQLWall {
    fn from(wall: crate::models::Wall) -> Self {
        Self {
            wall_id: wall.wall_id,
            scene_id: wall.scene_id,
            x1: wall.x1,
            y1: wall.y1,
            x2: wall.x2,
            y2: wall.y2,
            blocks_vision: wall.blocks_vision,
            blocks_movement: wall.blocks_movement,
            door_state: GraphQLDoorState::from_db_str(&wall.door_state),
            locked: wall.locked,
            secret: wall.secret,
            metadata: wall.metadata.map(Json),
            created_by: wall.created_by,
            updated_by: wall.updated_by,
            created_at: wall.created_at,
            updated_at: wall.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLLightSource {
    light_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    x: f64,
    y: f64,
    radius: f64,
    intensity: f64,
    color: Option<String>,
    attached_token_id: Option<uuid::Uuid>,
    casts_shadows: bool,
    metadata: Option<Json<serde_json::Value>>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::LightSource> for GraphQLLightSource {
    fn from(light: crate::models::LightSource) -> Self {
        Self {
            light_id: light.light_id,
            scene_id: light.scene_id,
            x: light.x,
            y: light.y,
            radius: light.radius,
            intensity: light.intensity,
            color: light.color,
            attached_token_id: light.attached_token_id,
            casts_shadows: light.casts_shadows,
            metadata: light.metadata.map(Json),
            created_by: light.created_by,
            updated_by: light.updated_by,
            created_at: light.created_at,
            updated_at: light.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLShape {
    shape_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    kind: GraphQLShapeKind,
    geometry: Json<serde_json::Value>,
    text: Option<String>,
    style: Option<Json<serde_json::Value>>,
    visible_to_players: bool,
    metadata: Option<Json<serde_json::Value>>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::Shape> for GraphQLShape {
    fn from(shape: crate::models::Shape) -> Self {
        Self {
            shape_id: shape.shape_id,
            scene_id: shape.scene_id,
            kind: GraphQLShapeKind::from_db_str(&shape.kind),
            geometry: Json(shape.geometry),
            text: shape.text,
            style: shape.style.map(Json),
            visible_to_players: shape.visible_to_players,
            metadata: shape.metadata.map(Json),
            created_by: shape.created_by,
            updated_by: shape.updated_by,
            created_at: shape.created_at,
            updated_at: shape.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLToken {
    token_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    actor_id: Option<uuid::Uuid>,
    x: f64,
    y: f64,
    rotation: f64,
    scale: f64,
    metadata: Option<Json<serde_json::Value>>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
    owner_user_id: Option<uuid::Uuid>,
    is_primary: bool,
    photo_url: Option<String>,
    health: Option<i32>,
    max_health: Option<i32>,
    /// What this token represents: `character`, `npc`, `vehicle`, `object`.
    token_type: String,
}

impl From<crate::models::Token> for GraphQLToken {
    fn from(token: crate::models::Token) -> Self {
        Self {
            token_id: token.token_id,
            scene_id: token.scene_id,
            actor_id: token.actor_id,
            x: token.x,
            y: token.y,
            rotation: token.rotation,
            scale: token.scale,
            metadata: token.metadata.map(Json),
            created_at: token.created_at,
            updated_at: token.updated_at,
            owner_user_id: token.owner_user_id,
            is_primary: token.is_primary,
            photo_url: token.photo_url,
            health: token.health,
            max_health: token.max_health,
            token_type: token.token_type,
        }
    }
}

// Token input types moved to input_types.rs (Phase 4.9.Z Step 3)

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLFogMask {
    fog_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    bitmap_data_base64: String,
    version: i32,
    width: i32,
    height: i32,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::FogMask> for GraphQLFogMask {
    fn from(fog: crate::models::FogMask) -> Self {
        Self {
            fog_id: fog.fog_id,
            scene_id: fog.scene_id,
            bitmap_data_base64: fog.bitmap_data_base64(),
            version: fog.version,
            width: fog.width,
            height: fog.height,
            updated_by: fog.updated_by,
            created_at: fog.created_at,
            updated_at: fog.updated_at,
        }
    }
}

// World token input types moved to input_types.rs (Phase 4.9.Z Step 3)

#[derive(Default)]
pub struct WorldTokenMutation;

#[async_graphql::Object]
impl WorldTokenMutation {
    async fn create_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = uuid::Uuid::now_v7().to_string();
        let world_id = input.world_id;
        let label = input.label;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        let created_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;

            diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create world token"))?;

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, input.world_id).await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(created_token))
    }

    async fn upsert_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpsertWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input
            .token_id
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let world_id = input.world_id;
        let label = input.label.clone();
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        // 🎮📤 Phase 4.6: Circular flow begins - client sent mutation
        eprintln!(
            "[Phase4.6📤] upsertToken mutation received: token_id={}, pos=({},{},{})",
            token_id, x, y, z
        );

        // Combine all DB operations into a single spawn_blocking call
        let token_id_clone = token_id.clone();
        let (upserted_token, event_id) = tokio::task::spawn_blocking(move || {
            use crate::schema::{world_events, world_tokens};
            use diesel::prelude::*;

            // 1. UPSERT token
            let upserted = diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id_clone),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    // 🔐 ADR-010: Server assigns ownership from auth context
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .on_conflict(world_tokens::id)
                .do_update()
                .set((
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::updated_by.eq(user_id),
                    world_tokens::updated_at.eq(now),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)?;

            // 2. Record world_event for audit trail
            let token_event_payload = serde_json::json!({
                "token_id": token_id_clone,
                "x": x,
                "y": y,
                "z": z,
                "label": label,
                "health": health,
                "max_health": max_health,
            });

            let event_id = diesel::insert_into(world_events::table)
                .values((
                    world_events::world_id.eq(world_id),
                    // Event code 1 = token_event
                    world_events::event_code.eq(1),
                    world_events::token_event.eq(token_event_payload),
                    world_events::schema_version.eq(1),
                    world_events::created_at.eq(now),
                    world_events::updated_at.eq(now),
                    world_events::created_by.eq(user_id),
                    world_events::updated_by.eq(user_id),
                ))
                .returning(world_events::id)
                .get_result::<i64>(&mut conn)?;

            // 3. Trigger pg_notify for backplane broadcast
            diesel::sql_query("SELECT pg_notify('world_events_channel', $1)")
                .bind::<diesel::sql_types::Text, _>(event_id.to_string())
                .execute(&mut conn)?;

            Ok::<_, diesel::result::Error>((upserted, event_id))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| Error::new(format!("Database operation failed: {}", e)))?;

        eprintln!(
            "[Phase4.6✅] upsertToken complete: token_id={}, event_id={}, broadcasted",
            token_id, event_id
        );

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, world_id).await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(upserted_token))
    }

    async fn move_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLMoveTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id;
        let x = input.x;
        let y = input.y;
        let z = input.z.unwrap_or(0.0);

        let moved_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;

            diesel::update(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .set((
                world_tokens::x.eq(x),
                world_tokens::y.eq(y),
                world_tokens::z.eq(z),
                world_tokens::updated_by.eq(user_id),
                world_tokens::updated_at.eq(now),
            ))
            .returning(crate::models::WorldToken::as_returning())
            .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to move token"))?;

        // Phase 4.9.B.2: Touch last_seen on mutation
        if let Err(e) =
            crate::session::touch_last_seen(state.db_pool.clone(), user_id, moved_token.world_id)
                .await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(moved_token))
    }

    async fn delete_world_token(&self, ctx: &Context<'_>, token_id: String) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            diesel::delete(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

        Ok(deleted > 0)
    }
}

// ============================================================================
// Phase 4.8.1: Actor System Data Mutations (Generic for all systems)
// ============================================================================

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateActorSystemDataInput {
    actor_id: uuid::Uuid,
    game_system_id: String,        // 'dnd5e', 'pathfinder2e', etc.
    data_type: String, // 'ability_data', 'resource_data', 'proficiency_data', 'trait_data', 'spell_data'
    data: Json<serde_json::Value>, // Raw JSON for system-specific content
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorSystemData {
    id: uuid::Uuid,
    actor_id: uuid::Uuid,
    game_system_id: String,
    ability_data: Option<Json<serde_json::Value>>,
    resource_data: Option<Json<serde_json::Value>>,
    proficiency_data: Option<Json<serde_json::Value>>,
    trait_data: Option<Json<serde_json::Value>>,
    spell_data: Option<Json<serde_json::Value>>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::ActorSystemData> for GraphQLActorSystemData {
    fn from(data: crate::models::ActorSystemData) -> Self {
        Self {
            id: data.id,
            actor_id: data.actor_id,
            game_system_id: data.game_system_id,
            ability_data: data.ability_data.map(Json),
            resource_data: data.resource_data.map(Json),
            proficiency_data: data.proficiency_data.map(Json),
            trait_data: data.trait_data.map(Json),
            spell_data: data.spell_data.map(Json),
            created_at: data.created_at,
            updated_at: data.updated_at,
        }
    }
}

/// GraphQL event for actor system data changes (used in subscriptions)
/// Emitted from pg_notify backplane via PostgreSQL trigger
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActorSystemDataEvent {
    id: uuid::Uuid,
    actor_id: uuid::Uuid,
    game_system_id: String,
    event_type: String, // "INSERT", "UPDATE", "DELETE"
    ability_data: Option<Json<serde_json::Value>>,
    resource_data: Option<Json<serde_json::Value>>,
    proficiency_data: Option<Json<serde_json::Value>>,
    trait_data: Option<Json<serde_json::Value>>,
    spell_data: Option<Json<serde_json::Value>>,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Default)]
pub struct ActorSystemDataMutation;

#[async_graphql::Object]
impl ActorSystemDataMutation {
    /// Generic mutation for updating system-specific actor data
    /// Validates against manifest schema for the system
    /// 🎮📤 Phase 4.8.1: Client sends mutation to update game-specific stats
    async fn update_actor_system_data(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpdateActorSystemDataInput,
    ) -> GraphQLResult<GraphQLActorSystemData> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let actor_id = input.actor_id;
        let game_system_id = input.game_system_id.clone();
        let data_type = input.data_type.clone();
        let data_value = input.data.0.clone();
        let now = Utc::now().naive_utc();

        // Spec 010 (research.md §4): authorization moved from the old
        // single-owner `world_actors.owned_by` check to the real
        // Viewer/Editor/Owner permission model — Editor-or-Owner
        // effective permission required (the DM always qualifies). This
        // must happen (and be awaited) before the DB connection below is
        // obtained — PgConnection is not held-across-`.await` safe in
        // this codebase's usage pattern (see `world_membership.rs`).
        crate::auth::actor_permissions::require_actor_permission(
            state,
            user_id,
            auth_user.is_admin,
            actor_id,
            crate::graphql::types::ActorPermissionLevel::Editor,
        )
        .await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let upserted_data = tokio::task::spawn_blocking({
            let game_system_id = game_system_id.clone();
            let data_type = data_type.clone();
            let data_value = data_value.clone();
            move || {
                use crate::schema::{world_actor_system_data, world_actors};
                use diesel::prelude::*;

                // Permission already verified above; just load the actor.
                let actor = world_actors::table
                    .filter(world_actors::id.eq(actor_id))
                    .select(crate::models::WorldActor::as_select())
                    .first::<crate::models::WorldActor>(&mut conn)
                    .optional()
                    .map_err(|e| format!("Failed to load actor: {}", e))?
                    .ok_or_else(|| "Actor not found".to_string())?;

                // 2. Validate: Actor's game_system_id must match the mutation's game_system_id
                if actor.game_system_id.as_ref() != Some(&game_system_id) {
                    return Err(
                        "Game system mismatch: actor is not configured for this system".to_string(),
                    );
                }

                // 3. Validate data_type and use registry-based validators
                if !matches!(
                    data_type.as_str(),
                    "ability_data"
                        | "resource_data"
                        | "proficiency_data"
                        | "trait_data"
                        | "spell_data"
                ) {
                    return Err("Unknown data_type".to_string());
                }

                // 🔔 C2: Call system registry to validate data using game system's validators
                crate::systems::validate_actor_system_data(
                    &game_system_id,
                    &data_type,
                    &data_value,
                )
                .map_err(|e| format!("Validation failed: {}", e))?;

                // 4. UPSERT actor system data with appropriate column
                let result = match data_type.as_str() {
                    "ability_data" => diesel::insert_into(world_actor_system_data::table)
                        .values((
                            world_actor_system_data::actor_id.eq(actor_id),
                            world_actor_system_data::game_system_id.eq(&game_system_id),
                            world_actor_system_data::ability_data.eq(&data_value),
                            world_actor_system_data::created_by.eq(user_id),
                            world_actor_system_data::updated_by.eq(user_id),
                        ))
                        .on_conflict(world_actor_system_data::actor_id)
                        .do_update()
                        .set((
                            world_actor_system_data::ability_data.eq(&data_value),
                            world_actor_system_data::updated_by.eq(user_id),
                            world_actor_system_data::updated_at.eq(now),
                        ))
                        .returning(crate::models::ActorSystemData::as_returning())
                        .get_result(&mut conn),
                    "resource_data" => diesel::insert_into(world_actor_system_data::table)
                        .values((
                            world_actor_system_data::actor_id.eq(actor_id),
                            world_actor_system_data::game_system_id.eq(&game_system_id),
                            world_actor_system_data::resource_data.eq(&data_value),
                            world_actor_system_data::created_by.eq(user_id),
                            world_actor_system_data::updated_by.eq(user_id),
                        ))
                        .on_conflict(world_actor_system_data::actor_id)
                        .do_update()
                        .set((
                            world_actor_system_data::resource_data.eq(&data_value),
                            world_actor_system_data::updated_by.eq(user_id),
                            world_actor_system_data::updated_at.eq(now),
                        ))
                        .returning(crate::models::ActorSystemData::as_returning())
                        .get_result(&mut conn),
                    "proficiency_data" => diesel::insert_into(world_actor_system_data::table)
                        .values((
                            world_actor_system_data::actor_id.eq(actor_id),
                            world_actor_system_data::game_system_id.eq(&game_system_id),
                            world_actor_system_data::proficiency_data.eq(&data_value),
                            world_actor_system_data::created_by.eq(user_id),
                            world_actor_system_data::updated_by.eq(user_id),
                        ))
                        .on_conflict(world_actor_system_data::actor_id)
                        .do_update()
                        .set((
                            world_actor_system_data::proficiency_data.eq(&data_value),
                            world_actor_system_data::updated_by.eq(user_id),
                            world_actor_system_data::updated_at.eq(now),
                        ))
                        .returning(crate::models::ActorSystemData::as_returning())
                        .get_result(&mut conn),
                    "trait_data" => diesel::insert_into(world_actor_system_data::table)
                        .values((
                            world_actor_system_data::actor_id.eq(actor_id),
                            world_actor_system_data::game_system_id.eq(&game_system_id),
                            world_actor_system_data::trait_data.eq(&data_value),
                            world_actor_system_data::created_by.eq(user_id),
                            world_actor_system_data::updated_by.eq(user_id),
                        ))
                        .on_conflict(world_actor_system_data::actor_id)
                        .do_update()
                        .set((
                            world_actor_system_data::trait_data.eq(&data_value),
                            world_actor_system_data::updated_by.eq(user_id),
                            world_actor_system_data::updated_at.eq(now),
                        ))
                        .returning(crate::models::ActorSystemData::as_returning())
                        .get_result(&mut conn),
                    "spell_data" => diesel::insert_into(world_actor_system_data::table)
                        .values((
                            world_actor_system_data::actor_id.eq(actor_id),
                            world_actor_system_data::game_system_id.eq(&game_system_id),
                            world_actor_system_data::spell_data.eq(&data_value),
                            world_actor_system_data::created_by.eq(user_id),
                            world_actor_system_data::updated_by.eq(user_id),
                        ))
                        .on_conflict(world_actor_system_data::actor_id)
                        .do_update()
                        .set((
                            world_actor_system_data::spell_data.eq(&data_value),
                            world_actor_system_data::updated_by.eq(user_id),
                            world_actor_system_data::updated_at.eq(now),
                        ))
                        .returning(crate::models::ActorSystemData::as_returning())
                        .get_result(&mut conn),
                    _ => return Err("Invalid data type".to_string()),
                };

                result.map_err(|e| e.to_string())
            }
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| Error::new(format!("Failed to upsert actor system data: {}", e)))?;

        eprintln!(
            "[Phase4.8.1✅] updateActorSystemData complete: actor_id={}, system={}, data_type={}",
            actor_id, game_system_id, data_type
        );

        Ok(GraphQLActorSystemData::from(upserted_data))
    }
}

// Query structs moved to queries modules (Phase 4.9.Z Step 5)

/// Testable core of `SceneMutation::update_scene_hidden` (spec 022,
/// FR-007/FR-019). Unlike `update_scene` (owner-of-scene-gated,
/// pre-existing), this uses the broader `is_dm_of_world` check — any
/// GM/Owner of the scene's world may toggle visibility, not just whoever
/// created the scene, matching spec.md's "GM/Owner members" wording.
pub async fn update_scene_hidden_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    scene_id: uuid::Uuid,
    hidden: bool,
) -> GraphQLResult<GraphQLScene> {
    use crate::schema::scenes;
    use diesel::prelude::*;

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<uuid::Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Scene not found"))?;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a scene's visibility",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let updated_scene = tokio::task::spawn_blocking(move || {
        diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
            .set(scenes::hidden.eq(hidden))
            .returning(crate::models::Scene::as_returning())
            .get_result(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update scene visibility"))?;

    Ok(GraphQLScene::from(updated_scene))
}

/// Testable core of `SceneMutation::launch_scene` (spec 022,
/// FR-002a/FR-002b/FR-002c, ADR-046). Sets the world's server-authoritative
/// active scene and broadcasts the change over the existing `world_events`
/// transport so every world member currently in Play live-switches to it
/// (research.md §6).
pub async fn launch_scene_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    scene_id: uuid::Uuid,
) -> GraphQLResult<GraphQLWorld> {
    use crate::schema::{scenes, worlds};
    use diesel::prelude::*;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may launch a scene"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated_world = tokio::task::spawn_blocking(move || {
        // The scene must belong to this world — a GM of world A must
        // not be able to point world B's active scene at one of A's
        // scenes (FR-002b implicitly assumes same-world).
        let scene_world_id = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<uuid::Uuid>(&mut conn)?;

        if scene_world_id != world_id {
            return Err(diesel::result::Error::RollbackTransaction);
        }

        let world = diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::active_scene_id.eq(scene_id))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)?;

        crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_SCENE_LAUNCHED,
            Some(serde_json::json!({ "sceneId": scene_id.to_string() })),
            user_id,
        )
        .map_err(|_| diesel::result::Error::RollbackTransaction)?;

        Ok(world)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to launch scene — it may not belong to this world"))?;

    Ok(GraphQLWorld::from(updated_world))
}

#[derive(Default)]
pub struct SceneMutation;

#[async_graphql::Object]
impl SceneMutation {
    async fn create_scene(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        // Creating a scene had **no membership check at all**: any signed-in
        // user could add a scene to any world by naming its id. Scene
        // authoring is a Game Master power, so it takes the same gate every
        // other content mutation takes.
        if !crate::auth::world_membership::is_dm_of_world(
            state,
            user_id,
            auth_user.is_admin,
            input.world_id,
        )
        .await?
        {
            return Err(Error::new("Forbidden"));
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = uuid::Uuid::now_v7();
        let world_id = input.world_id;
        let explicit_grid_type = input.grid_type;
        let name = input.name;
        let description = input.description;
        let type_ = input.type_.unwrap_or_else(|| "battlemap".to_string());
        let grid_size = input.grid_size.unwrap_or(5);
        let width = input.width.unwrap_or(100);
        let height = input.height.unwrap_or(100);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, worlds};
            use diesel::prelude::*;

            // Spec 022 (FR-015): a scene that doesn't explicitly choose a
            // grid type inherits the world's configured default instead of
            // always defaulting to "square" — the world row is already the
            // source of truth for this, no separate lookup table needed.
            let grid_type = match explicit_grid_type {
                Some(gt) => gt,
                None => worlds::table
                    .filter(worlds::id.eq(world_id))
                    .select(worlds::default_scene_grid_type)
                    .first::<String>(&mut conn)
                    .unwrap_or_else(|_| "square".to_string()),
            };

            let new_scene = crate::models::Scene {
                scene_id,
                world_id,
                name,
                description,
                type_,
                grid_size,
                grid_type,
                width,
                height,
                metadata,
                owner_id: user_id,
                created_at: now,
                updated_at: now,
                background_image_path: None,
                background_asset_id: None,
                summary_markdown: None,
                summary_rendered_html: None,
                // Spec 022 (FR-003, Clarifications): every newly created
                // scene starts hidden regardless of caller input — there is
                // no `hidden` field on this input type by design, so a
                // GM must explicitly un-hide via `updateSceneHidden`.
                hidden: true,
                preview_asset_id: None,
            };

            let values = (
                scenes::scene_id.eq(new_scene.scene_id),
                scenes::world_id.eq(new_scene.world_id),
                scenes::name.eq(&new_scene.name),
                scenes::description.eq(&new_scene.description),
                scenes::type_.eq(&new_scene.type_),
                scenes::grid_size.eq(new_scene.grid_size),
                scenes::grid_type.eq(&new_scene.grid_type),
                scenes::width.eq(new_scene.width),
                scenes::height.eq(new_scene.height),
                scenes::metadata.eq(&new_scene.metadata),
                scenes::owner_id.eq(new_scene.owner_id),
                scenes::created_at.eq(new_scene.created_at),
                scenes::updated_at.eq(new_scene.updated_at),
            );

            diesel::insert_into(scenes::table)
                .values(values)
                .returning(crate::models::Scene::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create scene"))?;

        Ok(GraphQLScene::from(inserted_scene))
    }

    async fn update_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
        input: GraphQLUpdateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let _now = Utc::now().naive_utc();

        let updated_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            // 🔐 A scene is content, so editing one follows the world role:
            // the Owner and any GM, not just whoever created the scene.
            // `updateSceneHidden` and `launchScene` next door already gate
            // this way — a GM who could hide and launch a scene but not
            // rename it was the inconsistency, not the fix.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
                return Err(diesel::result::Error::NotFound);
            }

            // Spec 022 (FR-006): summaryRenderedHtml is derived from
            // summaryMarkdown at write time (not on read, unlike lore
            // entries — scenes have no `[[link]]` resolution need, so
            // there's no staleness concern to justify computing it lazily).
            let summary_rendered_html = input
                .summary_markdown
                .as_deref()
                .map(crate::markdown::render_to_safe_html);

            let update_data = crate::models::SceneUpdate {
                name: input.name,
                description: input.description,
                grid_size: input.grid_size,
                grid_type: input.grid_type,
                width: input.width,
                height: input.height,
                metadata: input.metadata.map(|j| j.0),
                summary_markdown: input.summary_markdown,
                summary_rendered_html,
                hidden: None,
                preview_asset_id: None,
            };

            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set(update_data)
                .returning(crate::models::Scene::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update scene"))?;

        Ok(GraphQLScene::from(updated_scene))
    }

    /// Spec 022 (FR-007, FR-019). See `update_scene_hidden_impl`.
    async fn update_scene_hidden(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
        hidden: bool,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_scene_hidden_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            scene_id,
            hidden,
        )
        .await
    }

    /// Spec 022 (FR-002a/FR-002b/FR-002c, ADR-046). See `launch_scene_impl`.
    async fn launch_scene(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        launch_scene_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            scene_id,
        )
        .await
    }

    async fn delete_scene(&self, ctx: &Context<'_>, scene_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            // 🔐 Deleting a *scene* is a content act and follows the world
            // role. Deleting the *world* — and every other world-level
            // right — stays Owner-only and is gated elsewhere; a GM gains
            // nothing here beyond authority over the content of the world
            // they are running.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
                // Same answer an unauthorized caller got before: nothing
                // was deleted.
                return Ok(0);
            }

            diesel::delete(scenes::table.filter(scenes::scene_id.eq(scene_id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete scene"))?;

        Ok(deleted > 0)
    }

    // NOTE: scene-scoped token mutations (create_token/update_token/delete_token) live in
    // `mutations_tokens::TokenMutation` — that replaces an earlier `upsert_token`/`delete_token`
    // pair that lived here with no scene-ownership check at all. See TokenMutation for the
    // ownership-enforced, NOTIFY-synced replacement.

    async fn update_fog_mask(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpdateFogMaskInput,
    ) -> GraphQLResult<GraphQLFogMask> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        // Fog is what a Game Master uses to decide what a table can see, and
        // it had **no membership check at all** — any signed-in user could
        // write a mask onto any scene by naming its id. Reveal is the
        // dangerous direction: an attacker could uncover a map the GM was
        // deliberately keeping hidden, which is a spoiler at best and, on a
        // scene built around a secret, the whole session.
        let scene_id = input.scene_id;
        let is_admin = auth_user.is_admin;
        {
            let mut gate = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            let permitted = tokio::task::spawn_blocking(move || {
                crate::auth::world_membership::is_dm_of_scene(
                    &mut gate, user_id, is_admin, scene_id,
                )
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .unwrap_or(false);
            if !permitted {
                return Err(Error::new("Forbidden"));
            }
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = input.scene_id;
        let bitmap_data_base64 = input.bitmap_data_base64;
        let width = input.width;
        let height = input.height;

        let updated_fog_mask = tokio::task::spawn_blocking(move || {
            use crate::schema::fog_masks;
            use diesel::prelude::*;

            let bitmap_bytes = base64::engine::general_purpose::STANDARD
                .decode(&bitmap_data_base64)
                .map_err(|_| DieselError::NotFound)?;

            diesel::insert_into(fog_masks::table)
                .values((
                    fog_masks::fog_id.eq(uuid::Uuid::now_v7()),
                    fog_masks::scene_id.eq(scene_id),
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::created_at.eq(now),
                    fog_masks::updated_at.eq(now),
                ))
                .on_conflict(fog_masks::scene_id)
                .do_update()
                .set((
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(fog_masks::version + 1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::updated_at.eq(now),
                ))
                .returning(crate::models::FogMask::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update fog mask"))?;

        Ok(GraphQLFogMask::from(updated_fog_mask))
    }
}

// Query structs moved to queries modules (Phase 4.9.Z Step 5)

/// Spec 008 (US1, FR-004/FR-006): creates a world and its one default
/// scene in a single DB transaction — both succeed or both fail, so a
/// world can never exist with zero scenes through this path. Default
/// scene values mirror `create_scene`'s own defaults exactly
/// (data-model.md), inlined here rather than calling that resolver since
/// this needs to run inside the same transaction as the world insert.
/// Factored out of the `create_world` resolver (mirrors this codebase's
/// `_impl` convention, e.g. `mutations_assets.rs`'s
/// `upload_canvas_image_impl`) so it's directly unit-testable without a
/// full GraphQL execution context.
pub async fn create_world_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    input: GraphQLCreateWorldInput,
) -> Result<GraphQLWorld, String> {
    let prepared_input = prepare_world_input(input)?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    let new_world = World {
        id: uuid::Uuid::now_v7(),
        name: prepared_input.name,
        description: prepared_input.description,
        game_system_id: prepared_input.game_system_id,
        interface_pack_id: prepared_input.interface_pack_id,
        created_by: user_id,
        updated_by: user_id,
        created_at: now,
        updated_at: now,
        session_notes: None,
        allow_player_created_actors: false,
        genie_resource_carryover_enabled: false,
        default_scene_grid_type: "square".to_string(),
        active_scene_id: None,
    };

    let inserted_world = new_world.clone();
    let world_name_for_scene = new_world.name.clone();
    // Spec 022 (FR-002d, ADR-046): Play now shows an empty/unloaded canvas
    // whenever `worlds.active_scene_id` is null — but spec 010 (FR-004)
    // already guarantees every freshly created world has its default
    // scene ready to play immediately, with no separate "create a scene"
    // step. Reconciling the two: the default scene created here is also
    // immediately set as the world's active scene, so a brand-new world
    // is never stuck in the empty-canvas state; `active_scene_id` only
    // stays null for a world where nothing has ever been created/launched
    // (not reachable via normal world creation).
    let default_scene_id = uuid::Uuid::now_v7();
    let is_genie_world = inserted_world.game_system_id.as_deref() == Some("genie");
    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;

        conn.transaction(|conn| {
            diesel::insert_into(worlds::table)
                .values(&inserted_world)
                .execute(conn)?;

            let scene_values = (
                scenes::scene_id.eq(default_scene_id),
                scenes::world_id.eq(inserted_world.id),
                scenes::name.eq(&world_name_for_scene),
                scenes::description.eq::<Option<String>>(None),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::metadata.eq::<Option<serde_json::Value>>(None),
                scenes::owner_id.eq(user_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            );

            diesel::insert_into(scenes::table)
                .values(scene_values)
                .execute(conn)?;

            diesel::update(worlds::table.filter(worlds::id.eq(inserted_world.id)))
                .set(worlds::active_scene_id.eq(default_scene_id))
                .execute(conn)?;

            // Genie session UI (GenieSessionPanel.tsx) previously required
            // the GM to manually click "Start Genie session" before Wish
            // Pool/Doom Clock/grants became usable. Removed that manual
            // gate in favor of the session simply existing from world
            // creation on — doomClockMax 6 matches that button's prior
            // hardcoded default.
            if is_genie_world {
                let new_session = NewGenieSession {
                    world_id: inserted_world.id,
                    doom_clock_max: 6,
                    created_by: user_id,
                };
                diesel::insert_into(world_genie_sessions::table)
                    .values(&new_session)
                    .execute(conn)?;
            }

            Ok::<_, diesel::result::Error>(())
        })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|error| world_write_error(error, "Failed to create world").message)?;

    // NOTE: world creation does not insert a world_members owner row.
    // require_world_member() (src/server/src/auth/world_membership.rs,
    // spec 002) falls back to worlds.created_by to compensate for this
    // gap. See that module's doc comment for the full story — fixing it
    // at the source (inserting an owner world_members row here) is a
    // separate, deliberate follow-up, not done as part of this cleanup.
    let mut returned_world = new_world;
    returned_world.active_scene_id = Some(default_scene_id);
    Ok(GraphQLWorld::from(returned_world))
}

/// Spec 011: "Last Session Notes" — a single per-world freeform recap,
/// DM/GM-only to write (contracts/session-notes.md), read by any world
/// member via the existing `world(id)` query's `sessionNotes` field.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldSessionNotesInput {
    pub world_id: uuid::Uuid,
    pub notes: String,
}

/// Testable core of `WorldMutation::update_world_session_notes`, split out
/// so tests don't need a GraphQL `Context` (see `mutations_actors.rs`'s
/// `_impl` convention). DM/GM-only (FR-012). Saving an empty string is a
/// valid, explicit save (FR-013), not rejected as "no change".
pub async fn update_world_session_notes_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldSessionNotesInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may update session notes",
        ));
    }

    let world_id = input.world_id;
    let notes = input.notes;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::session_notes.eq(notes))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update session notes"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 016 (FR-004, T009): assigns/changes a world's active system pack.
/// No such mutation existed before this spec — `game_system_id` could
/// previously only be set at `createWorld` time (and spec 008 removed the
/// UI for that), leaving no way to assign or change it afterward. This is
/// the write half of the new System Settings surface
/// (`WorldSystemSettingsPage.tsx`) that also renders the target system's
/// `legal` notice, per this feature's scope-correction note in tasks.md.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldGameSystemInput {
    pub world_id: uuid::Uuid,
    pub game_system_id: String,
}

/// Testable core of `WorldMutation::update_world_game_system` (see
/// `update_world_session_notes_impl`'s identical shape/rationale).
/// DM/GM-only — mirrors `update_world_session_notes_impl`'s permission
/// check exactly, since assigning a world's ruleset is as GM-scoped a
/// decision as its session recap.
pub async fn update_world_game_system_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldGameSystemInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a world's game system",
        ));
    }

    let world_id = input.world_id;
    let game_system_id = input.game_system_id;
    if game_system_id.trim().is_empty() {
        return Err(Error::new("gameSystemId must not be empty"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::game_system_id.eq(Some(game_system_id)))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update game system"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 017 (FR-007): the GM-controlled world setting gating whether the
/// Actor Selection screen's "create your own character" option is shown.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldAllowPlayerCreatedActorsInput {
    pub world_id: uuid::Uuid,
    pub allow: bool,
}

/// Testable core of `WorldMutation::update_world_allow_player_created_actors`
/// (mirrors `update_world_session_notes_impl`'s shape/rationale exactly).
/// DM/GM-only.
pub async fn update_world_allow_player_created_actors_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldAllowPlayerCreatedActorsInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's player-created-actors setting",
        ));
    }

    let world_id = input.world_id;
    let allow = input.allow;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::allow_player_created_actors.eq(allow))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update allow_player_created_actors"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 020 (FR-003, research.md R1): the GM-controlled per-world setting
/// gating whether Genie Session Resource holdings carry over into the
/// next session.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldGenieResourceCarryoverInput {
    pub world_id: uuid::Uuid,
    pub enabled: bool,
}

/// Testable core of `WorldMutation::update_world_genie_resource_carryover`
/// (mirrors `update_world_allow_player_created_actors_impl`'s identical
/// shape/rationale). DM/GM-only.
pub async fn update_world_genie_resource_carryover_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldGenieResourceCarryoverInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's resource carryover setting",
        ));
    }

    let world_id = input.world_id;
    let enabled = input.enabled;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::genie_resource_carryover_enabled.eq(enabled))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update genie_resource_carryover_enabled"))?;

    Ok(GraphQLWorld::from(updated))
}

/// Spec 022 (FR-014): the GM-controlled per-world default grid type
/// applied to newly created scenes.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateWorldDefaultSceneGridTypeInput {
    pub world_id: uuid::Uuid,
    pub grid_type: String,
}

/// Testable core of `WorldMutation::update_world_default_scene_grid_type`
/// (mirrors `update_world_genie_resource_carryover_impl`'s identical
/// shape). DM/GM-only. `grid_type` is validated against the same set the
/// `scenes.grid_type`/`worlds.default_scene_grid_type` CHECK constraints
/// already enforce at the DB layer — this just turns a constraint
/// violation into a clean GraphQL error instead of a raw SQL error.
pub async fn update_world_default_scene_grid_type_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateWorldDefaultSceneGridTypeInput,
) -> GraphQLResult<GraphQLWorld> {
    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, input.world_id)
        .await?
    {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change this world's default scene grid type",
        ));
    }

    if !matches!(input.grid_type.as_str(), "square" | "hex" | "gridless") {
        return Err(Error::new(
            "gridType must be one of \"square\", \"hex\", \"gridless\"",
        ));
    }

    let world_id = input.world_id;
    let grid_type = input.grid_type;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::default_scene_grid_type.eq(grid_type))
            .returning(World::as_returning())
            .get_result::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update default_scene_grid_type"))?;

    Ok(GraphQLWorld::from(updated))
}

#[derive(Default)]
pub struct WorldMutation;

#[async_graphql::Object]
impl WorldMutation {
    async fn create_world(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_world_impl(state, auth_user.user_id, input)
            .await
            .map_err(Error::new)
    }

    async fn update_world_session_notes(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldSessionNotesInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_session_notes_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn update_world_game_system(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldGameSystemInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_game_system_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn update_world_allow_player_created_actors(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldAllowPlayerCreatedActorsInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_allow_player_created_actors_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn update_world_genie_resource_carryover(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldGenieResourceCarryoverInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_genie_resource_carryover_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn update_world_default_scene_grid_type(
        &self,
        ctx: &Context<'_>,
        input: UpdateWorldDefaultSceneGridTypeInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_world_default_scene_grid_type_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    async fn rename_world(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        world_name: String,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let world_name = normalize_world_name(&world_name);
        validate_world_name(&world_name).map_err(Error::new)?;
        let now = Utc::now().naive_utc();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let updated = tokio::task::spawn_blocking(move || {
            diesel::update(
                worlds::table
                    .filter(worlds::id.eq(world_id))
                    .filter(worlds::created_by.eq(user_id)),
            )
            .set((
                worlds::name.eq(world_name),
                worlds::updated_by.eq(user_id),
                worlds::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

            worlds::table
                .filter(worlds::id.eq(world_id))
                .select(World::as_select())
                .first::<World>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|error| world_write_error(error, "Failed to rename world"))?;

        match updated {
            Some(world) => Ok(GraphQLWorld::from(world)),
            None => Err(Error::new("Forbidden")),
        }
    }

    async fn delete_world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLDeleteWorldPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // Owner only, and this was a real hole: the check below is world
        // *membership*, so before this line existed any member — including a
        // Player who had merely accepted an invite — could delete the whole
        // world. Deleting is the one action with no way back, so it is the
        // one that most needs the narrow gate.
        //
        // Checked before the world is loaded, so a non-owner learns nothing
        // about a world they cannot act on beyond the fact of their own
        // membership, which they already knew.
        if !crate::auth::world_membership::is_owner_of_world(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            id,
        )
        .await?
        {
            return Err(Error::new("Forbidden"));
        }

        let existing = load_visible_world_by_id(state, auth_user.user_id, false, id).await?;

        let Some(world) = existing else {
            return Err(Error::new("World not found"));
        };

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            diesel::delete(worlds::table.filter(worlds::id.eq(id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete world"))?;

        Ok(GraphQLDeleteWorldPayload {
            id: world.id,
            status: "deleted".to_string(),
            message: format!("World '{}' was deleted", world.name),
        })
    }
}

#[derive(Default)]
pub struct UserDataMutation;

#[async_graphql::Object]
impl UserDataMutation {
    async fn delete_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLDeleteMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_user_data_owned(state, auth_user.user_id)
            .await
            .map(GraphQLDeleteMyDataPayload::from)
            .map_err(Error::new)
    }
}

#[derive(Default)]
pub struct AdminMutation;

#[async_graphql::Object]
impl AdminMutation {
    async fn update_oauth_provider(
        &self,
        ctx: &Context<'_>,
        provider_id: uuid::Uuid,
        config: GraphQLOAuthProviderConfigInput,
    ) -> GraphQLResult<GraphQLOAuthProvider> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_oauth_provider(state, provider_id, config.into())
            .await
            .map(GraphQLOAuthProvider::from)
            .map_err(Error::new)?;

        Ok(result)
    }

    async fn update_manifest_key(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_manifest_key(state, &key, &value)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)?;

        Ok(result)
    }

    async fn recalculate_disk_usage(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLAdminStats> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let stats = load_admin_stats(state).await.map_err(Error::new)?;
        let disk_usage = calculate_disk_usage(state).map_err(Error::new)?;

        Ok(GraphQLAdminStats {
            disk_usage_bytes: disk_usage.total_bytes,
            disk_usage: disk_usage.into(),
            total_users: stats.total_users,
            total_worlds: stats.total_worlds,
            total_world_tokens: stats.total_world_tokens,
            total_world_events: stats.total_world_events,
            total_policies: stats.total_policies,
        })
    }

    async fn update_two_factor_policy(
        &self,
        ctx: &Context<'_>,
        required_for_all_users: bool,
    ) -> GraphQLResult<GraphQLAuthSecuritySettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_two_factor_policy(state, required_for_all_users)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)?;

        Ok(result)
    }
}

/// Counters for the subscription hot path, kept instead of a log line per
/// event.
///
/// # Why this is not just tidiness
///
/// `eprintln!` takes a lock and issues a **blocking** `write(2)`. When stderr
/// is a pipe — which it is in every container, every CI harness and every
/// `cargo run | tee` — a consumer that stops reading for a moment fills the
/// 64KiB pipe buffer, and every one of those writes then blocks the thread it
/// is on until the reader comes back. These writes were happening on the
/// tokio worker threads that carry the subscriptions themselves, once per
/// event **per subscriber**, so a single slow log reader could stall the
/// whole fan-out at once.
///
/// That is not hypothetical: it is the mechanism behind the torture suite's
/// worst run. `scripts/marketing-metrics.mjs` reads the run's output through
/// a pipe and blocked its own event loop on a synchronous `docker stats`
/// every two seconds. With one line per event per subscriber the pipe filled,
/// the server's subscription tasks blocked in `write`, and 11 of 25
/// subscribers received nothing at all — with no panic, no error and no
/// timeout anywhere, because nothing was broken, only stopped. The identical
/// tier run through a file instead of that pipe passed 5/5.
///
/// So the hot path counts and the periodic reporter in
/// `network::listener` prints the totals once every ten seconds. Bounded log
/// volume is the property that matters here, not brevity: a diagnostic that
/// can stop delivery is worse than no diagnostic.
pub mod subscription_metrics {
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

    /// Events handed to a subscriber's socket.
    pub static DELIVERED: AtomicU64 = AtomicU64::new(0);
    /// Subscriptions established.
    pub static OPENED: AtomicU64 = AtomicU64::new(0);
    /// Subscriptions refused (no app state, bad id, not a member).
    pub static REFUSED: AtomicU64 = AtomicU64::new(0);
    /// Events a subscriber lost by falling behind the broadcast buffer.
    pub static LAGGED_EVENTS: AtomicU64 = AtomicU64::new(0);
    /// WebSocket connections currently being served.
    ///
    /// Live rather than cumulative on purpose. "How many sockets are attached
    /// right now" is the number that separates *the server stopped sending*
    /// from *the clients went away*, and telling those two apart is what the
    /// worst delivery investigation in this repository spent its time on.
    pub static SOCKETS_OPEN: AtomicI64 = AtomicI64::new(0);

    static SINCE: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    static LAST_LAG_LOG_MS: AtomicU64 = AtomicU64::new(0);

    /// Whether to print a lag line now, at most one every ten seconds.
    ///
    /// Lag is worth a sentence in the log — it means a client's view of the
    /// world is wrong — but it is not worth one per event: a subscriber that
    /// has wedged lags on *every* subsequent event, which is exactly the
    /// runaway volume this module exists to prevent. The count in the
    /// periodic report is the complete number; the line is there so somebody
    /// grepping finds it at all.
    pub fn should_log_lag() -> bool {
        let now = SINCE.elapsed().as_millis() as u64;
        let last = LAST_LAG_LOG_MS.load(Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < 10_000 {
            return false;
        }
        LAST_LAG_LOG_MS
            .compare_exchange(last, now.max(1), Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The cap has to hold for a subscriber that lags on every event.
        ///
        /// This is the shape that made the unrate-limited version dangerous:
        /// a wedged client does not lag once, it lags on everything that
        /// arrives afterwards, so "one line per lag" is one blocking write to
        /// stderr per event for as long as it stays wedged — the same runaway
        /// volume, arriving by a different door.
        #[test]
        fn the_lag_line_is_capped_however_many_times_lag_is_reported() {
            // The first report is always worth printing; the flood behind it
            // is not.
            assert!(should_log_lag(), "the first lag must be findable");
            let printed = (0..10_000).filter(|_| should_log_lag()).count();
            assert_eq!(
                printed, 0,
                "ten thousand further lag reports inside the window must \
                 print nothing; {printed} got through",
            );
        }
    }

    /// `(sockets_open, opened, refused, delivered, lagged_events)`.
    pub fn snapshot() -> (i64, u64, u64, u64, u64) {
        (
            SOCKETS_OPEN.load(Ordering::Relaxed),
            OPENED.load(Ordering::Relaxed),
            REFUSED.load(Ordering::Relaxed),
            DELIVERED.load(Ordering::Relaxed),
            LAGGED_EVENTS.load(Ordering::Relaxed),
        )
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;

/// Whether this subscriber may see this world at all.
///
/// Extracted because it was written once, for `world_events_created`, and
/// then simply not written for `players_online` — which subscribed anyone
/// who could name a world id. Two subscriptions over the same world data
/// must not be able to disagree about who may watch it, and the way to
/// guarantee that is for there to be one check rather than two.
///
/// Answers `false` for every failure — no app state, no session, a pool
/// error, a database error. A subscription is a long-lived grant of
/// access; refusing one because we could not confirm entitlement is the
/// safe direction, and the client's own retry covers the transient case.
async fn may_watch_world(
    ctx: &Context<'_>,
    app_state: &Option<AppState>,
    world_uuid: &Option<uuid::Uuid>,
) -> bool {
    match (app_state, world_uuid) {
        (Some(state), Some(uuid)) => match authenticated_user(ctx) {
            Ok(auth_user) => {
                let user_id = auth_user.user_id;
                let world_uuid = *uuid;
                let pool = state.db_pool.clone();
                tokio::task::spawn_blocking(move || {
                    pool.get()
                        .ok()
                        .and_then(|mut conn| {
                            require_world_member(&mut conn, user_id, world_uuid).ok()
                        })
                        .is_some()
                })
                .await
                .unwrap_or(false)
            }
            Err(_) => false,
        },
        _ => false,
    }
}

#[Subscription]
impl SubscriptionRoot {
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut value = 0;
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(1))),
            move |_| {
                value += 1;
                value
            },
        )
    }

    /// Subscribe to world events (tokens, actors, scenes, etc.)
    ///
    /// Phase 4.9.A.2: Real-time event streaming via PostgreSQL pub/sub backplane
    /// Phase 4.9.A.3: Backpressure handling for lagged subscribers
    ///
    /// All subscribers receive events broadcast from the database listener task.
    /// Events are sent immediately as they are recorded in world_events table.
    ///
    /// If a client falls behind (buffer fills), the subscription will stop receiving
    /// events until it catches up. This is graceful degradation under load.
    async fn world_events_created(
        &self,
        ctx: &Context<'_>,
        world_id: String,
    ) -> impl Stream<Item = Result<GraphQLWorldEvent, Error>> {
        use std::pin::Pin;

        let app_state = ctx.data::<AppState>().ok().cloned();
        let world_uuid = uuid::Uuid::parse_str(&world_id).ok();

        // Authorization: this previously had none at all — any authenticated
        // user could subscribe to any world's events by guessing a world_id,
        // bypassing per-world membership entirely.
        let membership_ok = may_watch_world(ctx, &app_state, &world_uuid).await;

        // Collect all validation to happen upfront
        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (_, _) if !membership_ok => (true, "You must be a member of this world", None),
            (Some(app_state), Some(world_uuid)) => {
                // This world's channel, not the whole process's. The stream
                // below no longer filters, because nothing else can arrive.
                (
                    false,
                    "",
                    Some(app_state.world_events.subscribe(*world_uuid)),
                )
            }
        };

        // Counted, not logged. A subscription storm is 25 of these inside a
        // second, and the churn test opens 155 — see `subscription_metrics`
        // for why a line each is a way to stop delivery rather than a way to
        // observe it. The refusal keeps its line: it is rare, and it is the
        // one that someone has to be able to find.
        if has_error {
            subscription_metrics::REFUSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "[GraphQL Subscription] 🚫 Refused a subscription to world_id={world_id}: \
                 {error_msg}"
            );
        } else {
            subscription_metrics::OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Create a combined stream that works for both cases
        // Return type is Pin<Box<dyn Stream>> for type erasure
        if let Some(rx) = rx_opt {
            // Success case: stream this world's channel. The id is no longer
            // needed to *filter* — the receiver is the filter now — but the
            // lag diagnostic below still names the world it lost events for.
            let world_uuid = world_uuid.unwrap();

            let stream =
                tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| {
                    match result {
                        Ok(event) => {
                            // No world check. The receiver is this world's
                            // channel, so an event arriving here is ours by
                            // construction — the old
                            // `if event.world_id == world_uuid` was the
                            // per-subscriber half of a fan-out that woke every
                            // client in the process for every event and had
                            // each of them throw away what was not theirs.
                            //
                            // Counted rather than logged: this runs once per
                            // event per subscriber, and `eprintln!` here is a
                            // blocking write on the task that carries the
                            // subscription. See `subscription_metrics`.
                            subscription_metrics::DELIVERED
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            Some(Ok(GraphQLWorldEvent::from(event)))
                        }
                        // The only error `BroadcastStream` yields is
                        // `Lagged(n)`: this receiver fell far enough behind
                        // that the channel overwrote `n` messages it had not
                        // read. Those events are **gone for this client** —
                        // no retry, no backfill, and the stream continues as
                        // if nothing happened.
                        //
                        // Dropping it to `None` is still the right stream
                        // behaviour (ending the subscription would be worse
                        // than missing an event), but it must not be quiet
                        // about *how many*. This previously logged the error
                        // with `{:?}` and no count in the message, which read
                        // as a transient warning rather than "this client's
                        // view of the world is now wrong".
                        //
                        // The client's recovery is the world sync it performs
                        // on open; there is no resync signal on this wire yet,
                        // which is precisely why the log has to be findable.
                        //
                        // Every one of them is counted; the line itself is
                        // capped at one every ten seconds, because a wedged
                        // subscriber lags on every event after the first and
                        // an uncapped line here is a way to stall the very
                        // fan-out it is reporting on.
                        Err(BroadcastStreamRecvError::Lagged(missed)) => {
                            subscription_metrics::LAGGED_EVENTS
                                .fetch_add(missed, std::sync::atomic::Ordering::Relaxed);
                            if subscription_metrics::should_log_lag() {
                                eprintln!(
                                    "[GraphQL Subscription] ⚠️  DROPPED {missed} event(s) for a \
                                     subscriber of world {world_uuid}: it fell behind the \
                                     broadcast buffer. Those events will never be delivered to \
                                     it. (Further lag lines are suppressed for 10s; the running \
                                     total is in the [PubSub] metrics line.)"
                                );
                            }
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        }
    }

    /// Spec 028 (T086): receive WebRTC signaling addressed to this session.
    ///
    /// The stream **is** the registration. It begins when this subscription
    /// is established and ends when it drops, which is what confines peer
    /// connections to the session (FR-050) without a cleanup job that could
    /// be skipped on a crash.
    ///
    /// `sessionId` is a deliberate, minimal extension to the contract's SDL:
    /// a client cannot be reachable without naming the address it wants to be
    /// reachable at, and `PeerSignal` carries no field that could tell it one.
    /// The server treats the value as opaque and forgets it on disconnect.
    async fn peer_signals(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        session_id: String,
    ) -> impl Stream<Item = Result<crate::peer_signaling::GraphQLPeerSignal, Error>> {
        crate::peer_signaling::peer_signals_stream(ctx, world_id, session_id).await
    }

    /// Subscribe to player presence changes (Phase 4.9.B.3)
    ///
    /// Streams updates when players connect, disconnect, or change scenes.
    /// Returns current list of all online players in the world.
    async fn players_online(
        &self,
        ctx: &Context<'_>,
        world_id: String,
    ) -> impl Stream<Item = Result<GraphQLPlayersOnlineList, Error>> {
        use std::pin::Pin;

        let app_state = ctx.data::<AppState>().ok().cloned();
        let world_uuid = uuid::Uuid::parse_str(&world_id).ok();

        // The same gate `world_events_created` uses, which this subscription
        // did not have: it accepted anyone who could name a world id. Harmless
        // only for as long as the payload below stays empty — which is exactly
        // the kind of "safe because unfinished" that stops being true the day
        // someone finishes it.
        let membership_ok = may_watch_world(ctx, &app_state, &world_uuid).await;

        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (_, _) if !membership_ok => (true, "You must be a member of this world", None),
            (Some(app_state), Some(_)) => (false, "", Some(app_state.presence_sender.subscribe())),
        };

        // Same reasoning as `world_events_created`: counted, not logged, and
        // only the refusal is worth a line. See `subscription_metrics`.
        if has_error {
            subscription_metrics::REFUSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            eprintln!(
                "[GraphQL Subscription] 🚫 Refused a presence subscription to \
                 world_id={world_id}: {error_msg}"
            );
        } else {
            subscription_metrics::OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        if let (Some(rx), Some(world_id_uuid)) = (rx_opt, world_uuid) {
            // Success case: emit presence notifications
            // Note: This is a simple implementation that emits on each presence event.
            // In production, you'd query the DB to get the full player list on each event.
            let stream =
                tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| {
                    match result {
                        Ok(_presence_event) => {
                            subscription_metrics::DELIVERED
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            // Still a stub. When this is wired up, read it
                            // from `AppState::presence` — presence lives in
                            // memory now, and the `players_online` table is
                            // no longer written on each heartbeat. A helper
                            // that queried that table used to sit below this
                            // file, marked `#[allow(dead_code)]` and waiting
                            // for a resolver; it was removed rather than left
                            // pointing at a table nothing fills.
                            Some(Ok(GraphQLPlayersOnlineList {
                                world_id: world_id_uuid,
                                players: vec![],
                            }))
                        }
                        // Same as the world-event stream above: `Lagged(n)`
                        // means n presence updates were overwritten before
                        // this subscriber read them. Less costly than a lost
                        // world event — presence is a snapshot, and the next
                        // update supersedes the ones missed — but the count
                        // is still the difference between "a blip" and "this
                        // client is minutes stale".
                        Err(BroadcastStreamRecvError::Lagged(missed)) => {
                            subscription_metrics::LAGGED_EVENTS
                                .fetch_add(missed, std::sync::atomic::Ordering::Relaxed);
                            if subscription_metrics::should_log_lag() {
                                eprintln!(
                                    "[GraphQL Subscription] ⚠️  DROPPED {missed} presence \
                                     update(s) for a subscriber: it fell behind the broadcast \
                                     buffer."
                                );
                            }
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        }
    }

    /// Subscribe to actor system data changes (D&D 5e, Pathfinder, CoC, etc.)
    ///
    /// PHASE D.2 STUB: This subscription will stream actor system data updates
    /// from the pg_notify backplane when client subscribes.
    /// Full implementation pending async database driver integration.
    ///
    /// For now, returns a tick stream that can be tested.
    async fn world_actor_system_data_updated(
        &self,
        _ctx: &Context<'_>,
        _world_id: String,
        _game_system_id: String,
    ) -> impl Stream<Item = GraphQLResult<GraphQLActorSystemDataEvent>> {
        // STUB: Return a placeholder stream
        // In production, this would listen to pg_notify and stream real events
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(10))),
            |_| {
                Ok(GraphQLActorSystemDataEvent {
                    id: uuid::Uuid::new_v4(),
                    actor_id: uuid::Uuid::new_v4(),
                    game_system_id: "dnd5e".to_string(),
                    event_type: "UPDATE".to_string(),
                    ability_data: None,
                    resource_data: None,
                    proficiency_data: None,
                    trait_data: None,
                    spell_data: None,
                    updated_at: chrono::Local::now().naive_utc(),
                })
            },
        )
    }
}

// Empty placeholder in the mutation root — the world_collaborators-based
// RBAC mutations this was meant to hold were never built; world/scene
// authorization instead runs through world_members (see
// src/server/src/auth/world_membership.rs).
#[derive(async_graphql::MergedObject, Default)]
pub struct CollaboratorMutation;

#[derive(MergedObject, Default)]
pub struct QueryRoot(
    PresenceQuery,
    HealthcheckQuery,
    UserQuery,
    AdminQuery,
    SceneQuery,
    queries::token_status::TokenStatusQuery,
    queries::token_attributes::TokenAttributesQuery,
    // Spec 030: `effectRegistry` and `interactives(sceneId)`.
    queries::interactives::InteractiveQuery,
    // Spec 031: `authoringTools(worldId)` — which tools the caller may use.
    queries::AuthoringToolsQuery,
    InviteQuery,
    AssetQuery,
    ActorQuery,
    ActorPermissionQuery,
    ActorShareQuery,
    LoreQuery,
    LorePermissionQuery,
    AbilityQuery,
    AbilityPermissionQuery,
    AbilityShareQuery,
    ActorAbilityQuery,
    ItemQuery,
    ItemPermissionQuery,
    ItemShareQuery,
    InventoryQuery,
    ModerationQuery,
    RollQuery,
    ActorClaimQuery,
    GenieSessionQuery,
    ChatQuery,
    CombatQuery,
    // Spec 028: `worldSyncPlan` — what a returning client must fetch and
    // discard for one world.
    WorldSyncPlanQuery,
    // `worldEventsSince` — what a client missed while its socket was down.
    // Live delivery is at-most-once by construction, so the durable record is
    // what a reconnecting client asks, not the wire it just lost.
    WorldEventsSinceQuery,
    // Spec 028 (T086): `peerSessions` — who else is reachable right now.
    crate::peer_signaling::PeerSignalingQuery,
);

#[derive(MergedObject, Default)]
pub struct MutationRoot(
    queries::token_status::TokenDisclosureMutation,
    WorldMutation,
    UserDataMutation,
    AdminMutation,
    SceneMutation,
    WorldTokenMutation,
    ActorSystemDataMutation,
    CollaboratorMutation,
    InviteMutation,
    WallMutation,
    LightSourceMutation,
    ShapeMutation,
    // Spec 030: authoring, activation and approval for interactive elements.
    mutations_interactives::InteractiveMutation,
    TokenMutation,
    AssetMutation,
    ActorMutation,
    ActorPermissionMutation,
    ActorShareMutation,
    mutations_actor_images::ActorImageMutation,
    LoreMutation,
    LorePermissionMutation,
    LoreImageMutation,
    mutations_lore_tree::LoreTreeMutation,
    AbilityMutation,
    AbilityPermissionMutation,
    AbilityShareMutation,
    ActorAbilityMutation,
    ItemMutation,
    ItemPermissionMutation,
    ItemShareMutation,
    mutations_item_prices::ItemPriceMutation,
    InventoryMutation,
    PickupMutation,
    PartyMutation,
    ModerationMutation,
    RollMutation,
    GenieSessionMutation,
    ActorClaimMutation,
    ChatMutation,
    CombatMutation,
    ReconcileMutation,
    HeartbeatMutation,
    // Spec 028 (T086): `sendPeerSignal` — the post box.
    crate::peer_signaling::PeerSignalingMutation,
);

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

#[cfg(test)]
mod tests {
    use super::{
        GraphQLCreateWorldInput, create_world_impl, prepare_world_input, validate_world_name,
    };

    /// Spec 008 (T022, FR-004/FR-006): `create_world` must always yield
    /// exactly one scene — never zero — since the whole point of this
    /// feature is that a freshly created world's canvas has content on it
    /// immediately, with no separate "create a scene" step.
    #[tokio::test]
    async fn create_world_always_yields_exactly_one_scene() {
        use crate::schema::scenes;
        use crate::test_support::*;
        use diesel::prelude::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let world = create_world_impl(
            &state,
            user_id,
            GraphQLCreateWorldInput {
                name: "The Ember Crown".to_string(),
                description: None,
                game_system_id: None,
                interface_pack_id: None,
            },
        )
        .await
        .expect("world creation should succeed");

        let mut conn = state.db_pool.get().unwrap();
        let scene_count = scenes::table
            .filter(scenes::world_id.eq(world.id))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("scene count query should succeed");

        assert_eq!(
            scene_count, 1,
            "create_world must always produce exactly one default scene"
        );
    }

    /// Spec 008 (T022): an invalid world name must fail validation
    /// *before* any DB write happens — confirming create_world_impl's
    /// early-return on prepare_world_input's error leaves nothing
    /// persisted (no orphaned world, no orphaned scene) for a rejected
    /// input, the same "both succeed or both fail" guarantee research.md
    /// §1 describes for the transaction itself.
    #[tokio::test]
    async fn create_world_rejects_invalid_name_before_any_write() {
        use crate::schema::worlds;
        use crate::test_support::*;
        use diesel::prelude::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        // Scoped to this test's own throwaway user, NOT a global world count.
        // A global count is not isolation-safe: any concurrently-running test
        // that creates a world lands between the two reads and fails this
        // assertion spuriously. Scoping preserves the intent exactly — "this
        // rejected call wrote nothing" — while being immune to neighbours.
        let before_count = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("world count query should succeed");
        drop(conn);

        let result = create_world_impl(
            &state,
            user_id,
            GraphQLCreateWorldInput {
                name: "ab".to_string(), // below MIN_WORLD_NAME_LEN
                description: None,
                game_system_id: None,
                interface_pack_id: None,
            },
        )
        .await;

        assert!(result.is_err(), "a too-short name must be rejected");

        let mut conn = state.db_pool.get().unwrap();
        let after_count = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("world count query should succeed");
        assert_eq!(
            before_count, after_count,
            "a rejected create_world call must not write a world row"
        );
    }

    #[test]
    fn world_name_validation_rejects_invalid_characters() {
        let result = validate_world_name("Bad@World");

        assert_eq!(
            result,
            Err(
                "World name may only contain letters, numbers, spaces, apostrophes, and - _ . , : ! ? ( )"
                    .to_string(),
            )
        );
    }

    #[test]
    fn prepare_world_input_trims_optional_fields() {
        let prepared = prepare_world_input(GraphQLCreateWorldInput {
            name: "  The   Ember   Crown  ".to_string(),
            description: Some("  A fallen kingdom  ".to_string()),
            game_system_id: Some("  systemless-sandbox ".to_string()),
            interface_pack_id: Some(" guild-hall-default ".to_string()),
        })
        .expect("world input should be valid");

        assert_eq!(prepared.name, "The Ember Crown");
        assert_eq!(prepared.description.as_deref(), Some("A fallen kingdom"));
        assert_eq!(
            prepared.game_system_id.as_deref(),
            Some("systemless-sandbox")
        );
        assert_eq!(
            prepared.interface_pack_id.as_deref(),
            Some("guild-hall-default")
        );
    }

    // Phase 1.4: Security test - validate_world_name rejects XSS attempts
    #[test]
    fn world_name_validation_rejects_xss_attempts() {
        let xss_attempts = vec![
            "<script>alert('xss')</script>",
            "World<img src=x onerror=alert('xss')>",
            "'; DROP TABLE worlds; --",
            "World\x00Null",
        ];

        for xss in xss_attempts {
            let result = validate_world_name(xss);
            assert!(result.is_err(), "Should reject XSS attempt: {}", xss);
        }
    }

    // Phase 1.4: Security test - world name length limits
    #[test]
    fn world_name_validation_enforces_length_limits() {
        // Valid: 64 characters (MAX_WORLD_NAME_LEN)
        let valid = "A".repeat(64);
        assert!(
            validate_world_name(&valid).is_ok(),
            "64 chars should be valid"
        );

        // Invalid: 65+ characters
        let invalid = "A".repeat(65);
        assert!(
            validate_world_name(&invalid).is_err(),
            "65+ chars should be rejected"
        );

        // Invalid: 2 characters (MIN_WORLD_NAME_LEN is 3)
        let too_short = "AB";
        assert!(
            validate_world_name(too_short).is_err(),
            "2 chars should be rejected"
        );

        // Valid: 3 characters (MIN_WORLD_NAME_LEN)
        let min_valid = "ABC";
        assert!(
            validate_world_name(min_valid).is_ok(),
            "3 chars should be valid"
        );
    }

    // Phase 1.4: Security test - prepare_world_input rejects empty name
    #[test]
    fn prepare_world_input_rejects_empty_name() {
        let result = prepare_world_input(GraphQLCreateWorldInput {
            name: "  \t\n  ".to_string(), // Only whitespace
            description: None,
            game_system_id: None,
            interface_pack_id: None,
        });

        assert!(result.is_err(), "Should reject empty/whitespace-only name");
    }

    // Phase 1.4: Security test - validate special characters are allowed (D&D names)
    #[test]
    fn world_name_validation_allows_dnd_style_names() {
        let valid_names = vec![
            "The Forgotten Realms",
            "Dragonlance: Time of Legend",
            "Spelljammer (Far Realm)",
            "Ravenloft's Dark Masters",
        ];

        for name in valid_names {
            assert!(
                validate_world_name(name).is_ok(),
                "Should allow D&D-style name: {}",
                name
            );
        }
    }

    // Spec 011: World Session Notes (contracts/session-notes.md)

    #[tokio::test]
    async fn dm_can_update_session_notes_and_read_it_back() {
        use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let updated = update_world_session_notes_impl(
            &state,
            owner_id,
            false,
            UpdateWorldSessionNotesInput {
                world_id,
                notes: "The party defeated the goblin ambush and pressed on.".to_string(),
            },
        )
        .await
        .expect("the DM should be able to update session notes");

        assert_eq!(
            updated.session_notes.as_deref(),
            Some("The party defeated the goblin ambush and pressed on.")
        );
    }

    #[tokio::test]
    async fn saving_empty_session_notes_succeeds() {
        use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let updated = update_world_session_notes_impl(
            &state,
            owner_id,
            false,
            UpdateWorldSessionNotesInput {
                world_id,
                notes: "".to_string(),
            },
        )
        .await
        .expect("saving an explicit empty value must not error");

        assert_eq!(updated.session_notes.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn player_role_cannot_update_session_notes() {
        use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = update_world_session_notes_impl(
            &state,
            player_id,
            false,
            UpdateWorldSessionNotesInput {
                world_id,
                notes: "Should not be saved".to_string(),
            },
        )
        .await;

        assert!(
            result.is_err(),
            "a Player-role world member must not be able to update session notes"
        );
    }

    #[tokio::test]
    async fn non_member_cannot_update_session_notes() {
        use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let result = update_world_session_notes_impl(
            &state,
            outsider_id,
            false,
            UpdateWorldSessionNotesInput {
                world_id,
                notes: "Should not be saved".to_string(),
            },
        )
        .await;

        assert!(
            result.is_err(),
            "a user with no relationship to the world must not be able to update session notes"
        );
    }

    // Spec 016: World System Assignment (T009)

    #[tokio::test]
    async fn dm_can_assign_a_world_game_system() {
        use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let updated = update_world_game_system_impl(
            &state,
            owner_id,
            false,
            UpdateWorldGameSystemInput {
                world_id,
                game_system_id: "dnd5e".to_string(),
            },
        )
        .await
        .expect("the DM should be able to assign a game system");

        assert_eq!(updated.game_system_id.as_deref(), Some("dnd5e"));
    }

    #[tokio::test]
    async fn assigning_an_empty_game_system_id_is_rejected() {
        use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = update_world_game_system_impl(
            &state,
            owner_id,
            false,
            UpdateWorldGameSystemInput {
                world_id,
                game_system_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err(), "an empty gameSystemId must be rejected");
    }

    #[tokio::test]
    async fn player_role_cannot_assign_a_world_game_system() {
        use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = update_world_game_system_impl(
            &state,
            player_id,
            false,
            UpdateWorldGameSystemInput {
                world_id,
                game_system_id: "dnd5e".to_string(),
            },
        )
        .await;

        assert!(
            result.is_err(),
            "a Player-role world member must not be able to change the world's game system"
        );
    }

    // ===== Spec 022: Scene Management Overhaul =====

    #[tokio::test]
    async fn update_scene_hidden_requires_dm_role() {
        use super::update_scene_hidden_impl;
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = update_scene_hidden_impl(&state, player_id, false, scene_id, false).await;
        assert!(
            result.is_err(),
            "a Player-role world member must not be able to toggle a scene's hidden state"
        );

        let updated = update_scene_hidden_impl(&state, owner_id, false, scene_id, false)
            .await
            .expect("the DM (Owner) toggling hidden should succeed");
        assert!(!updated.hidden, "hidden should now be false");
    }

    #[tokio::test]
    async fn launch_scene_requires_dm_role_and_rejects_cross_world_scene() {
        use super::launch_scene_impl;
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");

        // A second, unrelated world/scene pair — launching this scene into
        // the first world must be rejected (FR-002b implicitly assumes
        // same-world).
        let other_owner_id = insert_test_user(&mut conn);
        let other_world_id = insert_test_world(&mut conn, other_owner_id);
        let other_scene_id = insert_test_scene(&mut conn, other_world_id, other_owner_id);
        drop(conn);

        let player_result = launch_scene_impl(&state, player_id, false, world_id, scene_id).await;
        assert!(
            player_result.is_err(),
            "a Player-role world member must not be able to launch a scene"
        );

        let cross_world_result =
            launch_scene_impl(&state, owner_id, false, world_id, other_scene_id).await;
        assert!(
            cross_world_result.is_err(),
            "launching a scene that belongs to a different world must be rejected"
        );

        let updated_world = launch_scene_impl(&state, owner_id, false, world_id, scene_id)
            .await
            .expect("the DM launching an in-world scene should succeed");
        assert_eq!(
            updated_world.active_scene_id,
            Some(scene_id),
            "active_scene_id should now be the launched scene"
        );
    }

    #[tokio::test]
    async fn update_world_default_scene_grid_type_requires_dm_role_and_valid_value() {
        use super::{
            UpdateWorldDefaultSceneGridTypeInput, update_world_default_scene_grid_type_impl,
        };
        use crate::test_support::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let player_result = update_world_default_scene_grid_type_impl(
            &state,
            player_id,
            false,
            UpdateWorldDefaultSceneGridTypeInput {
                world_id,
                grid_type: "hex".to_string(),
            },
        )
        .await;
        assert!(
            player_result.is_err(),
            "a Player-role world member must not be able to change the default scene grid type"
        );

        let invalid_result = update_world_default_scene_grid_type_impl(
            &state,
            owner_id,
            false,
            UpdateWorldDefaultSceneGridTypeInput {
                world_id,
                grid_type: "triangles".to_string(),
            },
        )
        .await;
        assert!(
            invalid_result.is_err(),
            "an invalid gridType must be rejected"
        );

        let updated = update_world_default_scene_grid_type_impl(
            &state,
            owner_id,
            false,
            UpdateWorldDefaultSceneGridTypeInput {
                world_id,
                grid_type: "hex".to_string(),
            },
        )
        .await
        .expect("the DM setting a valid gridType should succeed");
        assert_eq!(updated.default_scene_grid_type, "hex");
    }

    // Note: `create_scene`'s "inherit world.default_scene_grid_type when
    // gridType is omitted" behavior (FR-015) is exercised end-to-end by
    // the Playwright e2e spec (apps/web/e2e/scene-default-grid-type.spec.ts)
    // instead of a unit test here — `create_scene` is an inline
    // `#[Object]` method (pre-existing, not extracted to a testable
    // `_impl` by this feature) whose GraphQL context is impractical to
    // construct in a focused unit test without also duplicating the
    // full `MutationRoot`/`QueryRoot` wiring.
}
