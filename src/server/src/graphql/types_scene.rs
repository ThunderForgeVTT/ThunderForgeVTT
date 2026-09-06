//! What a scene and the things standing on it look like over the wire.
//!
//! Split out of `graphql.rs`: scenes, walls, lighting, shapes, tokens, fog
//! and the actors and members attached to a world. Data declarations and the
//! `From<model>` conversions that feed them, nothing that decides anything.

use async_graphql::{Context, Json, Result as GraphQLResult, SimpleObject};

use super::*;
use crate::models::WorldActor;

// ========== Phase 3.5: Scene System GraphQL Types ==========

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLScene {
    pub scene_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    #[graphql(name = "type")]
    pub type_: String,
    pub grid_size: i32,
    pub grid_type: String,
    pub width: i32,
    pub height: i32,
    pub metadata: Option<Json<serde_json::Value>>,
    pub owner_id: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Native canvas authoring: set by map import (data-model.md's Scene
    /// section); resolves against the existing `/assets/<path>` static
    /// route. `None` = no background art. Superseded by
    /// `background_asset_id` (spec 002, FR-018).
    pub background_image_path: Option<String>,
    /// Spec 002 (FR-018): the RustFS-backed `canvas_image_assets` row
    /// for this scene's background, when migrated.
    pub background_asset_id: Option<uuid::Uuid>,
    /// Bug fix (found while investigating "map import doesn't take the
    /// image"): `background_asset_id` alone is a bare UUID with no
    /// fetchable URL — the frontend's canvas-loading path
    /// (`WorldPage.tsx`) only ever read `background_image_path`, which
    /// dd2vtt import (spec 022/002, `save_background_image`) never sets,
    /// since it writes to RustFS via `background_asset_id` instead. This
    /// computed field is the fetchable URL for whichever mechanism is
    /// actually populated (`background_asset_id` preferred — RustFS via
    /// `assets_serve/canvas.rs`'s existing `GET /canvas-assets/{id}`
    /// route, already used by `AssetPasteTool`; falls back to the legacy
    /// `background_image_path` static-file route for scenes never
    /// migrated to RustFS), mirroring `preview_url`'s existing pattern.
    pub background_url: Option<String>,
    /// Spec 022: GM-authored Markdown source for the scene's player-facing
    /// summary.
    pub summary_markdown: Option<String>,
    /// Spec 022: sanitized HTML rendered from `summary_markdown` — render
    /// this, never `summary_markdown` directly.
    pub summary_rendered_html: Option<String>,
    /// Spec 022 (FR-003, Clarifications): player-facing visibility, hidden
    /// by default on creation.
    pub hidden: bool,
    /// Spec 022 (FR-011): computed URL for the scene's reduced-size
    /// preview image, `None` until one has been generated.
    pub preview_url: Option<String>,
    /// Why this scene's grid does not match the background under it, or
    /// `None` when they agree.
    ///
    /// A sentence rather than a flag, because the useful thing to show is the
    /// two numbers that differ — and computed here rather than in the client
    /// so the rule has one home. See `map_import::alignment`.
    pub background_grid_mismatch: Option<String>,
}

impl From<crate::models::Scene> for GraphQLScene {
    fn from(scene: crate::models::Scene) -> Self {
        // Computed first: the struct literal below moves the fields it reads.
        let background_grid_mismatch = crate::map_import::alignment::grid_mismatch(
            scene.background_asset_id.is_some() || scene.background_image_path.is_some(),
            scene.width,
            scene.height,
            scene.grid_size,
            scene.metadata.as_ref(),
        );

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
            // `assets_serve::canvas::router()`/`assets_serve::scene::router()`)
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
            // object is WebP — see `assets_serve::canvas::parse_asset_id`,
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
            background_grid_mismatch,
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
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub actor_type: String,
    pub game_system_id: Option<String>,
    pub label: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub is_npc: bool,
    pub created_by: uuid::Uuid,
    pub owned_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Spec 017 (FR-004): GM-set flag offering this (PC-only) actor to a
    /// joining player on the Actor Selection screen.
    pub available_for_claim: bool,
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
        let rows =
            crate::graphql::mutations_actor_images::actor_images_impl(state, self.id).await?;
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
