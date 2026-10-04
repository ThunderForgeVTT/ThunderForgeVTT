//! `changeHitPoints`: the Game Master damages or heals a creature by hand
//! (spec 046 FR-014, contracts/fight.md §1).
//!
//! Game Master only. A player's creature is never changed without its
//! controller's say-so (decision 1); that path is an offer, and arrives with
//! attacks. This is the table's referee reaching over to move a number, which
//! is what a Game Master has always been able to do at a real table.
//!
//! The operation itself is `crate::combat::hit_points`; this file is only who
//! may reach it.

use async_graphql::{Context, Error, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::combat::hit_points::{HitPointChangeKind, apply_hit_point_change};
use crate::graphql::{app_state, authenticated_user};
use crate::play_pause::gate::refuse_if_paused;
use crate::state::AppState;

/// A creature's hit points after a change.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TokenHitPoints")]
pub struct GraphQLTokenHitPoints {
    pub token_id: Uuid,
    pub current: i32,
    pub max: i32,
    pub temporary: i32,
}

pub async fn change_hit_points_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    token_id: Uuid,
    kind: HitPointChangeKind,
    amount: i32,
) -> GraphQLResult<GraphQLTokenHitPoints> {
    // The world is read from the token, never taken from the client, so the
    // Game Master check cannot be aimed at a world the token is not in.
    let world_id = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens};
            tokens::table
                .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
                .filter(tokens::token_id.eq(token_id))
                .select(scenes::world_id)
                .first::<Uuid>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load token"))?
        .ok_or_else(|| Error::new("Token not found"))?
    };

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the Game Master may change a creature's hit points",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    refuse_if_paused(&mut conn, world_id)?;

    let systems_dir = state.directories.systems_dir.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        apply_hit_point_change(&mut conn, &systems_dir, token_id, kind, amount, user_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLTokenHitPoints {
        token_id: outcome.token_id,
        current: outcome.after.current,
        max: outcome.after.max,
        temporary: outcome.after.temporary,
    })
}

#[derive(Default)]
pub struct CombatHitPointsMutation;

#[async_graphql::Object]
impl CombatHitPointsMutation {
    /// Damage or heal a creature by hand. Game Master only.
    ///
    /// Temporary hit points absorb damage first, current stops at zero, and
    /// healing stops at the maximum. A creature taken to zero is marked out of
    /// the running combat; healed above zero, it is back in — unless the Game
    /// Master took it out by hand.
    async fn change_hit_points(
        &self,
        ctx: &Context<'_>,
        token_id: Uuid,
        kind: HitPointChangeKind,
        amount: i32,
    ) -> GraphQLResult<GraphQLTokenHitPoints> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        change_hit_points_impl(state, user.user_id, user.is_admin, token_id, kind, amount).await
    }
}
