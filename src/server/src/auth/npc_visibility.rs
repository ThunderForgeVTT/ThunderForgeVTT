//! Who may see an NPC (owner decision 2026-09-15).
//!
//! # The rule, stated once
//!
//! - A **player character** is seen by every member of its world, as before.
//! - An **NPC** is seen by everyone in its world when its Game Master has made
//!   it `visible_to_players`, and it is hidden until they do.
//! - A hidden NPC is still seen by whoever **runs the world** (Owner, GM, or a
//!   site admin), and by a player who **holds** it: an explicit Editor or
//!   Owner grant on it, a claim on it, a token of it they own, or the actor's
//!   `owned_by`. A Game Master who hands a goblin to a player has made it that
//!   player's creature (spec 046 research R7), and a player who cannot read
//!   the sheet of the creature they control cannot play it.
//!
//! Hidden answers exactly like missing: a by-id read of a hidden NPC says
//! "Actor not found", so probing ids tells a player nothing about which ones
//! exist.
//!
//! # Why this is not a permission level
//!
//! For the reason `permissioned_entities` gives at length: the ladder's floor,
//! `Viewer`, is also its default, so it cannot express "hidden". Visibility is
//! a separate axis, as `world_abilities.gm_only` is for abilities.
//!
//! # What this does not cover
//!
//! A token's name is the token's own rule (`name_visible_to_players`), and a
//! visible NPC's token still obeys it. The board sends every token, and a
//! token carries its `actor_id`; the world-event nudge for a sheet change
//! carries the actor's id too. Neither carries the name.

use std::collections::HashSet;

use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::types::ActorPermissionLevel;
use crate::models::WorldActor;
use crate::schema::{
    tokens, world_actor_claims, world_actor_permissions, world_actors, world_members,
};
use crate::state::AppState;

/// What the rule needs to know about an actor.
#[derive(Clone, Copy, Debug)]
pub struct Visibility {
    pub id: Uuid,
    pub is_npc: bool,
    pub visible_to_players: bool,
    pub owned_by: Uuid,
}

impl From<&WorldActor> for Visibility {
    fn from(actor: &WorldActor) -> Self {
        Visibility {
            id: actor.id,
            is_npc: actor.is_npc,
            visible_to_players: actor.visible_to_players,
            owned_by: actor.owned_by,
        }
    }
}

impl Visibility {
    /// Seen by every member of the world, whoever they are.
    pub fn seen_by_everyone(&self) -> bool {
        !self.is_npc || self.visible_to_players
    }
}

fn runs_the_world(conn: &mut PgConnection, user_id: Uuid, is_admin: bool, world_id: Uuid) -> bool {
    crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
        .runs_the_world()
}

fn holding_levels() -> Vec<&'static str> {
    [ActorPermissionLevel::Editor, ActorPermissionLevel::Owner]
        .into_iter()
        .map(ActorPermissionLevel::as_db_str)
        .collect()
}

/// Every actor in `world_id` that `user_id` holds by a grant, a claim or a
/// token, whatever the actor is. Three indexed reads, not one per actor.
fn held_in_world(
    conn: &mut PgConnection,
    user_id: Uuid,
    world_id: Uuid,
) -> QueryResult<HashSet<Uuid>> {
    let in_world = world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .select(world_actors::id);

    let mut held: HashSet<Uuid> = world_actor_permissions::table
        .filter(world_actor_permissions::user_id.eq(user_id))
        .filter(world_actor_permissions::level.eq_any(holding_levels()))
        .filter(world_actor_permissions::actor_id.eq_any(in_world))
        .select(world_actor_permissions::actor_id)
        .load::<Uuid>(conn)?
        .into_iter()
        .collect();

    held.extend(
        world_actor_claims::table
            .inner_join(world_members::table)
            .filter(world_members::user_id.eq(user_id))
            .filter(world_members::world_id.eq(world_id))
            .select(world_actor_claims::actor_id)
            .load::<Uuid>(conn)?,
    );

    held.extend(
        tokens::table
            .filter(tokens::owner_user_id.eq(user_id))
            .filter(tokens::actor_id.eq_any(in_world.nullable()))
            .select(tokens::actor_id.assume_not_null())
            .load::<Uuid>(conn)?,
    );

    Ok(held)
}

/// Whether `user_id` holds this one actor.
fn holds(conn: &mut PgConnection, user_id: Uuid, actor_id: Uuid) -> QueryResult<bool> {
    let granted = diesel::select(diesel::dsl::exists(
        world_actor_permissions::table
            .filter(world_actor_permissions::actor_id.eq(actor_id))
            .filter(world_actor_permissions::user_id.eq(user_id))
            .filter(world_actor_permissions::level.eq_any(holding_levels())),
    ))
    .get_result::<bool>(conn)?;
    if granted {
        return Ok(true);
    }
    let claimed = diesel::select(diesel::dsl::exists(
        world_actor_claims::table
            .inner_join(world_members::table)
            .filter(world_actor_claims::actor_id.eq(actor_id))
            .filter(world_members::user_id.eq(user_id)),
    ))
    .get_result::<bool>(conn)?;
    if claimed {
        return Ok(true);
    }
    diesel::select(diesel::dsl::exists(
        tokens::table
            .filter(tokens::actor_id.eq(actor_id))
            .filter(tokens::owner_user_id.eq(user_id)),
    ))
    .get_result::<bool>(conn)
}

/// The actors of one world a caller may see, from a list already loaded.
///
/// Every actor in `rows` must belong to `world_id`: the caller has already
/// scoped its query to that world, which is what makes one role lookup and
/// one set of holdings answer for all of them.
pub fn retain_visible_sync<T>(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    rows: Vec<T>,
    visibility: impl Fn(&T) -> Visibility,
) -> QueryResult<Vec<T>> {
    if rows.iter().all(|row| visibility(row).seen_by_everyone())
        || runs_the_world(conn, user_id, is_admin, world_id)
    {
        return Ok(rows);
    }
    let held = held_in_world(conn, user_id, world_id)?;
    Ok(rows
        .into_iter()
        .filter(|row| {
            let v = visibility(row);
            v.seen_by_everyone() || v.owned_by == user_id || held.contains(&v.id)
        })
        .collect())
}

/// [`retain_visible_sync`] for a list of world actors, off the async runtime.
pub async fn retain_visible_actors(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    rows: Vec<WorldActor>,
) -> GraphQLResult<Vec<WorldActor>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        retain_visible_sync(&mut conn, user_id, is_admin, world_id, rows, |actor| {
            Visibility::from(actor)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load world actors"))
}

/// Whether this caller may see this actor. `false` for an actor that does not
/// exist, so the two cannot be told apart.
pub fn is_actor_visible_sync(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> QueryResult<bool> {
    let Some((world_id, is_npc, visible_to_players, owned_by)) = world_actors::table
        .filter(world_actors::id.eq(actor_id))
        .select((
            world_actors::world_id,
            world_actors::is_npc,
            world_actors::visible_to_players,
            world_actors::owned_by,
        ))
        .first::<(Uuid, bool, bool, Uuid)>(conn)
        .optional()?
    else {
        return Ok(false);
    };
    let v = Visibility {
        id: actor_id,
        is_npc,
        visible_to_players,
        owned_by,
    };
    if v.seen_by_everyone() || owned_by == user_id {
        return Ok(true);
    }
    if runs_the_world(conn, user_id, is_admin, world_id) {
        return Ok(true);
    }
    holds(conn, user_id, actor_id)
}

/// Refuses a by-id read of an actor this caller may not see, in the words a
/// missing actor gets.
pub async fn require_actor_visible(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<()> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let visible = tokio::task::spawn_blocking(move || {
        is_actor_visible_sync(&mut conn, user_id, is_admin, actor_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?;
    if visible {
        Ok(())
    } else {
        Err(Error::new("Actor not found").extend_with(|_, ext| ext.set("code", "NOT_FOUND")))
    }
}

#[cfg(test)]
#[path = "npc_visibility_tests.rs"]
mod tests;
