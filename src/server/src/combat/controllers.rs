//! Who acts for a creature (spec 046 research R7).
//!
//! The codebase has four signals that could mean "this player's creature":
//! the token's owner, Owner permission on its actor, a claim, and the actor's
//! `owned_by`. Movement already answers the question that matters on a board —
//! who may move this token — with the first two, so the player who can move a
//! token is the player who takes its hits and swings its sword. One rule, used
//! by `moveOwnToken`, `makeAttack` and `resolveOffer`, rather than three
//! copies of it that could drift.
//!
//! A token no player controls is the Game Masters'. That is what makes an NPC
//! an NPC for auto-apply (research R15), and it is decided here, never from a
//! token's type: a goblin a Game Master handed to a player is that player's.

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{scenes, tokens, world_actor_permissions, world_actors};

/// The rows of a token this module reads.
#[derive(Clone, Copy, Debug)]
pub struct TokenControl {
    pub token_id: Uuid,
    pub world_id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
}

/// Load what control is decided from. `None` when the token is not there.
pub fn token_control(conn: &mut PgConnection, token_id: Uuid) -> QueryResult<Option<TokenControl>> {
    tokens::table
        .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
        .filter(tokens::token_id.eq(token_id))
        .select((
            tokens::token_id,
            scenes::world_id,
            tokens::owner_user_id,
            tokens::actor_id,
        ))
        .first::<(Uuid, Uuid, Option<Uuid>, Option<Uuid>)>(conn)
        .optional()
        .map(|row| {
            row.map(
                |(token_id, world_id, owner_user_id, actor_id)| TokenControl {
                    token_id,
                    world_id,
                    owner_user_id,
                    actor_id,
                },
            )
        })
}

fn runs_the_world(conn: &mut PgConnection, user_id: Uuid, is_admin: bool, world_id: Uuid) -> bool {
    crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
        .runs_the_world()
}

/// Whether `user_id` holds Owner on `actor_id` by an explicit grant.
fn holds_owner_grant(conn: &mut PgConnection, user_id: Uuid, actor_id: Uuid) -> QueryResult<bool> {
    let level = world_actor_permissions::table
        .filter(world_actor_permissions::actor_id.eq(actor_id))
        .filter(world_actor_permissions::user_id.eq(user_id))
        .select(world_actor_permissions::level)
        .first::<String>(conn)
        .optional()?;
    Ok(level
        .and_then(|value| ActorPermissionLevel::from_db_str(&value))
        .is_some_and(|level| level.rank() >= ActorPermissionLevel::Owner.rank()))
}

/// The rule `moveOwnToken` has always applied: the token's owner, or Owner on
/// its actor — which a Game Master of the actor's world holds implicitly
/// (spec 010), exactly as `effective_actor_permission` resolves it.
///
/// A Game Master is *not* granted an actorless token they do not own here;
/// they move those with `updateToken`. [`may_act_for`] is the rule for
/// everything a Game Master may do for any creature.
pub fn may_move(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    token: &TokenControl,
) -> QueryResult<bool> {
    if token.owner_user_id == Some(user_id) {
        return Ok(true);
    }
    let Some(actor_id) = token.actor_id else {
        return Ok(false);
    };
    let Some(actor_world) = world_actors::table
        .filter(world_actors::id.eq(actor_id))
        .select(world_actors::world_id)
        .first::<Uuid>(conn)
        .optional()?
    else {
        return Ok(false);
    };
    if runs_the_world(conn, user_id, is_admin, actor_world) {
        return Ok(true);
    }
    holds_owner_grant(conn, user_id, actor_id)
}

/// Whether `user_id` may act for the creature: a controller of it (C2), or a
/// Game Master of its world, who may act for anything (FR-009, FR-061).
pub fn may_act_for(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    token: &TokenControl,
) -> QueryResult<bool> {
    if runs_the_world(conn, user_id, is_admin, token.world_id) {
        return Ok(true);
    }
    may_move(conn, user_id, is_admin, token)
}

/// The players who control a token: its owner and every explicit Owner on its
/// actor, less anyone who runs the world.
///
/// Empty means the Game Masters run it. That is the whole of "an NPC the Game
/// Master runs" for auto-apply (R15): not the token's type, not `is_npc`.
pub fn player_controllers(conn: &mut PgConnection, token: &TokenControl) -> QueryResult<Vec<Uuid>> {
    let mut users: Vec<Uuid> = token.owner_user_id.into_iter().collect();
    if let Some(actor_id) = token.actor_id {
        let granted = world_actor_permissions::table
            .filter(world_actor_permissions::actor_id.eq(actor_id))
            .select((
                world_actor_permissions::user_id,
                world_actor_permissions::level,
            ))
            .load::<(Uuid, String)>(conn)?;
        users.extend(granted.into_iter().filter_map(|(user, level)| {
            ActorPermissionLevel::from_db_str(&level)
                .filter(|level| level.rank() >= ActorPermissionLevel::Owner.rank())
                .map(|_| user)
        }));
    }
    users.sort();
    users.dedup();
    users.retain(|user| !runs_the_world(conn, *user, false, token.world_id));
    Ok(users)
}

/// The tokens in a scene `user_id` controls as a player: the eyes a viewer sees
/// an attack through (contract §3). A Game Master's own tokens count as theirs
/// only by ownership or grant — a Game Master is never redacted anyway.
pub fn controlled_tokens_in_scene(
    conn: &mut PgConnection,
    user_id: Uuid,
    scene_id: Uuid,
) -> QueryResult<Vec<Uuid>> {
    let granted_actors = world_actor_permissions::table
        .filter(world_actor_permissions::user_id.eq(user_id))
        .select((
            world_actor_permissions::actor_id,
            world_actor_permissions::level,
        ))
        .load::<(Uuid, String)>(conn)?
        .into_iter()
        .filter_map(|(actor, level)| {
            ActorPermissionLevel::from_db_str(&level)
                .filter(|level| level.rank() >= ActorPermissionLevel::Owner.rank())
                .map(|_| actor)
        })
        .collect::<Vec<Uuid>>();
    tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .filter(
            tokens::owner_user_id
                .eq(user_id)
                .or(tokens::actor_id.eq_any(granted_actors)),
        )
        .select(tokens::token_id)
        .load::<Uuid>(conn)
}

#[cfg(test)]
#[path = "controllers_tests.rs"]
mod tests;
