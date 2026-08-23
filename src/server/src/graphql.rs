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

use crate::admin::{
    load_admin_stats, recalculate_disk_usage as calculate_disk_usage,
    update_manifest_key as persist_manifest_key, update_oauth_provider as persist_oauth_provider,
    update_two_factor_policy as persist_two_factor_policy,
};
use crate::models::{
    World,
    WorldActor,
    // Policy - disabled pending schema
};
use crate::schema::{world_actors, worlds}; // policies disabled
use crate::state::AppState;
use crate::users::{UserDataDeleteSummary, UserDataExport, delete_user_data_owned};
// Phase 4.8.1: dnd5e_server will be loaded at runtime via game system registry

// Phase 4.9.Z Step 1: Core entity types extracted to separate module
pub mod types;
pub use types::{
    GraphQLGameSystem, GraphQLUser, GraphQLWorld, GraphQLWorldEvent, GraphQLWorldToken,
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
    ActorQuery, AdminQuery, HealthcheckQuery, InventoryQuery, InviteQuery, ItemQuery, SceneQuery,
    UserQuery,
};

// Phase 4.10.B: Invite & Membership mutations for multiplayer campaigns
pub mod mutations_invites;
pub use mutations_invites::InviteMutation;

// Phase 6: Wall mutations (vision-blocking scene geometry)
pub mod mutations_walls;
pub use mutations_walls::WallMutation;

// Native canvas authoring: light source mutations
pub mod mutations_lighting;
pub use mutations_lighting::LightSourceMutation;

// Native canvas authoring: shape (stroke/rect/ellipse/line/text) mutations
pub mod mutations_shapes;
pub use mutations_shapes::ShapeMutation;

// Native canvas authoring: scene-scoped token mutations
pub mod mutations_tokens;
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
pub mod mutations_actor_shares;
pub use mutations_actor_shares::{ActorShareMutation, ActorShareQuery};

// Spec 013: item creation/field-editing/deletion and effect CRUD
pub mod mutations_items;
pub use mutations_items::ItemMutation;

// Spec 013: the item "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_item_permissions;
pub use mutations_item_permissions::{ItemPermissionMutation, ItemPermissionQuery};

// Spec 013: item sharing and cross-world deep copy
pub mod mutations_item_shares;
pub use mutations_item_shares::{ItemShareMutation, ItemShareQuery};

// Spec 013: actor inventory (Item + quantity, permissioned via the actor)
pub mod mutations_inventory;
pub use mutations_inventory::InventoryMutation;

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
            background_image_path: scene.background_image_path,
            background_asset_id: scene.background_asset_id,
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
        }
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
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = uuid::Uuid::now_v7();
        let new_scene = crate::models::Scene {
            scene_id,
            world_id: input.world_id,
            name: input.name,
            description: input.description,
            type_: input.type_.unwrap_or_else(|| "battlemap".to_string()),
            grid_size: input.grid_size.unwrap_or(5),
            grid_type: input.grid_type.unwrap_or_else(|| "square".to_string()),
            width: input.width.unwrap_or(100),
            height: input.height.unwrap_or(100),
            metadata: input.metadata.map(|j| j.0),
            owner_id: user_id,
            created_at: now,
            updated_at: now,
            background_image_path: None,
            background_asset_id: None,
        };

        let inserted_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

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
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let _now = Utc::now().naive_utc();

        let updated_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            let update_data = crate::models::SceneUpdate {
                name: input.name,
                description: input.description,
                grid_size: input.grid_size,
                grid_type: input.grid_type,
                width: input.width,
                height: input.height,
                metadata: input.metadata.map(|j| j.0),
            };

            diesel::update(
                scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .filter(scenes::owner_id.eq(user_id)),
            )
            .set(update_data)
            .returning(crate::models::Scene::as_returning())
            .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update scene"))?;

        Ok(GraphQLScene::from(updated_scene))
    }

    async fn delete_scene(&self, ctx: &Context<'_>, scene_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;
            diesel::delete(
                scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .filter(scenes::owner_id.eq(user_id)),
            )
            .execute(&mut conn)
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
    };

    let inserted_world = new_world.clone();
    let world_name_for_scene = new_world.name.clone();
    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;

        conn.transaction(|conn| {
            diesel::insert_into(worlds::table)
                .values(&inserted_world)
                .execute(conn)?;

            let scene_values = (
                scenes::scene_id.eq(uuid::Uuid::now_v7()),
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
    Ok(GraphQLWorld::from(new_world))
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
    if !crate::auth::actor_permissions::is_dm_of_world(state, user_id, is_admin, input.world_id)
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

/// Helper function to query current online players for a world (Phase 4.9.B.3)
#[allow(dead_code)] // no resolver wires presence querying to this yet
async fn query_players_online(
    pool: &crate::state::DbPool,
    world_id: uuid::Uuid,
) -> Result<Vec<GraphQLPlayerPresence>, String> {
    let mut conn = pool.get().map_err(|e| format!("Pool error: {}", e))?;

    let players = tokio::task::spawn_blocking(move || {
        use crate::schema::players_online;
        use diesel::prelude::*;

        players_online::table
            .filter(players_online::world_id.eq(world_id))
            .select((
                players_online::player_id,
                players_online::world_id,
                players_online::scene_id,
                players_online::idle_duration_secs,
            ))
            .load::<(uuid::Uuid, uuid::Uuid, Option<uuid::Uuid>, i32)>(&mut conn)
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
    .map_err(|e| format!("Query error: {}", e))?;

    Ok(players
        .into_iter()
        .map(
            |(player_id, world_id, scene_id, idle_secs)| GraphQLPlayerPresence {
                player_id,
                world_id,
                scene_id,
                idle_duration_secs: idle_secs,
            },
        )
        .collect())
}

#[derive(Default)]
pub struct SubscriptionRoot;

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

        // Collect all validation to happen upfront
        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (Some(app_state), Some(_)) => {
                (false, "", Some(app_state.world_event_sender.subscribe()))
            }
        };

        eprintln!(
            "[GraphQL Subscription] 🎮 New subscription for world_id={}, error={}",
            world_id, has_error
        );

        // Create a combined stream that works for both cases
        // Return type is Pin<Box<dyn Stream>> for type erasure
        if let Some(rx) = rx_opt {
            // Success case: stream from broadcast channel
            let world_uuid = world_uuid.unwrap();

            let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
                .filter_map(move |result| {
                    match result {
                        Ok(event) => {
                            // Only send events for this world
                            if event.world_id == world_uuid {
                                eprintln!("[GraphQL Subscription] 📤 Sending event id={} to client", event.id);
                                Some(Ok(GraphQLWorldEvent::from(event)))
                            } else {
                                None
                            }
                        }
                        Err(broadcast_err) => {
                            // Handle lagged subscribers gracefully
                            // BroadcastStream wraps RecvError, we just log and drop
                            eprintln!("[GraphQL Subscription] ⚠️  Broadcast stream error: {:?} (backpressure/drop)", broadcast_err);
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream =
                tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        }
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

        let (has_error, error_msg, rx_opt) = match (&app_state, &world_uuid) {
            (None, _) => (true, "Failed to get app state", None),
            (_, None) => (true, "Invalid world_id format", None),
            (Some(app_state), Some(_)) => (false, "", Some(app_state.presence_sender.subscribe())),
        };

        eprintln!(
            "[GraphQL Subscription] 🎮 New presence subscription for world_id={}, error={}",
            world_id, has_error
        );

        if let (Some(rx), Some(world_id_uuid)) = (rx_opt, world_uuid) {
            // Success case: emit presence notifications
            // Note: This is a simple implementation that emits on each presence event.
            // In production, you'd query the DB to get the full player list on each event.
            let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
                .filter_map(move |result| {
                    match result {
                        Ok(_presence_event) => {
                            eprintln!("[GraphQL Subscription] 📤 Presence updated");
                            // For now, return an empty list (real implementation would query DB)
                            Some(Ok(GraphQLPlayersOnlineList {
                                world_id: world_id_uuid,
                                players: vec![],
                            }))
                        }
                        Err(_broadcast_err) => {
                            eprintln!("[GraphQL Subscription] ⚠️  Broadcast stream error (backpressure/drop)");
                            None
                        }
                    }
                });
            Pin::new(Box::new(stream))
                as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream =
                tokio_stream::iter(vec![Err(Error::new(error_msg))]).filter_map(Some);
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
    HealthcheckQuery,
    UserQuery,
    AdminQuery,
    SceneQuery,
    InviteQuery,
    AssetQuery,
    ActorQuery,
    ActorPermissionQuery,
    ActorShareQuery,
    ItemQuery,
    ItemPermissionQuery,
    ItemShareQuery,
    InventoryQuery,
);

#[derive(MergedObject, Default)]
pub struct MutationRoot(
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
    TokenMutation,
    AssetMutation,
    ActorMutation,
    ActorPermissionMutation,
    ActorShareMutation,
    ItemMutation,
    ItemPermissionMutation,
    ItemShareMutation,
    InventoryMutation,
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
        let before_count = worlds::table
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
}
