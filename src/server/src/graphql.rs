use async_graphql::{
    Context, Enum, Error, InputObject, Json, MergedObject, Result as GraphQLResult, Schema,
    SimpleObject, Subscription,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use futures_util::Stream;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

use crate::admin::{
    AdminStatsSnapshot, AdminWelcomeSummarySnapshot, DiskUsageSummary, OAuthProviderUpdate,
    SystemManifestDocument, editable_manifest_keys, load_admin_bootstrap_settings,
    load_admin_stats, load_admin_welcome_summary, load_auth_security_settings,
    load_oauth_providers, read_system_manifest, recalculate_disk_usage as calculate_disk_usage,
    update_manifest_key as persist_manifest_key, update_oauth_provider as persist_oauth_provider,
    update_two_factor_policy as persist_two_factor_policy,
};
// use crate::audit::{log_mutation, log_deletion, log_admin_query}; // Phase 4.X: Disabled pending schema
use crate::auth_middleware::AuthenticatedUser;
use crate::db_types::PolicyEffectEnum;
use crate::models::{
    AdminBootstrapSetup, AuthSecuritySetting, GameSystem, OAuthProvider, User, World,
    WorldEvent, WorldToken, WorldActor, ActorSystemData,
    // Policy - disabled pending schema
};
use crate::schema::{game_systems, users, world_events, world_tokens, worlds, world_actors, world_actor_system_data}; // policies disabled
use crate::state::AppState;
use crate::users::{
    UserDataDeleteSummary, UserDataExport, delete_user_data_owned, export_user_data_payload,
};
// Phase 4.8.1: dnd5e_server will be loaded at runtime via game system registry

// Phase 4.9.Z Step 1: Core entity types extracted to separate module
pub mod types;
pub use types::{GraphQLUser, GraphQLGameSystem, GraphQLWorld, GraphQLWorldToken, GraphQLWorldEvent};

// Phase 4.9.Z Step 2: Admin types extracted to separate module
pub mod admin_types;
pub use admin_types::{
    GraphQLAdminStats, GraphQLAdminWelcomeSummary, GraphQLOAuthProvider, GraphQLManifestEntry,
    GraphQLSystemManifest, GraphQLAuthSecuritySettings, GraphQLAdminBootstrapSettings,
    GraphQLOAuthProviderConfigInput, GraphQLDiskUsageBreakdown,
};

// Phase 4.9.Z Step 3: Input & utility types extracted to separate module
pub mod input_types;
pub use input_types::{
    GraphQLCreateWorldInput, GraphQLCreateSceneInput, GraphQLUpdateSceneInput,
    GraphQLPlayerPresence, GraphQLPlayersOnlineList, GraphQLPolicyEffect, GraphQLPolicy,
    GraphQLPlaceholderDomainObject, GraphQLExportManifest, GraphQLExportMyDataPayload,
    GraphQLDeleteMyDataPayload, GraphQLDeleteWorldPayload, GraphQLCreateWorldTokenInput,
    GraphQLUpsertWorldTokenInput, GraphQLMoveTokenInput, GraphQLUpsertTokenInput,
    GraphQLUpdateFogMaskInput, GraphQLGenerateInviteCodeInput, GraphQLJoinWorldInput,
    GraphQLUpdateMemberRoleInput, GraphQLWorldInvite, GraphQLWorldMembership,
    GraphQLCreateWallInput, GraphQLUpdateWallInput, GraphQLDoorState,
    GraphQLCreateLightSourceInput, GraphQLUpdateLightSourceInput,
};

// Phase 4.9.Z Step 4a: Helper functions extracted to separate module
pub mod helpers;
pub use helpers::{
    app_state, authenticated_user, admin_user, normalize_world_name, normalize_optional_text,
    validate_world_name, validate_optional_reference_id, prepare_world_input, world_write_error,
    load_game_systems, load_owned_worlds, load_all_worlds, load_owned_world_tokens,
    load_owned_world_events, load_visible_world_by_id,
    load_owned_world_token_by_id, load_owned_world_event_by_id,
    load_game_system_by_id, get_world_id_from_scene, PreparedWorldInput,
};

// Phase 4.9.Z Step 5: Query extraction into separate modules
pub mod queries;
pub use queries::{AdminQuery, HealthcheckQuery, SceneQuery, UserQuery, InviteQuery};

// Phase 4.10.B: Invite & Membership mutations for multiplayer campaigns
pub mod mutations_invites;
pub use mutations_invites::{InviteMutation, WorldInvitePayload, WorldMembershipPayload};

// Phase 6: Wall mutations (vision-blocking scene geometry)
pub mod mutations_walls;
pub use mutations_walls::WallMutation;

// Native canvas authoring: light source mutations
pub mod mutations_lighting;
pub use mutations_lighting::LightSourceMutation;



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
        if let Err(e) = crate::session::touch_last_seen(
            state.db_pool.clone(),
            user_id,
            input.world_id,
        )
        .await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        // Phase 1.3: Log mutation to audit trail - disabled
        // if let Ok(token_uuid) = uuid::Uuid::parse_str(&created_token.id) {
        //     let _ = log_mutation(
        //         state,
        //         "create",
        //         user_id,
        //         "world_token",
        //         token_uuid,
        //     )
        //     .await;
        // }

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

        let token_id = input.token_id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
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
            use crate::schema::{world_tokens, world_events};
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
        if let Err(e) = crate::session::touch_last_seen(
            state.db_pool.clone(),
            user_id,
            world_id,
        )
        .await
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
        if let Err(e) = crate::session::touch_last_seen(
            state.db_pool.clone(),
            user_id,
            moved_token.world_id,
        )
        .await
        {
            eprintln!("⚠️  Failed to update session: {}", e);
        }

        Ok(GraphQLWorldToken::from(moved_token))
    }

    async fn delete_world_token(
        &self,
        ctx: &Context<'_>,
        token_id: String,
    ) -> GraphQLResult<bool> {
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
    game_system_id: String,  // 'dnd5e', 'pathfinder2e', etc.
    data_type: String,        // 'ability_data', 'resource_data', 'proficiency_data', 'trait_data', 'spell_data'
    data: Json<serde_json::Value>,  // Raw JSON for system-specific content
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
    event_type: String,  // "INSERT", "UPDATE", "DELETE"
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
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let actor_id = input.actor_id;
        let game_system_id = input.game_system_id.clone();
        let data_type = input.data_type.clone();
        let data_value = input.data.0.clone();
        let now = Utc::now().naive_utc();

        // 🔐 ADR-010: Verify user owns the actor + persist update in single spawn_blocking call
        let upserted_data = tokio::task::spawn_blocking({
            let actor_id = actor_id;
            let user_id = user_id;
            let game_system_id = game_system_id.clone();
            let data_type = data_type.clone();
            let data_value = data_value.clone();
            move || {
                use crate::schema::{world_actors, world_actor_system_data};
                use diesel::prelude::*;

                // 1. Verify user owns the actor
                let actor = world_actors::table
                    .filter(world_actors::id.eq(actor_id))
                    .filter(world_actors::owned_by.eq(user_id))
                    .select(crate::models::WorldActor::as_select())
                    .first::<crate::models::WorldActor>(&mut conn)
                    .optional()
                    .map_err(|e| format!("Failed to load actor: {}", e))?
                    .ok_or_else(|| "Actor not found or not owned by user".to_string())?;

                // 2. Validate: Actor's game_system_id must match the mutation's game_system_id
                if actor.game_system_id.as_ref() != Some(&game_system_id) {
                    return Err("Game system mismatch: actor is not configured for this system".to_string());
                }

                // 3. Validate data_type and use registry-based validators
                if !matches!(data_type.as_str(), "ability_data" | "resource_data" | "proficiency_data" | "trait_data" | "spell_data") {
                    return Err("Unknown data_type".to_string());
                }

                // 🔔 C2: Call system registry to validate data using game system's validators
                crate::systems::validate_actor_system_data(&game_system_id, &data_type, &data_value)
                    .map_err(|e| format!("Validation failed: {}", e))?;

                // 4. UPSERT actor system data with appropriate column
                let result = match data_type.as_str() {
                    "ability_data" => {
                        diesel::insert_into(world_actor_system_data::table)
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
                            .get_result(&mut conn)
                    },
                    "resource_data" => {
                        diesel::insert_into(world_actor_system_data::table)
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
                            .get_result(&mut conn)
                    },
                    "proficiency_data" => {
                        diesel::insert_into(world_actor_system_data::table)
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
                            .get_result(&mut conn)
                    },
                    "trait_data" => {
                        diesel::insert_into(world_actor_system_data::table)
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
                            .get_result(&mut conn)
                    },
                    "spell_data" => {
                        diesel::insert_into(world_actor_system_data::table)
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
                            .get_result(&mut conn)
                    },
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

    async fn delete_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<bool> {
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

    async fn upsert_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpsertTokenInput,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id.unwrap_or_else(uuid::Uuid::now_v7);
        let scene_id = input.scene_id;
        let actor_id = input.actor_id;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let rotation = input.rotation.unwrap_or(0.0);
        let scale = input.scale.unwrap_or(1.0);
        let metadata = input.metadata.map(|j| j.0);

        let upserted_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            use diesel::prelude::*;
            
            diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::actor_id.eq(actor_id),
                    tokens::x.eq(x),
                    tokens::y.eq(y),
                    tokens::rotation.eq(rotation),
                    tokens::scale.eq(scale),
                    tokens::metadata.eq(&metadata),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .on_conflict(tokens::token_id)
                .do_update()
                .set((
                    tokens::actor_id.eq(actor_id),
                    tokens::x.eq(x),
                    tokens::y.eq(y),
                    tokens::rotation.eq(rotation),
                    tokens::scale.eq(scale),
                    tokens::metadata.eq(&metadata),
                    tokens::updated_at.eq(now),
                ))
                .returning(crate::models::Token::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to upsert token"))?;

        Ok(GraphQLToken::from(upserted_token))
    }

    async fn delete_token(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            use diesel::prelude::*;
            diesel::delete(tokens::table.filter(tokens::token_id.eq(token_id)))
                .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

        Ok(deleted > 0)
    }

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
        let prepared_input = prepare_world_input(input).map_err(Error::new)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let new_world = World {
            id: uuid::Uuid::now_v7(),
            name: prepared_input.name,
            description: prepared_input.description,
            game_system_id: prepared_input.game_system_id,
            interface_pack_id: prepared_input.interface_pack_id,
            created_by: auth_user.user_id,
            updated_by: auth_user.user_id,
            created_at: now,
            updated_at: now,
        };

        let inserted_world = new_world.clone();
        tokio::task::spawn_blocking(move || {
            diesel::insert_into(worlds::table)
                .values(&inserted_world)
                .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|error| world_write_error(error, "Failed to create world"))?;

        // Phase 2.4: Auto-assign creator as OWNER for RBAC - disabled pending schema
        // crate::rbac::RbacEngine::assign_creator_as_owner(state, new_world.id, auth_user.user_id)
        //     .await
        //     .map_err(|e| Error::new(format!("Failed to assign owner role: {}", e)))?;

        // Phase 1.3: Log mutation to audit trail - disabled
        // let _ = log_mutation(
        //     state,
        //     "create",
        //     auth_user.user_id,
        //     "world",
        //     new_world.id,
        // )
        // .await;

        Ok(GraphQLWorld::from(new_world))
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

        // Phase 1.3: Log deletion to audit trail - disabled
        // let _ = log_deletion(
        //     state,
        //     auth_user.user_id,
        //     "world",
        //     world.id,
        //     None,
        // )
        // .await;

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
        let admin_user_id = admin_user(ctx)?.user_id;
        let result = persist_oauth_provider(state, provider_id, config.into())
            .await
            .map(GraphQLOAuthProvider::from)
            .map_err(Error::new)?;

        // Phase 1.3: Log admin action to audit trail - disabled
        // let _ = log_admin_query(
        //     state,
        //     admin_user_id,
        //     "update_oauth_provider",
        //     None,
        // )
        // .await;

        Ok(result)
    }

    async fn update_manifest_key(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let admin_user_id = admin_user(ctx)?.user_id;
        let result = persist_manifest_key(state, &key, &value)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)?;

        // Phase 1.3: Log admin action to audit trail - disabled
        // let _ = log_admin_query(
        //     state,
        //     admin_user_id,
        //     "update_manifest_key",
        //     None,
        // )
        // .await;

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
        let admin_user_id = admin_user(ctx)?.user_id;
        let result = persist_two_factor_policy(state, required_for_all_users)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)?;

        // Phase 1.3: Log admin action to audit trail - disabled
        // let _ = log_admin_query(
        //     state,
        //     admin_user_id,
        //     "update_two_factor_policy",
        //     None,
        // )
        // .await;

        Ok(result)
    }
}

/// Helper function to query current online players for a world (Phase 4.9.B.3)
async fn query_players_online(
    pool: &crate::state::DbPool,
    world_id: uuid::Uuid,
) -> Result<Vec<GraphQLPlayerPresence>, String> {
    let mut conn = pool
        .get()
        .map_err(|e| format!("Pool error: {}", e))?;

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
        .map(|(player_id, world_id, scene_id, idle_secs)| GraphQLPlayerPresence {
            player_id,
            world_id,
            scene_id,
            idle_duration_secs: idle_secs,
        })
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
            (Some(app_state), Some(_)) => (false, "", Some(app_state.world_event_sender.subscribe())),
        };
        
        eprintln!("[GraphQL Subscription] 🎮 New subscription for world_id={}, error={}", world_id, has_error);
        
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
            Pin::new(Box::new(stream)) as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))])
                .filter_map(|x| { Some(x) });
            Pin::new(Box::new(stream)) as Pin<Box<dyn Stream<Item = Result<GraphQLWorldEvent, Error>> + Send>>
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
        
        eprintln!("[GraphQL Subscription] 🎮 New presence subscription for world_id={}, error={}", world_id, has_error);
        
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
            Pin::new(Box::new(stream)) as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
        } else {
            // Error case: single error item
            let stream = tokio_stream::iter(vec![Err(Error::new(error_msg))])
                .filter_map(|x| { Some(x) });
            Pin::new(Box::new(stream)) as Pin<Box<dyn Stream<Item = Result<GraphQLPlayersOnlineList, Error>> + Send>>
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
            }
        )
    }
}

// Phase 2.5: RBAC Collaborator Management - DISABLED pending schema completion
// #[derive(Default)]
// pub struct CollaboratorMutation;

// #[async_graphql::Object]
// impl CollaboratorMutation { ... }

// Re-define CollaboratorMutation as empty for now
#[derive(async_graphql::MergedObject, Default)]
pub struct CollaboratorMutation;

#[derive(MergedObject, Default)]
pub struct QueryRoot(HealthcheckQuery, UserQuery, AdminQuery, SceneQuery, InviteQuery);

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
);

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

#[cfg(test)]
mod tests {
    use super::{GraphQLCreateWorldInput, prepare_world_input, validate_world_name};

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
        assert!(validate_world_name(&valid).is_ok(), "64 chars should be valid");

        // Invalid: 65+ characters
        let invalid = "A".repeat(65);
        assert!(validate_world_name(&invalid).is_err(), "65+ chars should be rejected");

        // Invalid: 2 characters (MIN_WORLD_NAME_LEN is 3)
        let too_short = "AB";
        assert!(validate_world_name(too_short).is_err(), "2 chars should be rejected");

        // Valid: 3 characters (MIN_WORLD_NAME_LEN)
        let min_valid = "ABC";
        assert!(validate_world_name(min_valid).is_ok(), "3 chars should be valid");
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
// 
//     // Phase 2.6: RBAC tests
//     #[test]
//     fn rbac_role_from_str_parses_all_variants() {
//         assert_eq!(crate::rbac::Role::from_str("OWNER"), Some(crate::rbac::Role::Owner));
//         assert_eq!(crate::rbac::Role::from_str("EDITOR"), Some(crate::rbac::Role::Editor));
//         assert_eq!(crate::rbac::Role::from_str("VIEWER"), Some(crate::rbac::Role::Viewer));
//         assert_eq!(crate::rbac::Role::from_str("INVALID"), None);
//         assert_eq!(crate::rbac::Role::from_str("owner"), None); // case-sensitive
//     }
// 
//     #[test]
//     fn rbac_role_as_str_round_trips() {
//         let owner = crate::rbac::Role::Owner;
//         assert_eq!(crate::rbac::Role::from_str(owner.as_str()), Some(owner));
// 
//         let editor = crate::rbac::Role::Editor;
//         assert_eq!(crate::rbac::Role::from_str(editor.as_str()), Some(editor));
// 
//         let viewer = crate::rbac::Role::Viewer;
//         assert_eq!(crate::rbac::Role::from_str(viewer.as_str()), Some(viewer));
//     }
// 
//     #[test]
//     fn rbac_owner_has_all_permissions() {
//         let owner = crate::rbac::Role::Owner;
//         assert!(owner.has_permission("view"));
//         assert!(owner.has_permission("edit"));
//         assert!(owner.has_permission("delete"));
//         assert!(owner.has_permission("invite"));
//         assert!(owner.has_permission("any_permission"));
//     }
// 
//     #[test]
//     fn rbac_editor_has_view_and_edit_permissions() {
//         let editor = crate::rbac::Role::Editor;
//         assert!(editor.has_permission("view"));
//         assert!(editor.has_permission("edit"));
//         assert!(!editor.has_permission("delete"));
//         assert!(!editor.has_permission("invite"));
//     }
// 
//     #[test]
//     fn rbac_viewer_has_only_view_permission() {
//         let viewer = crate::rbac::Role::Viewer;
//         assert!(viewer.has_permission("view"));
//         assert!(!viewer.has_permission("edit"));
//         assert!(!viewer.has_permission("delete"));
//         assert!(!viewer.has_permission("invite"));
//     }
// 
//     #[test]
//     fn rbac_role_equality() {
//         let owner1 = crate::rbac::Role::Owner;
//         let owner2 = crate::rbac::Role::Owner;
//         let editor = crate::rbac::Role::Editor;
// 
//         assert_eq!(owner1, owner2);
//         assert_ne!(owner1, editor);
//     }
// }
}

