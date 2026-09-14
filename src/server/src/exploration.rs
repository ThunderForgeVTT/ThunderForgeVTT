//! Whether a scene remembers where a player has been, and resetting it.
//!
//! Spec 045 US7, owner decision 3. **What a player has explored is not here.**
//! It is in that player's own browser, and clearing their storage loses it —
//! which is theirs to lose. The server holds two things only: whether a scene
//! remembers at all, and a number that tells a browser its memory is stale.
//!
//! # Why an epoch and not a broadcast
//!
//! A broadcast only reaches whoever is listening. A player who was offline
//! when the Game Master reset the fog would come back with their old map
//! intact and no way to know it was meant to be gone. An epoch is a fact the
//! client checks on arrival, so the reset survives the player being away — and
//! it needs no per-player state on the server at all, except for the one case
//! that genuinely does.
//!
//! # Two epochs, and taking the greater
//!
//! A reset for everyone bumps the scene's own epoch. A reset for one player
//! writes a row for that player. A client takes whichever is greater, so
//! resetting one player does not wipe the table's maps, and a later reset for
//! everyone still reaches a player who was reset individually.

use diesel::prelude::*;

use crate::AppState;
use crate::play_pause::gate::refuse_world_if_paused;
use async_graphql::{Error, Result as GraphQLResult};
use uuid::Uuid;

/// What a client needs to decide whether its stored map is still good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplorationState {
    pub enabled: bool,
    /// The scene's own epoch — a reset for everyone.
    pub epoch: i32,
    /// The greater of the scene's epoch and this viewer's own reset row.
    ///
    /// Resolved server-side rather than handing a client both and asking it
    /// to compare: a client that took the wrong one would keep a map it was
    /// told to drop, and the mistake would be invisible.
    pub mine: i32,
}

/// Read a scene's exploration state for one viewer.
pub fn state_for(
    conn: &mut PgConnection,
    scene_id: Uuid,
    user_id: Uuid,
) -> QueryResult<ExplorationState> {
    use crate::schema::{scene_exploration_resets, scenes};

    let (enabled, epoch): (bool, i32) = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select((scenes::exploration_enabled, scenes::exploration_epoch))
        .first(conn)?;

    let mine: Option<i32> = scene_exploration_resets::table
        .filter(scene_exploration_resets::scene_id.eq(scene_id))
        .filter(scene_exploration_resets::user_id.eq(user_id))
        .select(scene_exploration_resets::epoch)
        .first(conn)
        .optional()?;

    Ok(ExplorationState {
        enabled,
        epoch,
        mine: mine.unwrap_or(epoch).max(epoch),
    })
}

/// Turn a scene's memory on or off (Game Master only).
pub async fn set_enabled(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    enabled: bool,
) -> GraphQLResult<bool> {
    let world_id = require_game_master(state, user_id, is_admin, scene_id).await?;
    refuse_world_if_paused(state, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
            .set(scenes::exploration_enabled.eq(enabled))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to set exploration: {e}")))?;

    Ok(enabled)
}

/// Reset what has been explored — for one player, or for everyone.
///
/// `for_user` of `None` means everyone, and bumps the scene's own epoch.
pub async fn reset(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    for_user: Option<Uuid>,
) -> GraphQLResult<i32> {
    let world_id = require_game_master(state, user_id, is_admin, scene_id).await?;
    refuse_world_if_paused(state, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let epoch = tokio::task::spawn_blocking(move || {
        use crate::schema::{scene_exploration_resets, scenes};

        match for_user {
            // Everyone: the scene's epoch must end up greater than every
            // number any client is holding — including a player who was reset
            // on their own and is holding *their row's* number, which is
            // already above the scene's.
            //
            // Simply incrementing the scene does not do that. A player reset
            // alone holds 1 while the scene holds 0; incrementing takes the
            // scene to 1, they take the greater of the two, and it is still 1
            // — so the reset for everyone reaches everyone except the player
            // who was reset most recently, which is precisely backwards.
            None => {
                let highest_row: Option<i32> = scene_exploration_resets::table
                    .filter(scene_exploration_resets::scene_id.eq(scene_id))
                    .select(diesel::dsl::max(scene_exploration_resets::epoch))
                    .first(&mut conn)?;
                let scene_epoch: i32 = scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .select(scenes::exploration_epoch)
                    .first(&mut conn)?;
                let next = scene_epoch.max(highest_row.unwrap_or(0)) + 1;

                diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                    .set(scenes::exploration_epoch.eq(next))
                    .execute(&mut conn)?;
                // The rows are subsumed: every one of them is now below the
                // scene's own number and can only be noise for the next
                // per-player reset to reason about.
                diesel::delete(
                    scene_exploration_resets::table
                        .filter(scene_exploration_resets::scene_id.eq(scene_id)),
                )
                .execute(&mut conn)?;
                Ok::<i32, diesel::result::Error>(next)
            }
            Some(target) => {
                // One player: their row goes past the scene's epoch, so it is
                // the greater of the two and only they drop their map.
                let scene_epoch: i32 = scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .select(scenes::exploration_epoch)
                    .first(&mut conn)?;
                let existing: Option<i32> = scene_exploration_resets::table
                    .filter(scene_exploration_resets::scene_id.eq(scene_id))
                    .filter(scene_exploration_resets::user_id.eq(target))
                    .select(scene_exploration_resets::epoch)
                    .first(&mut conn)
                    .optional()?;
                let next = existing.unwrap_or(scene_epoch).max(scene_epoch) + 1;

                diesel::insert_into(scene_exploration_resets::table)
                    .values((
                        scene_exploration_resets::scene_id.eq(scene_id),
                        scene_exploration_resets::user_id.eq(target),
                        scene_exploration_resets::epoch.eq(next),
                    ))
                    .on_conflict((
                        scene_exploration_resets::scene_id,
                        scene_exploration_resets::user_id,
                    ))
                    .do_update()
                    .set(scene_exploration_resets::epoch.eq(next))
                    .execute(&mut conn)?;
                Ok::<i32, diesel::result::Error>(next)
            }
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to reset exploration: {e}")))?;

    // Announced as a scene change, so a client that is watching drops its map
    // without waiting to be asked. A client that was away finds out by
    // comparing epochs when it next opens the scene, which is what the epoch
    // is for — the announcement is the fast path, not the mechanism.
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    if let Ok(world_id) = crate::world_events::world_id_for_scene(&mut conn, scene_id) {
        let _ = crate::world_events::record_world_event(
            &mut conn,
            world_id,
            crate::world_events::EVENT_CODE_SCENE_EXPLORATION_RESET,
            Some(serde_json::json!({
                "action": "reset",
                "sceneId": scene_id,
                "forUser": for_user,
                "epoch": epoch,
            })),
            user_id,
        );
    }

    Ok(epoch)
}

/// Exploration is the Game Master's to turn on and to reset.
///
/// The broader `is_dm_of_world` check, matching `update_scene_hidden`: any
/// Game Master or Owner of the scene's world, not only whoever created the
/// scene.
async fn require_game_master(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Scene not found"))?;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change or reset exploration",
        ));
    }
    Ok(world_id)
}

#[cfg(test)]
#[path = "exploration_tests.rs"]
mod tests;
