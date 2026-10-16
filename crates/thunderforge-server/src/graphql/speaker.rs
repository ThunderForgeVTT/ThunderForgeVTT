//! Spec 103: a chat line or a roll said as a token.
//!
//! One rule for both (research R4): the world's Game Master may speak as any
//! token of the world, a player only as a token they may move
//! (`controllers::may_move`). Anything else is refused, not downgraded, as a
//! player asking for `gmOnly` is. A token that is not there is refused the
//! same way, so the answer never tells a player which ids exist.
//!
//! The stored label is the token's real name. A player reads it only where
//! they may read that token's name on the board (R5), else "Someone".

use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};
use diesel::PgConnection;
use diesel::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use crate::auth::npc_visibility::{readable_token_names_sync, token_name_readable_sync};
use crate::auth::world_membership::actor_in_world;
use crate::combat::attack::token_label;
use crate::combat::controllers::{TokenControl, may_move};
use crate::schema::{scenes, tokens};

pub const REFUSED: &str = "You can't speak as that token";

/// What a player reads for a token whose name they may not read.
pub const SOMEONE: &str = "Someone";

fn refused() -> Error {
    Error::new(REFUSED).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
}

fn db(e: diesel::result::Error) -> Error {
    Error::new(format!("Failed to load the speaker: {e}"))
}

/// The token `user_id` speaks as, and its name now, or FORBIDDEN.
///
/// The token must stand in `world_id`, and on `scene_id` when one is given.
pub fn resolve_speaker(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Option<Uuid>,
    user_id: Uuid,
    is_admin: bool,
    token_id: Uuid,
) -> GraphQLResult<(Uuid, String)> {
    let Some((token_scene, token_world, owner_user_id, actor_id)) = tokens::table
        .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
        .filter(tokens::token_id.eq(token_id))
        .select((
            tokens::scene_id,
            scenes::world_id,
            tokens::owner_user_id,
            tokens::actor_id,
        ))
        .first::<(Uuid, Uuid, Option<Uuid>, Option<Uuid>)>(conn)
        .optional()
        .map_err(db)?
    else {
        return Err(refused());
    };
    if token_world != world_id || scene_id.is_some_and(|scene| scene != token_scene) {
        return Err(refused());
    }
    let control = TokenControl {
        token_id,
        world_id,
        owner_user_id,
        actor_id,
    };
    let allowed = actor_in_world(conn, user_id, is_admin, world_id).runs_the_world()
        || may_move(conn, user_id, is_admin, &control).map_err(db)?;
    if !allowed {
        return Err(refused());
    }
    Ok((token_id, token_label(conn, token_id).map_err(db)?))
}

/// The label `stored` as this viewer reads it, given the speaker tokens whose
/// names players may read. A token since deleted reads as "Someone".
pub fn speaker_label(
    is_gm: bool,
    readable: &HashSet<Uuid>,
    token_id: Option<Uuid>,
    stored: Option<String>,
) -> Option<String> {
    let stored = stored?;
    if is_gm || token_id.is_some_and(|id| readable.contains(&id)) {
        Some(stored)
    } else {
        Some(SOMEONE.to_string())
    }
}

/// Of these speaker tokens, the ones whose names players may read: one query
/// for a whole page of lines. Empty for a Game Master, who reads them all.
pub fn readable_speakers(
    conn: &mut PgConnection,
    is_gm: bool,
    token_ids: impl IntoIterator<Item = Option<Uuid>>,
) -> QueryResult<HashSet<Uuid>> {
    if is_gm {
        return Ok(HashSet::new());
    }
    let ids: Vec<Uuid> = token_ids.into_iter().flatten().collect();
    readable_token_names_sync(conn, &ids)
}

/// [`speaker_label`] for one line.
pub fn speaker_label_for(
    conn: &mut PgConnection,
    is_gm: bool,
    token_id: Option<Uuid>,
    stored: Option<String>,
) -> QueryResult<Option<String>> {
    let readable = match token_id {
        Some(id) if !is_gm && stored.is_some() && token_name_readable_sync(conn, id)? => {
            HashSet::from([id])
        }
        _ => HashSet::new(),
    };
    Ok(speaker_label(is_gm, &readable, token_id, stored))
}

#[cfg(test)]
#[path = "speaker_tests.rs"]
mod tests;
