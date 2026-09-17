//! Spec 010: actor creation and field-editing mutations (`createActor`,
//! `updateActor`). See contracts/actor-crud.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;

use crate::auth::actor_permissions::require_actor_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::permissioned_entity_resolvers::{PausableContent, refuse_content_if_paused};
use crate::graphql::types::ActorPermissionLevel;
use crate::graphql::{GraphQLWorldActor, app_state, authenticated_user};
use crate::models::{NewWorldActor, WorldActor};
use crate::play_pause::gate::refuse_world_if_paused;
use crate::schema::{scenes, world_actors, worlds};
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
    refuse_world_if_paused(state, input.world_id).await?;

    let world_id = input.world_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_type = input.actor_type.clone().unwrap_or_else(|| {
        if input.is_npc {
            "npc".to_string()
        } else {
            "character".to_string()
        }
    });
    let label = input.label.clone();
    let is_npc = input.is_npc;
    // The `actor_system_type_consistency` DB check requires a non-null
    // `game_system_id` for "npc"/"character" actor types (only
    // hazard/prop/light_source require it to be null) — default to a
    // generic placeholder when the caller doesn't supply one, since this
    // feature doesn't ask the DM to pick a game system up front.
    // An explicit choice wins; otherwise the actor takes its world's system,
    // resolved below where a connection is available.
    //
    // It used to fall straight to "generic", which is a system nothing
    // declares — so every actor created through the compendium published no
    // values and rendered an empty sheet, whatever its world was playing.
    // Invisible until spec 032 made a sheet out of what a system declares:
    // before that the only sheet was Genie's hand-written one, which read the
    // actor's stored slots directly and never asked what system it was.
    let requested_system_id = input.game_system_id.clone();
    let description = input.description.clone();

    tokio::task::spawn_blocking(move || {
        let game_system_id = Some(match requested_system_id {
            Some(chosen) => chosen,
            None => worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first::<Option<String>>(&mut conn)
                .ok()
                .flatten()
                // A world that has itself chosen nothing. The DB check
                // requires a non-null id for npc/character actors, so this is
                // the placeholder of last resort rather than a default anyone
                // picked.
                .unwrap_or_else(|| "generic".to_string()),
        });

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
            // An NPC is hidden from players until its Game Master shows it.
            visible_to_players: false,
            // Not a named individual until its Game Master says so.
            is_unique: false,
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
    refuse_content_if_paused(state, PausableContent::Actor(input.actor_id)).await?;

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

/// Testable core of `ActorMutation::set_actor_visible_to_players`: a Game
/// Master shows an NPC to the players of its world, or hides it again (owner
/// decision 2026-09-15). A player character is seen by everyone and has no
/// such setting, so asking to change one is refused rather than recorded.
///
/// The announcement is the sheet-changed nudge `setActorUnique` sends, which
/// carries the actor's id and nothing else, never the name.
///
/// And, since showing an NPC also shows its tokens' names (owner decision
/// 2026-09-15, `auth::npc_visibility`), the token-changed nudge for every
/// scene the creature stands on and the combat-changed nudge for a fight it
/// is in — the two a client re-reads its board and its tracker on. Without
/// them a revealed NPC would keep reading "Unknown" until a reload. Both
/// carry ids only.
/// Tells every seat to re-read what this creature's tokens are called: the
/// board (token-changed, per scene it stands on) and the tracker
/// (combat-changed, for the world's running fight). Ids only, never a name.
fn announce_names_changed(
    conn: &mut PgConnection,
    world_id: uuid::Uuid,
    actor_id: uuid::Uuid,
    user_id: uuid::Uuid,
) {
    use crate::schema::{tokens, world_combats};

    let placed = tokens::table
        .filter(tokens::actor_id.eq(actor_id))
        .select((tokens::token_id, tokens::scene_id))
        .load::<(uuid::Uuid, uuid::Uuid)>(conn)
        .unwrap_or_default();
    let mut announced = std::collections::HashSet::new();
    for (token_id, scene_id) in placed {
        if !announced.insert(scene_id) {
            continue;
        }
        let _ = crate::world_events::record_world_event(
            conn,
            world_id,
            crate::world_events::EVENT_CODE_TOKEN_CHANGED,
            Some(serde_json::json!({
                "action": "updated",
                "token_id": token_id,
                "scene_id": scene_id,
            })),
            user_id,
        );
    }

    if let Ok(Some(combat_id)) = world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .select(world_combats::id)
        .first::<uuid::Uuid>(conn)
        .optional()
    {
        let _ = crate::world_events::record_world_event(
            conn,
            world_id,
            crate::world_events::EVENT_CODE_COMBAT_CHANGED,
            Some(serde_json::json!({ "combatId": combat_id })),
            user_id,
        );
    }
}

pub async fn set_actor_visible_to_players_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    actor_id: uuid::Uuid,
    visible: bool,
) -> GraphQLResult<WorldActor> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let found = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select((world_actors::world_id, world_actors::is_npc))
            .first::<(uuid::Uuid, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?;

    // Not the Game Master: answered as if the actor were not there, since a
    // player may not know a hidden NPC exists.
    let Some((world_id, is_npc)) = found else {
        return Err(Error::new("Actor not found"));
    };
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Actor not found"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<WorldActor> {
        crate::play_pause::gate::refuse_if_paused(&mut conn, world_id)?;
        if !is_npc {
            return Err(Error::new(
                "Only an NPC can be hidden from players; every player sees the characters",
            ));
        }
        let actor = diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set((
                world_actors::visible_to_players.eq(visible),
                world_actors::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldActor::as_returning())
            .get_result(&mut conn)
            .map_err(|e| Error::new(format!("Failed to change the actor: {e}")))?;
        let _ = crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_ACTOR_SHEET_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "actorId": actor_id,
                "dataType": "visible_to_players",
            })),
            user_id,
        );
        announce_names_changed(&mut conn, world_id, actor_id, user_id);
        Ok(actor)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `ActorMutation::set_actor_art_locked`: spec 044 FR-030b.
/// A Game Master locks one character's look, so the player holding it may no
/// longer change its portrait or token (`auth::actor_imagery`), whatever the
/// world setting says. Locking changes no image, and the Game Master's own
/// authority over the art is untouched.
///
/// Answered as not found for anyone else, as `setActorVisibleToPlayers` is.
pub async fn set_actor_art_locked_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    actor_id: uuid::Uuid,
    locked: bool,
) -> GraphQLResult<WorldActor> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?
    .ok_or_else(|| Error::new("Actor not found"))?;
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Actor not found"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<WorldActor> {
        crate::play_pause::gate::refuse_if_paused(&mut conn, world_id)?;
        let actor = diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set((
                world_actors::art_locked.eq(locked),
                world_actors::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldActor::as_returning())
            .get_result(&mut conn)
            .map_err(|e| Error::new(format!("Failed to change the actor: {e}")))?;
        // The holder's page re-reads on it, so its "Build look" appears or
        // goes without a reload. Ids only.
        let _ = crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_ACTOR_SHEET_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "actorId": actor_id,
                "dataType": "art_locked",
            })),
            user_id,
        );
        Ok(actor)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
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

    /// Show an NPC to the players of its world, or hide it from them. Game
    /// Master only. A hidden NPC reaches no player's actor list, search or
    /// by-id read; its tokens' names follow their own setting either way.
    async fn set_actor_visible_to_players(
        &self,
        ctx: &Context<'_>,
        actor_id: uuid::Uuid,
        visible: bool,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_actor_visible_to_players_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            visible,
        )
        .await
        .map(GraphQLWorldActor::from)
    }

    /// Lock one character's look against its player, or unlock it. Game
    /// Master only; changes no image.
    async fn set_actor_art_locked(
        &self,
        ctx: &Context<'_>,
        actor_id: uuid::Uuid,
        locked: bool,
    ) -> GraphQLResult<GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_actor_art_locked_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            locked,
        )
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

        assert!(
            result.is_err(),
            "a Player-role caller must not be able to create actors"
        );
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
        assert!(
            viewer_result.is_err(),
            "a default-Viewer caller must not be able to update the actor"
        );

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
