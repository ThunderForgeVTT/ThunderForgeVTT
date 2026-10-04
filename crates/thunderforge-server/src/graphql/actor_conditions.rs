//! `worldSystemConditions`, `applyActorCondition` and `clearActorCondition`
//! (spec 067 Story 4).
//!
//! Any member may read what the world's system declares: a marker on the
//! board means nothing to a player who cannot look up what it stands for.
//! Only a Game Master may put a character under a condition or lift one.
//!
//! What a character *is* under is not read here. It rides on the character's
//! tokens (`GraphQLToken.conditions`), so it reaches exactly the clients
//! those tokens reach and there is no second rule to keep in step.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use pack_system_spec::conditions::SystemCondition;

use crate::actor_conditions::{
    announce_changed, apply, clear, declarations_for_system, held_by_actor,
};
use crate::auth::world_membership::{actor_in_world, is_dm_of_world};
use crate::graphql::types_scene::TokenCondition;
use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;
use crate::world_system_settings::system_of_world;

/// One condition the world's system declares.
#[derive(SimpleObject, Debug, Clone)]
pub struct WorldSystemCondition {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    /// The marker's shape, from the host's list.
    pub glyph: String,
    /// The marker's colour, as a token the client's theme resolves.
    pub color: String,
}

impl From<SystemCondition> for WorldSystemCondition {
    fn from(condition: SystemCondition) -> Self {
        Self {
            id: condition.id,
            label: condition.label,
            description: condition.description,
            glyph: condition.marker.glyph.as_str().to_string(),
            color: condition.marker.color.as_str().to_string(),
        }
    }
}

#[derive(Default)]
pub struct ActorConditionsQuery;

#[Object]
impl ActorConditionsQuery {
    /// The conditions this world's game system declares, in its own order.
    /// Empty when the world has no system or the system declares none.
    async fn world_system_conditions(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<WorldSystemCondition>> {
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let state = app_state(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            let actor = actor_in_world(&mut conn, user_id, is_admin, world_id);
            if actor.role.is_none() && !actor.is_site_admin {
                return Err(Error::new("Not a member of this world"));
            }
            let Some(system_id) =
                system_of_world(&mut conn, world_id).map_err(|_| Error::new("World not found"))?
            else {
                return Ok(Vec::new());
            };
            Ok(declarations_for_system(&systems_dir, &system_id)
                .into_iter()
                .map(WorldSystemCondition::from)
                .collect())
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}

#[derive(Default)]
pub struct ActorConditionsMutation;

#[Object]
impl ActorConditionsMutation {
    /// Put a character under a condition its world's system declares. Game
    /// Master only. Answers with every condition the character is now under.
    async fn apply_actor_condition(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        condition_id: String,
    ) -> GraphQLResult<Vec<TokenCondition>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_condition_impl(
            state,
            user.user_id,
            user.is_admin,
            actor_id,
            condition_id,
            Change::Apply,
        )
        .await
    }

    /// Lift a condition from a character. Game Master only. Answers with
    /// every condition the character is still under.
    async fn clear_actor_condition(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        condition_id: String,
    ) -> GraphQLResult<Vec<TokenCondition>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_condition_impl(
            state,
            user.user_id,
            user.is_admin,
            actor_id,
            condition_id,
            Change::Clear,
        )
        .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Apply,
    Clear,
}

/// Answered as not found for anyone who does not run the actor's world, as
/// `setActorVisibleToPlayers` is: a refusal that named the actor would tell a
/// player that a creature hidden from them exists.
pub async fn change_condition_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    condition_id: String,
    change: Change,
) -> GraphQLResult<Vec<TokenCondition>> {
    use crate::schema::world_actors;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?
    .ok_or_else(|| Error::new("Actor not found"))?;
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Actor not found"));
    }

    let systems_dir = state.directories.systems_dir.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<Vec<TokenCondition>> {
        // What a character is under changes how they play, which is what
        // pausing stops. Before any write (ADR-100).
        crate::play_pause::gate::refuse_if_paused(&mut conn, world_id)?;

        match change {
            Change::Apply => {
                let system_id = system_of_world(&mut conn, world_id)
                    .map_err(|_| Error::new("World not found"))?
                    .ok_or_else(|| Error::new("This world has no game system"))?;
                let declared = declarations_for_system(&systems_dir, &system_id)
                    .iter()
                    .any(|declaration| declaration.id == condition_id);
                if !declared {
                    return Err(Error::new(format!(
                        "This world's system has no condition \"{condition_id}\""
                    )));
                }
                apply(&mut conn, actor_id, &condition_id, user_id)
                    .map_err(|e| Error::new(format!("Failed to apply the condition: {e}")))?;
            }
            Change::Clear => {
                clear(&mut conn, actor_id, &condition_id)
                    .map_err(|e| Error::new(format!("Failed to clear the condition: {e}")))?;
            }
        }

        announce_changed(&mut conn, world_id, actor_id, user_id);

        Ok(held_by_actor(&mut conn, &systems_dir, actor_id)
            .map_err(|e| Error::new(format!("Failed to read the actor's conditions: {e}")))?
            .into_iter()
            .map(TokenCondition::from)
            .collect())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[cfg(test)]
#[path = "actor_conditions_tests.rs"]
mod tests;
