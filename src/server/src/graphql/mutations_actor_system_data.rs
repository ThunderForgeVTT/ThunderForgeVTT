//! An actor's per-system data blob: whatever the game system says an actor
//! tracks, stored generically so the server never has to know.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult, SimpleObject};

use super::*;

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
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub game_system_id: String,
    pub event_type: String, // "INSERT", "UPDATE", "DELETE"
    pub ability_data: Option<Json<serde_json::Value>>,
    pub resource_data: Option<Json<serde_json::Value>>,
    pub proficiency_data: Option<Json<serde_json::Value>>,
    pub trait_data: Option<Json<serde_json::Value>>,
    pub spell_data: Option<Json<serde_json::Value>>,
    pub updated_at: chrono::NaiveDateTime,
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
