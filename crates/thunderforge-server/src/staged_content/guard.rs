//! Spec 048 FR-036, FR-036a, FR-037a: content the world has not adopted is
//! not usable in play, and the server says why.
//!
//! The play field is never sent a staged piece (T036), so only a client that
//! was not composed by the server can name one. A staged link has no world
//! row, so a play path names it in one of two ways:
//!
//! - by the piece's own id, where it takes an ability or item id
//!   (`makeAttack`, the share links);
//! - by the link's id, where it takes an inventory entry (`adjustInventoryQuantity`,
//!   which is how an item is spent).
//!
//! Each finds the piece here, and refuses with `CONTENT_NOT_ADOPTED` and the
//! sentence below. A piece is found only for someone who could already see it:
//! the attacker's own links, its bringer, or whoever manages the world's
//! content. Anyone else gets the path's ordinary "not found", so the refusal
//! never reveals a piece.
//!
//! `rollCheck` takes a check the system declared, never content, so it cannot
//! name a piece at all.

use async_graphql::{Error, ErrorExtensions};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{ManagesContentError, require_manages_content};
use crate::schema::{world_actor_abilities, world_actor_inventory, world_staged_content};
use crate::staged_content::StagedState;
use crate::state::AppState;

/// The refusal's code (contracts/graphql-sheet-import.md).
pub const NOT_ADOPTED_CODE: &str = "CONTENT_NOT_ADOPTED";

/// FR-036a: why, in the player's terms. The web client shows the same words
/// (`apps/web/src/pages/world/actor/import/refusal.ts`).
pub const NOT_ADOPTED: &str =
    "This came in with the character, and the Game Master has not adopted it yet.";

/// A piece that was named in play and has not been adopted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unadopted {
    pub staged_id: Uuid,
    pub world_id: Uuid,
    pub name: String,
    /// The character it was used on, when the path names one.
    pub actor_id: Option<Uuid>,
}

impl Unadopted {
    /// The refusal every play path returns.
    pub fn refusal(&self) -> Error {
        Error::new(NOT_ADOPTED).extend_with(|_, ext| ext.set("code", NOT_ADOPTED_CODE))
    }
}

/// The piece an attack names, if `id` is one of the attacker's staged links,
/// or any staged piece in the world for someone who runs it.
pub fn named_by_attacker(
    conn: &mut PgConnection,
    world_id: Uuid,
    actor_id: Option<Uuid>,
    runs_the_world: bool,
    id: Uuid,
) -> QueryResult<Option<Unadopted>> {
    let piece = world_staged_content::table
        .filter(world_staged_content::id.eq(id))
        .filter(world_staged_content::world_id.eq(world_id))
        .filter(world_staged_content::state.ne(StagedState::Adopted))
        .select(world_staged_content::name)
        .first::<String>(conn)
        .optional()?;
    let Some(name) = piece else {
        return Ok(None);
    };
    if !runs_the_world {
        let Some(actor_id) = actor_id else {
            return Ok(None);
        };
        let linked = diesel::select(diesel::dsl::exists(
            world_actor_abilities::table
                .filter(world_actor_abilities::actor_id.eq(actor_id))
                .filter(world_actor_abilities::staged_id.eq(id)),
        ))
        .get_result::<bool>(conn)?
            || diesel::select(diesel::dsl::exists(
                world_actor_inventory::table
                    .filter(world_actor_inventory::actor_id.eq(actor_id))
                    .filter(world_actor_inventory::staged_id.eq(id)),
            ))
            .get_result::<bool>(conn)?;
        if !linked {
            return Ok(None);
        }
    }
    Ok(Some(Unadopted {
        staged_id: id,
        world_id,
        name,
        actor_id,
    }))
}

/// The piece an inventory entry carries, if it is a staged one.
pub fn carried_by_entry(conn: &mut PgConnection, entry_id: Uuid) -> QueryResult<Option<Unadopted>> {
    world_actor_inventory::table
        .inner_join(world_staged_content::table)
        .filter(world_actor_inventory::id.eq(entry_id))
        .filter(world_actor_inventory::item_id.is_null())
        .filter(world_staged_content::state.ne(StagedState::Adopted))
        .select((
            world_staged_content::id,
            world_staged_content::world_id,
            world_staged_content::name,
            world_actor_inventory::actor_id,
        ))
        .first::<(Uuid, Uuid, String, Uuid)>(conn)
        .optional()
        .map(|found| {
            found.map(|(staged_id, world_id, name, actor_id)| Unadopted {
                staged_id,
                world_id,
                name,
                actor_id: Some(actor_id),
            })
        })
}

/// The piece a share names, for its bringer or someone who manages the
/// world's content.
pub fn named_by_sharer(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: Uuid,
) -> QueryResult<Option<Unadopted>> {
    let piece = world_staged_content::table
        .filter(world_staged_content::id.eq(id))
        .filter(world_staged_content::state.ne(StagedState::Adopted))
        .select((
            world_staged_content::world_id,
            world_staged_content::name,
            world_staged_content::player_user_id,
        ))
        .first::<(Uuid, String, Uuid)>(conn)
        .optional()?;
    let Some((world_id, name, brought_by)) = piece else {
        return Ok(None);
    };
    let sees_it = brought_by == user_id
        || match require_manages_content(conn, world_id, user_id) {
            Ok(_) => true,
            Err(ManagesContentError::Database(error)) => {
                return Err(diesel::result::Error::QueryBuilderError(error.into()));
            }
            Err(_) => false,
        };
    Ok(sees_it.then_some(Unadopted {
        staged_id: id,
        world_id,
        name,
        actor_id: None,
    }))
}

/// The share paths' check: refused if `id` is a piece the caller can see and
/// the world has not adopted.
pub async fn refuse_share(state: &AppState, user_id: Uuid, id: Uuid) -> async_graphql::Result<()> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let found = tokio::task::spawn_blocking(move || named_by_sharer(&mut conn, user_id, id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to read the piece"))?;
    match found {
        Some(piece) => Err(piece.refusal()),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
