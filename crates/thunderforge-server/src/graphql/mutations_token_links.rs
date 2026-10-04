//! A token is its actor, or a copy of it (spec 046 FR-015–FR-017, ADR-102).
//!
//! Three things live here:
//!
//! - **Placement defaults** ([`placement_for`]), which `createToken` asks
//!   before it inserts: a character's token is linked, a unique NPC's token is
//!   linked, any other NPC's token is an unlinked copy seeded from the NPC's
//!   data at that moment, and a token with no actor is a marker.
//! - **`setTokenLink`**: a Game Master changes a placed token. Linking a copy
//!   discards its own record, and is not a merge (spec edge case "relinking a
//!   copy"); unlinking a linked token seeds a record from the actor.
//! - **`setActorUnique`**: a Game Master marks an NPC as a named individual
//!   ("Boblin the goblin"), whose tokens are then placed linked.
//!
//! The resolvers themselves sit on `TokenMutation` (`mutations_tokens.rs`)
//! beside the token's other Game Master writes; this module is what they do.
//!
//! Neither mutation touches tokens already placed beyond the one named:
//! marking an NPC unique changes where its *next* token starts, never what the
//! goblins already on the board are.

use async_graphql::{Error, Result as GraphQLResult};
use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::token_kind::TokenKind;

use crate::graphql::types_scene::{GraphQLToken, GraphQLWorldActor};
use crate::play_pause::gate::refuse_if_paused;
use crate::schema::{scenes, tokens, world_actor_system_data, world_actors};
use crate::state::AppState;
use crate::world_events::{
    EVENT_CODE_ACTOR_SHEET_CHANGED, EVENT_CODE_TOKEN_CHANGED, record_world_event,
};

/// What placement needs to know about the actor a token is placed for.
#[derive(Clone, Debug)]
pub(crate) struct PlacementActor {
    pub world_id: Uuid,
    pub actor_type: String,
    pub is_npc: bool,
    pub is_unique: bool,
}

/// How a new token starts.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placement {
    pub linked: bool,
    /// Whether the token needs its actor's data copied onto it.
    pub seeds_from_actor: bool,
}

/// Whether a token placed for this actor is linked, when nobody said.
///
/// `is_npc` decides NPC-ness rather than `actor_type`: the two disagree on
/// real rows (characters flagged NPC and the reverse), and `is_npc` is what the
/// staging roster, token status and bringing the party already read.
pub(crate) fn default_linked(actor: Option<&PlacementActor>) -> bool {
    match actor {
        None => false,
        Some(actor) => !actor.is_npc || actor.is_unique,
    }
}

/// The placement for an actor and an optional explicit choice.
///
/// A choice of "linked" for a token with no actor is refused: there is
/// nothing for it to be.
pub(crate) fn placement_for(
    actor: Option<&PlacementActor>,
    requested: Option<bool>,
) -> Result<Placement, String> {
    let linked = requested.unwrap_or_else(|| default_linked(actor));
    if linked && actor.is_none() {
        return Err("A token with no actor cannot be linked".to_string());
    }
    Ok(Placement {
        linked,
        seeds_from_actor: !linked && actor.is_some(),
    })
}

/// The kind of token an actor's token is, when the caller names none.
///
/// Fixes NPC tokens being written as `character` (research R4): the kind
/// used to be the column default whatever was placed.
pub(crate) fn token_kind_for_actor(actor: &PlacementActor) -> TokenKind {
    match actor.actor_type.as_str() {
        "vehicle" => TokenKind::Vehicle,
        "hazard" | "prop" | "light_source" => TokenKind::Object,
        _ if actor.is_npc => TokenKind::Npc,
        _ => TokenKind::Character,
    }
}

/// Load the facts placement needs, or `None` when the actor does not exist.
pub(crate) fn load_placement_actor(
    conn: &mut PgConnection,
    actor_id: Uuid,
) -> QueryResult<Option<PlacementActor>> {
    world_actors::table
        .filter(world_actors::id.eq(actor_id))
        .select((
            world_actors::world_id,
            world_actors::actor_type,
            world_actors::is_npc,
            world_actors::is_unique,
        ))
        .first::<(Uuid, String, bool, bool)>(conn)
        .optional()
        .map(|row| {
            row.map(|(world_id, actor_type, is_npc, is_unique)| PlacementActor {
                world_id,
                actor_type,
                is_npc,
                is_unique,
            })
        })
}

/// An actor's `resource_data` as it stands, which is what a copy starts from.
///
/// `None` when the actor has no system data: the copy is then a marker until
/// it is relinked or given data.
pub(crate) fn actor_resource_data(
    conn: &mut PgConnection,
    actor_id: Uuid,
) -> QueryResult<Option<serde_json::Value>> {
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(world_actor_system_data::resource_data)
        .first::<Option<serde_json::Value>>(conn)
        .optional()
        .map(Option::flatten)
}

/// Change whether a token is its actor. Game Master only.
pub async fn set_token_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    token_id: Uuid,
    linked: bool,
) -> GraphQLResult<GraphQLToken> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let token = tokio::task::spawn_blocking(move || -> GraphQLResult<crate::models::Token> {
        conn.transaction(|conn| {
            // Locked: a hit landing on this token while it is relinked waits,
            // and then writes to whichever record the token has by then.
            let (scene_id, actor_id, was_linked) = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select((tokens::scene_id, tokens::actor_id, tokens::linked))
                .for_update()
                .first::<(Uuid, Option<Uuid>, bool)>(conn)
                .optional()
                .map_err(|e| Error::new(format!("Failed to load token: {e}")))?
                .ok_or_else(|| Error::new("Token not found"))?;
            let world_id = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::world_id)
                .first::<Uuid>(conn)
                .map_err(|e| Error::new(format!("Failed to load scene: {e}")))?;

            if !crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)
                .map_err(|e| Error::new(format!("Failed to check authority: {e}")))?
            {
                return Err(Error::new(
                    "Only the Game Master may change whether a token is linked",
                ));
            }
            refuse_if_paused(conn, world_id)?;

            // Asking for what the token already is changes nothing. Above all,
            // "unlink" on a copy must not re-seed it from its actor and quietly
            // heal a goblin somebody has just hit.
            if was_linked == linked {
                return tokens::table
                    .filter(tokens::token_id.eq(token_id))
                    .select(crate::models::Token::as_select())
                    .first(conn)
                    .map_err(|e| Error::new(format!("Failed to load token: {e}")));
            }

            let system_data = match (linked, actor_id) {
                (true, None) => return Err(Error::new("A token with no actor cannot be linked")),
                // Linking discards the copy's own record; the actor's is the
                // one that counts from now on.
                (true, Some(_)) => None,
                // Unlinking starts the copy from the actor as it stands.
                (false, Some(actor_id)) => actor_resource_data(conn, actor_id)
                    .map_err(|e| Error::new(format!("Failed to read the actor: {e}")))?,
                // A linked token with no actor (its actor was deleted) has
                // nothing to copy: it becomes a marker.
                (false, None) => None,
            };

            let now = chrono::Utc::now().naive_utc();
            let token = diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                .set((
                    tokens::linked.eq(linked),
                    tokens::system_data.eq(system_data),
                    tokens::updated_at.eq(now),
                ))
                .returning(crate::models::Token::as_returning())
                .get_result(conn)
                .map_err(|e| Error::new(format!("Failed to change the token: {e}")))?;

            crate::scene_fingerprint::refresh_scene_fingerprint(conn, scene_id, user_id);
            // 14: the token changed, and every client re-reads its bars on it.
            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_TOKEN_CHANGED,
                Some(serde_json::json!({
                    "action": "updated",
                    "token_id": token_id,
                    "scene_id": scene_id,
                })),
                user_id,
            );
            Ok(token)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    crate::graphql::token_art::token_with_art(state, token, user_id, is_admin).await
}

/// Mark an NPC as a named individual, or not. Game Master only.
pub async fn set_actor_unique_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    unique: bool,
) -> GraphQLResult<GraphQLWorldActor> {
    let world_id = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load actor"))?
        .ok_or_else(|| Error::new("Actor not found"))?
    };

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the Game Master may mark a creature unique",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let actor = tokio::task::spawn_blocking(move || -> GraphQLResult<crate::models::WorldActor> {
        refuse_if_paused(&mut conn, world_id)?;
        let actor = diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
            .set((
                world_actors::is_unique.eq(unique),
                world_actors::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(crate::models::WorldActor::as_returning())
            .get_result(&mut conn)
            .map_err(|e| Error::new(format!("Failed to change the actor: {e}")))?;
        // 26: the actor's sheet moved. Its NPC editor re-reads on it.
        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_ACTOR_SHEET_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "actorId": actor_id,
                "dataType": "is_unique",
            })),
            user_id,
        );
        Ok(actor)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    Ok(actor.into())
}

#[cfg(test)]
#[path = "mutations_token_links_tests.rs"]
mod tests;
