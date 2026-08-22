//! Spec 010: actor creation and field-editing mutations (`createActor`,
//! `updateActor`). See contracts/actor-crud.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;

use crate::auth::actor_permissions::{is_dm_of_world, require_actor_permission};
use crate::graphql::types::ActorPermissionLevel;
use crate::graphql::{GraphQLWorldActor, app_state, authenticated_user};
use crate::models::{NewWorldActor, WorldActor};
use crate::schema::{scenes, world_actors};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CreateActorInput {
    pub world_id: uuid::Uuid,
    pub label: String,
    pub is_npc: bool,
    pub actor_type: Option<String>,
    pub game_system_id: Option<String>,
    pub description: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateActorInput {
    pub actor_id: uuid::Uuid,
    pub label: Option<String>,
    pub is_npc: Option<bool>,
    pub actor_type: Option<String>,
    pub description: Option<String>,
}

/// Testable core of `ActorMutation::create_actor`, split out so tests
/// don't need a GraphQL `Context` (see `mutations_assets.rs`'s `_impl`
/// convention). DM-only (FR-019); `scene_id` defaults to the target
/// world's earliest-created scene (research.md §6).
pub async fn create_actor_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: CreateActorInput,
) -> GraphQLResult<WorldActor> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may create actors"));
    }

    let world_id = input.world_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_type = input
        .actor_type
        .clone()
        .unwrap_or_else(|| if input.is_npc { "npc".to_string() } else { "character".to_string() });
    let label = input.label.clone();
    let is_npc = input.is_npc;
    // The `actor_system_type_consistency` DB check requires a non-null
    // `game_system_id` for "npc"/"character" actor types (only
    // hazard/prop/light_source require it to be null) — default to a
    // generic placeholder when the caller doesn't supply one, since this
    // feature doesn't ask the DM to pick a game system up front.
    let game_system_id = Some(input.game_system_id.clone().unwrap_or_else(|| "generic".to_string()));
    let description = input.description.clone();

    tokio::task::spawn_blocking(move || {
        let scene_id = scenes::table
            .filter(scenes::world_id.eq(world_id))
            .order(scenes::created_at.asc())
            .select(scenes::scene_id)
            .first::<uuid::Uuid>(&mut conn)
            .map_err(|_| "World has no scenes to assign the new actor to".to_string())?;

        let new_actor = NewWorldActor {
            world_id,
            scene_id,
            actor_type,
            game_system_id,
            label,
            created_by: user_id,
            owned_by: user_id,
            is_public: false,
            is_npc,
            description,
        };

        diesel::insert_into(world_actors::table)
            .values(&new_actor)
            .returning(WorldActor::as_returning())
            .get_result::<WorldActor>(&mut conn)
            .map_err(|e| format!("Failed to create actor: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ActorMutation::update_actor`. Requires Editor or
/// Owner effective permission (FR-010, FR-011).
pub async fn update_actor_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: UpdateActorInput,
) -> GraphQLResult<WorldActor> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        input.actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let actor_id = input.actor_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let label = input.label.clone();
    let is_npc = input.is_npc;
    let actor_type = input.actor_type.clone();
    let description = input.description.clone();

    tokio::task::spawn_blocking(move || {
        let existing = world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(WorldActor::as_select())
            .first::<WorldActor>(&mut conn)
            .map_err(|_| "Actor not found".to_string())?;

        diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set((
                world_actors::label.eq(label.unwrap_or(existing.label)),
                world_actors::is_npc.eq(is_npc.unwrap_or(existing.is_npc)),
                world_actors::actor_type.eq(actor_type.unwrap_or(existing.actor_type)),
                world_actors::description.eq(description.or(existing.description)),
            ))
            .returning(WorldActor::as_returning())
            .get_result::<WorldActor>(&mut conn)
            .map_err(|e| format!("Failed to update actor: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct ActorMutation;

#[async_graphql::Object]
impl ActorMutation {
    async fn create_actor(
        &self,
        ctx: &Context<'_>,
        input: CreateActorInput,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_actor_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLWorldActor::from)
    }

    async fn update_actor(
        &self,
        ctx: &Context<'_>,
        input: UpdateActorInput,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_actor_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLWorldActor::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    /// FR-019: the DM (Owner via `worlds.created_by` fallback) can create
    /// an actor, and it lands on the world's default (earliest) scene.
    #[tokio::test]
    async fn dm_can_create_actor_on_default_scene() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Bo Jangles".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should be able to create an actor");

        assert_eq!(actor.label, "Bo Jangles");
        assert_eq!(actor.scene_id, scene_id);
        assert!(actor.is_npc);
    }

    /// FR-019: a Player-role (non-DM) caller is rejected.
    #[tokio::test]
    async fn non_dm_cannot_create_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = create_actor_impl(
            &state,
            player_id,
            false,
            CreateActorInput {
                world_id,
                label: "Should not exist".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await;

        assert!(result.is_err(), "a Player-role caller must not be able to create actors");
    }

    /// FR-010/FR-011: Editor and Owner can update; Viewer (default) cannot.
    #[tokio::test]
    async fn update_actor_requires_editor_or_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let actor = create_actor_impl(
            &state,
            owner_id,
            false,
            CreateActorInput {
                world_id,
                label: "Original Name".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should be able to create an actor");

        let mut conn = state.db_pool.get().unwrap();
        let viewer_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, viewer_id, "Player");
        drop(conn);

        let viewer_result = update_actor_impl(
            &state,
            viewer_id,
            false,
            UpdateActorInput {
                actor_id: actor.id,
                label: Some("Hacked Name".to_string()),
                is_npc: None,
                actor_type: None,
                description: None,
            },
        )
        .await;
        assert!(viewer_result.is_err(), "a default-Viewer caller must not be able to update the actor");

        let owner_result = update_actor_impl(
            &state,
            owner_id,
            false,
            UpdateActorInput {
                actor_id: actor.id,
                label: Some("Updated By DM".to_string()),
                is_npc: None,
                actor_type: None,
                description: None,
            },
        )
        .await
        .expect("the DM (implicit Owner) should be able to update the actor");
        assert_eq!(owner_result.label, "Updated By DM");
    }
}
