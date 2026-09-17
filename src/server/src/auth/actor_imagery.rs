//! Spec 044 phase (c), contracts §5 B6, ADR-105: who may change an actor's
//! portrait and token.
//!
//! # Why a right beside the ladder, not a rung on it
//!
//! ADR-050's ladder (Viewer < Editor < Owner) answers "may this person change
//! this actor". A player who holds a character resolves to Viewer on it, and
//! granting them Editor would hand over the label, the sheet, the abilities
//! and the inventory along with the picture. FR-030 gives them the picture
//! and nothing else, so the grant lives here and is called by exactly two
//! mutations, `uploadActorImage` and `removeActorImage`.
//!
//! # The rule
//!
//! Editor or above by the ladder may, always: a Game Master is untouched by
//! either switch (FR-032). Otherwise the caller may when, and only when, all
//! three hold:
//!
//! 1. they hold a live claim on the actor (`world_actor_claims`), which is
//!    also how a player who created their own character holds it;
//! 2. the actor's world has `allow_player_actor_art` on (FR-030a);
//! 3. the actor is not `art_locked` (FR-030b).
//!
//! Each failure is its own refusal with its own words, because SC-010 asks
//! that a player be told which rule stopped them.

use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::effective_actor_permission;
use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{world_actor_claims, world_actors, world_members, worlds};
use crate::state::AppState;

/// Why a caller may not change an actor's imagery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageryRefusal {
    /// Neither Editor on the actor nor the player holding it.
    NotHolder,
    /// The Game Master has turned the grant off for the whole world.
    WorldSettingOff,
    /// The Game Master has locked this one character's look.
    Locked,
}

impl ImageryRefusal {
    /// The sentence a refused player reads (contracts §5).
    pub fn message(self) -> &'static str {
        match self {
            ImageryRefusal::NotHolder => {
                "Only the player who holds this character may change its art."
            }
            ImageryRefusal::WorldSettingOff => {
                "The Game Master has turned off players changing their character's art in this world."
            }
            ImageryRefusal::Locked => "The Game Master has locked this character's look.",
        }
    }

    /// A stable name for the refusal, so a client can tell them apart
    /// without comparing sentences.
    pub fn reason(self) -> &'static str {
        match self {
            ImageryRefusal::NotHolder => "NOT_HOLDER",
            ImageryRefusal::WorldSettingOff => "PLAYER_ART_OFF",
            ImageryRefusal::Locked => "ART_LOCKED",
        }
    }

    pub fn into_error(self) -> Error {
        Error::new(self.message()).extend_with(|_, ext| {
            ext.set("code", "FORBIDDEN");
            ext.set("reason", self.reason());
        })
    }
}

/// B6 for the holder alone, given a connection: `Ok(())` when the caller's
/// claim, the world setting and the lock all allow it.
///
/// A lock is reported before the world setting when both apply: it is the
/// narrower of the two, and the one a player is likelier to ask about.
pub fn holder_may_change_imagery(
    conn: &mut PgConnection,
    user_id: Uuid,
    actor_id: Uuid,
) -> QueryResult<Result<(), ImageryRefusal>> {
    let holds = world_actor_claims::table
        .inner_join(world_members::table)
        .filter(world_actor_claims::actor_id.eq(actor_id))
        .filter(world_members::user_id.eq(user_id))
        .count()
        .get_result::<i64>(conn)?
        > 0;
    if !holds {
        return Ok(Err(ImageryRefusal::NotHolder));
    }

    let (locked, allowed) = world_actors::table
        .inner_join(worlds::table)
        .filter(world_actors::id.eq(actor_id))
        .select((world_actors::art_locked, worlds::allow_player_actor_art))
        .first::<(bool, bool)>(conn)?;
    if locked {
        return Ok(Err(ImageryRefusal::Locked));
    }
    if !allowed {
        return Ok(Err(ImageryRefusal::WorldSettingOff));
    }
    Ok(Ok(()))
}

/// The whole of B6: `Ok(Ok(()))` when the caller may change the actor's
/// portrait and token, `Ok(Err(refusal))` when they may not, and `Err` when
/// the question could not be answered (no such actor, no connection).
pub async fn may_change_actor_imagery(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<Result<(), ImageryRefusal>> {
    let level = effective_actor_permission(state, user_id, is_admin, actor_id).await?;
    if level.rank() >= ActorPermissionLevel::Editor.rank() {
        return Ok(Ok(()));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || holder_may_change_imagery(&mut conn, user_id, actor_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load actor"))
}

/// [`may_change_actor_imagery`], refusing with the refusal's own message.
pub async fn require_actor_imagery(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<()> {
    may_change_actor_imagery(state, user_id, is_admin, actor_id)
        .await?
        .map_err(ImageryRefusal::into_error)
}

#[cfg(test)]
#[path = "actor_imagery_tests.rs"]
mod tests;
