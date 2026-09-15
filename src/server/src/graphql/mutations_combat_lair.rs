//! `addLairCombatant`: the Game Master puts a lair in the turn order (spec
//! 046 US6, FR-053, contracts/fight.md §1).
//!
//! Beside `mutations_combat.rs` (the tracker) so that file stays under the
//! length check. What a lair is, and how it acts, is `crate::combat::lair`.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::combat::lair::{LAIR_INITIATIVE, LAIR_TIEBREAK};
use crate::combat::records::KIND_LAIR;
use crate::graphql::mutations_combat::{
    GraphQLCombat, combat_world, load_combat, touch_and_broadcast,
};
use crate::graphql::{app_state, authenticated_user};
use crate::play_pause::gate::refuse_if_paused;
use crate::schema::world_combatants;
use crate::state::AppState;

/// Add a lair to a running encounter: no token, no actor, at initiative
/// count 20, losing ties. Game Master only.
pub async fn add_lair_combatant_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    combat_id: Uuid,
    label: String,
) -> GraphQLResult<GraphQLCombat> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(Error::new("A lair needs a name"));
    }

    // The world is read from the combat row, never taken from the client.
    let combat = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || combat_world(&mut conn, combat_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(Error::new)?
    };
    let world_id = combat.world_id;
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM may change combat"));
    }
    if combat.ended_at.is_some() {
        return Err(Error::new("This combat has already ended"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    refuse_if_paused(&mut conn, world_id)?;
    let systems_dir = state.directories.systems_dir.clone();

    tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_combatants::table)
            .values((
                world_combatants::id.eq(Uuid::now_v7()),
                world_combatants::combat_id.eq(combat_id),
                world_combatants::label.eq(&label),
                world_combatants::initiative.eq(LAIR_INITIATIVE),
                world_combatants::tiebreak.eq(LAIR_TIEBREAK),
                world_combatants::is_npc.eq(true),
                world_combatants::kind.eq(KIND_LAIR),
                world_combatants::created_at.eq(now),
                world_combatants::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to add the lair: {e}"))?;
        // No budget row: a lair has none (`combat::budget`).
        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, &systems_dir, combat, user_id, is_admin)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct CombatLairMutation;

#[async_graphql::Object]
impl CombatLairMutation {
    /// Put a lair in the turn order at initiative count 20, losing ties. Game
    /// Master only. A lair acts through `makeAttack` with `lairCombatantId`.
    async fn add_lair_combatant(
        &self,
        ctx: &Context<'_>,
        combat_id: Uuid,
        label: String,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        add_lair_combatant_impl(state, user.user_id, user.is_admin, combat_id, label).await
    }
}
