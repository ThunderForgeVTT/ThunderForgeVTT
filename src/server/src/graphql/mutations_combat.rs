//! Play-view Combat: a persisted, shared initiative tracker.
//!
//! Shared is the requirement that shapes everything here — the GM and every
//! player must see the same turn order and the same active combatant, so
//! turn state is server-authoritative and every mutation broadcasts
//! `EVENT_CODE_COMBAT_CHANGED` on the existing `world_events` bus. Clients
//! refetch the (small, always-read-whole) combat on the nudge.
//!
//! Authorization: every mutation here is GM-only (`is_dm_of_world`),
//! matching how the rest of the app treats scene/encounter authoring.
//! Reading is open to any world member (`require_world_member`) — players
//! must see the tracker they are in.
//!
//! Ordering is defined in exactly one place, `sort_combatants`, and is a
//! total order: initiative descending, then `tiebreak` descending, then id.
//! That last id term is not decoration. Without a deterministic final key,
//! two combatants tied on both initiative and tiebreak could come back in
//! whatever order Postgres happened to return, and "next turn" would step
//! through a different sequence on the GM's screen than on a player's —
//! precisely the disagreement a shared tracker exists to prevent.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{is_dm_of_world, require_world_member};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{Combat, Combatant, NewCombat, NewCombatant};
use crate::schema::{world_combatants, world_combats};
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_COMBAT_CHANGED, record_world_event};

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLCombatant {
    pub id: Uuid,
    pub combat_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub label: String,
    pub initiative: i32,
    pub tiebreak: i32,
    pub is_npc: bool,
    pub active: bool,
}

impl From<Combatant> for GraphQLCombatant {
    fn from(row: Combatant) -> Self {
        GraphQLCombatant {
            id: row.id,
            combat_id: row.combat_id,
            actor_id: row.actor_id,
            token_id: row.token_id,
            label: row.label,
            initiative: row.initiative,
            tiebreak: row.tiebreak,
            is_npc: row.is_npc,
            active: row.active,
        }
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLCombat {
    pub id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
    pub round: i32,
    pub active_combatant_id: Option<Uuid>,
    pub ended_at: Option<chrono::NaiveDateTime>,
    /// Already in turn order — clients render this as given and never
    /// re-sort, so there is one ordering rule in the system, not two.
    pub combatants: Vec<GraphQLCombatant>,
}

/// The single definition of turn order. See this module's doc comment for
/// why the trailing id comparison is load-bearing.
fn sort_combatants(combatants: &mut [Combatant]) {
    combatants.sort_by(|a, b| {
        b.initiative
            .cmp(&a.initiative)
            .then(b.tiebreak.cmp(&a.tiebreak))
            .then(a.id.cmp(&b.id))
    });
}

/// Index of the combatant that takes the turn after `active_id`.
///
/// Returns `(index, wrapped)` — `wrapped` is true when the turn passed the
/// end of the order, which is what increments the round. Skips inactive
/// (downed/removed) combatants. Returns `None` when nobody is eligible, so
/// a combat whose combatants are all inactive stops rather than spinning.
fn next_turn_index(ordered: &[Combatant], active_id: Option<Uuid>) -> Option<(usize, bool)> {
    if ordered.is_empty() {
        return None;
    }

    let current = active_id.and_then(|id| ordered.iter().position(|c| c.id == id));

    // From just past the current position, walk the whole ring exactly
    // once. Starting at `start + i` for i in 1..=len (rather than 0..len)
    // is what makes "next" skip the current combatant instead of landing
    // back on it when it is the only active one.
    let start = current.unwrap_or(0);
    let len = ordered.len();

    for offset in 1..=len {
        let idx = (start + offset) % len;
        if ordered[idx].active {
            // A wrap happened if we passed index 0 on the way. With no
            // current combatant we are entering the order for the first
            // time, which is not a new round.
            let wrapped = current.is_some() && start + offset >= len;
            return Some((idx, wrapped));
        }
    }

    None
}

/// Loads a combat plus its combatants, in turn order.
fn load_combat(conn: &mut PgConnection, combat: Combat) -> Result<GraphQLCombat, String> {
    let mut combatants = world_combatants::table
        .filter(world_combatants::combat_id.eq(combat.id))
        .select(Combatant::as_select())
        .load::<Combatant>(conn)
        .map_err(|e| format!("Failed to load combatants: {e}"))?;

    sort_combatants(&mut combatants);

    Ok(GraphQLCombat {
        id: combat.id,
        world_id: combat.world_id,
        scene_id: combat.scene_id,
        round: combat.round,
        active_combatant_id: combat.active_combatant_id,
        ended_at: combat.ended_at,
        combatants: combatants.into_iter().map(GraphQLCombatant::from).collect(),
    })
}

/// The world's running combat, if any. `ended_at IS NULL` is the same
/// predicate as the partial unique index, so this can only ever match one.
fn find_active_combat(conn: &mut PgConnection, world_id: Uuid) -> Result<Option<Combat>, String> {
    world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .select(Combat::as_select())
        .first::<Combat>(conn)
        .optional()
        .map_err(|e| format!("Failed to load combat: {e}"))
}

/// Loads a combat by id and returns it with the world it belongs to, so
/// callers can authorize against the world without trusting a
/// client-supplied world id.
fn combat_world(conn: &mut PgConnection, combat_id: Uuid) -> Result<Combat, String> {
    world_combats::table
        .filter(world_combats::id.eq(combat_id))
        .select(Combat::as_select())
        .first::<Combat>(conn)
        .map_err(|_| "Combat not found".to_string())
}

fn touch_and_broadcast(
    conn: &mut PgConnection,
    combat_id: Uuid,
    world_id: Uuid,
    user_id: Uuid,
) -> Result<(), String> {
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
        .set(world_combats::updated_at.eq(Utc::now().naive_utc()))
        .execute(conn)
        .map_err(|e| format!("Failed to touch combat: {e}"))?;

    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_COMBAT_CHANGED,
        Some(serde_json::json!({ "combatId": combat_id })),
        user_id,
    );
    Ok(())
}

// ============================================================================
// Inputs
// ============================================================================

#[derive(InputObject, Debug, Clone)]
pub struct StartCombatInput {
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
}

#[derive(InputObject, Debug, Clone)]
pub struct AddCombatantInput {
    pub combat_id: Uuid,
    pub label: String,
    pub actor_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub initiative: Option<i32>,
    pub tiebreak: Option<i32>,
    pub is_npc: Option<bool>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateCombatantInput {
    pub combatant_id: Uuid,
    pub initiative: Option<i32>,
    pub tiebreak: Option<i32>,
    pub active: Option<bool>,
    pub label: Option<String>,
}

// ============================================================================
// Implementations
// ============================================================================

/// Starts a combat, or returns the world's already-running one.
///
/// Idempotent by design rather than erroring on a second call: the partial
/// unique index makes a duplicate a hard database error, and a GM
/// double-clicking "Start combat" should get the encounter they already
/// have, not a failure.
pub async fn start_combat_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: StartCombatInput,
) -> GraphQLResult<GraphQLCombat> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the GM may start combat"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = input.world_id;
    let scene_id = input.scene_id;

    let combat = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        if let Some(existing) = find_active_combat(&mut conn, world_id)? {
            return load_combat(&mut conn, existing);
        }

        let combat = diesel::insert_into(world_combats::table)
            .values(&NewCombat {
                id: Uuid::now_v7(),
                world_id,
                scene_id,
                created_by: user_id,
            })
            .returning(Combat::as_returning())
            .get_result::<Combat>(&mut conn)
            .map_err(|e| format!("Failed to start combat: {e}"))?;

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_COMBAT_CHANGED,
            Some(serde_json::json!({ "combatId": combat.id })),
            user_id,
        );

        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(combat)
}

pub async fn add_combatant_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AddCombatantInput,
) -> GraphQLResult<GraphQLCombat> {
    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err(Error::new("Combatant needs a name"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat_id = input.combat_id;

    // The world is read from the combat row, never taken from the client,
    // so the GM check below cannot be aimed at a world the combat is not in.
    let world_id = {
        let mut authz_conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || combat_world(&mut authz_conn, combat_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(Error::new)?
            .world_id
    };

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM may change combat"));
    }

    let combat = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        diesel::insert_into(world_combatants::table)
            .values(&NewCombatant {
                id: Uuid::now_v7(),
                combat_id,
                actor_id: input.actor_id,
                token_id: input.token_id,
                label,
                initiative: input.initiative.unwrap_or(0),
                tiebreak: input.tiebreak.unwrap_or(0),
                is_npc: input.is_npc.unwrap_or(false),
            })
            .execute(&mut conn)
            .map_err(|e| format!("Failed to add combatant: {e}"))?;

        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;

        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(combat)
}

pub async fn update_combatant_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateCombatantInput,
) -> GraphQLResult<GraphQLCombat> {
    let combatant_id = input.combatant_id;

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat = tokio::task::spawn_blocking(move || -> Result<Combat, String> {
        let combat_id = world_combatants::table
            .filter(world_combatants::id.eq(combatant_id))
            .select(world_combatants::combat_id)
            .first::<Uuid>(&mut lookup_conn)
            .map_err(|_| "Combatant not found".to_string())?;
        combat_world(&mut lookup_conn, combat_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, combat.world_id).await? {
        return Err(Error::new("Only the GM may change combat"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat_id = combat.id;
    let world_id = combat.world_id;

    let updated = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        let now = Utc::now().naive_utc();

        // Applied as individual statements rather than one tuple `set`
        // because each field is independently optional — a `None` must
        // leave the stored value alone, not overwrite it with a default.
        if let Some(initiative) = input.initiative {
            diesel::update(world_combatants::table.filter(world_combatants::id.eq(combatant_id)))
                .set((
                    world_combatants::initiative.eq(initiative),
                    world_combatants::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update initiative: {e}"))?;
        }
        if let Some(tiebreak) = input.tiebreak {
            diesel::update(world_combatants::table.filter(world_combatants::id.eq(combatant_id)))
                .set((
                    world_combatants::tiebreak.eq(tiebreak),
                    world_combatants::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update tiebreak: {e}"))?;
        }
        if let Some(active) = input.active {
            diesel::update(world_combatants::table.filter(world_combatants::id.eq(combatant_id)))
                .set((
                    world_combatants::active.eq(active),
                    world_combatants::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update active: {e}"))?;
        }
        if let Some(label) = input.label.as_ref() {
            let label = label.trim();
            if label.is_empty() {
                return Err("Combatant needs a name".to_string());
            }
            diesel::update(world_combatants::table.filter(world_combatants::id.eq(combatant_id)))
                .set((
                    world_combatants::label.eq(label),
                    world_combatants::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to update label: {e}"))?;
        }

        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(updated)
}

pub async fn remove_combatant_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    combatant_id: Uuid,
) -> GraphQLResult<GraphQLCombat> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat = tokio::task::spawn_blocking(move || -> Result<Combat, String> {
        let combat_id = world_combatants::table
            .filter(world_combatants::id.eq(combatant_id))
            .select(world_combatants::combat_id)
            .first::<Uuid>(&mut lookup_conn)
            .map_err(|_| "Combatant not found".to_string())?;
        combat_world(&mut lookup_conn, combat_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, combat.world_id).await? {
        return Err(Error::new("Only the GM may change combat"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat_id = combat.id;
    let world_id = combat.world_id;
    let was_active_turn = combat.active_combatant_id == Some(combatant_id);

    let updated = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        // Hand the turn on *before* deleting when the combatant being
        // removed is the one currently acting. The FK is ON DELETE SET
        // NULL, so deleting first would silently drop the encounter back to
        // "no active turn" and the tracker would restart from the top of
        // the order on the GM's next click.
        if was_active_turn {
            let mut ordered = world_combatants::table
                .filter(world_combatants::combat_id.eq(combat_id))
                .select(Combatant::as_select())
                .load::<Combatant>(&mut conn)
                .map_err(|e| format!("Failed to load combatants: {e}"))?;
            sort_combatants(&mut ordered);

            let successor = next_turn_index(&ordered, Some(combatant_id))
                .map(|(idx, _)| ordered[idx].id)
                .filter(|id| *id != combatant_id);

            diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
                .set(world_combats::active_combatant_id.eq(successor))
                .execute(&mut conn)
                .map_err(|e| format!("Failed to hand off turn: {e}"))?;
        }

        diesel::delete(world_combatants::table.filter(world_combatants::id.eq(combatant_id)))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to remove combatant: {e}"))?;

        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(updated)
}

pub async fn advance_turn_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    combat_id: Uuid,
) -> GraphQLResult<GraphQLCombat> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat = tokio::task::spawn_blocking(move || combat_world(&mut lookup_conn, combat_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, combat.world_id).await? {
        return Err(Error::new("Only the GM may advance the turn"));
    }
    if combat.ended_at.is_some() {
        return Err(Error::new("This combat has already ended"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = combat.world_id;
    let active_id = combat.active_combatant_id;
    let round = combat.round;

    let updated = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        let mut ordered = world_combatants::table
            .filter(world_combatants::combat_id.eq(combat_id))
            .select(Combatant::as_select())
            .load::<Combatant>(&mut conn)
            .map_err(|e| format!("Failed to load combatants: {e}"))?;
        sort_combatants(&mut ordered);

        let Some((idx, wrapped)) = next_turn_index(&ordered, active_id) else {
            return Err("No active combatants to advance to".to_string());
        };

        diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
            .set((
                world_combats::active_combatant_id.eq(Some(ordered[idx].id)),
                world_combats::round.eq(if wrapped { round + 1 } else { round }),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to advance turn: {e}"))?;

        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(updated)
}

pub async fn end_combat_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    combat_id: Uuid,
) -> GraphQLResult<GraphQLCombat> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let combat = tokio::task::spawn_blocking(move || combat_world(&mut lookup_conn, combat_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, combat.world_id).await? {
        return Err(Error::new("Only the GM may end combat"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = combat.world_id;

    let updated = tokio::task::spawn_blocking(move || -> Result<GraphQLCombat, String> {
        diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
            .set(world_combats::ended_at.eq(Some(Utc::now().naive_utc())))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to end combat: {e}"))?;

        touch_and_broadcast(&mut conn, combat_id, world_id, user_id)?;
        let combat = combat_world(&mut conn, combat_id)?;
        load_combat(&mut conn, combat)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(updated)
}

pub async fn active_combat_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Option<GraphQLCombat>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let combat = tokio::task::spawn_blocking(move || -> Result<Option<GraphQLCombat>, String> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You are not a member of this world".to_string())?;

        match find_active_combat(&mut conn, world_id)? {
            None => Ok(None),
            Some(combat) => load_combat(&mut conn, combat).map(Some),
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(combat)
}

// ============================================================================
// GraphQL roots
// ============================================================================

#[derive(Default)]
pub struct CombatMutation;

#[async_graphql::Object]
impl CombatMutation {
    async fn start_combat(
        &self,
        ctx: &Context<'_>,
        input: StartCombatInput,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        start_combat_impl(state, user.user_id, user.is_admin, input).await
    }

    async fn add_combatant(
        &self,
        ctx: &Context<'_>,
        input: AddCombatantInput,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        add_combatant_impl(state, user.user_id, user.is_admin, input).await
    }

    async fn update_combatant(
        &self,
        ctx: &Context<'_>,
        input: UpdateCombatantInput,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        update_combatant_impl(state, user.user_id, user.is_admin, input).await
    }

    async fn remove_combatant(
        &self,
        ctx: &Context<'_>,
        combatant_id: Uuid,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        remove_combatant_impl(state, user.user_id, user.is_admin, combatant_id).await
    }

    async fn advance_turn(
        &self,
        ctx: &Context<'_>,
        combat_id: Uuid,
    ) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        advance_turn_impl(state, user.user_id, user.is_admin, combat_id).await
    }

    async fn end_combat(&self, ctx: &Context<'_>, combat_id: Uuid) -> GraphQLResult<GraphQLCombat> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        end_combat_impl(state, user.user_id, user.is_admin, combat_id).await
    }
}

#[derive(Default)]
pub struct CombatQuery;

#[async_graphql::Object]
impl CombatQuery {
    /// This world's running combat, or null when none is in progress.
    async fn active_combat(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLCombat>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        active_combat_impl(state, user.user_id, world_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combatant(id: u128, initiative: i32, tiebreak: i32, active: bool) -> Combatant {
        let now = Utc::now().naive_utc();
        Combatant {
            id: Uuid::from_u128(id),
            combat_id: Uuid::nil(),
            actor_id: None,
            token_id: None,
            label: format!("c{id}"),
            initiative,
            tiebreak,
            is_npc: false,
            active,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn sort_is_initiative_then_tiebreak_then_id() {
        let mut rows = vec![
            combatant(3, 10, 0, true),
            combatant(1, 20, 0, true),
            combatant(2, 10, 5, true),
        ];
        sort_combatants(&mut rows);
        let order: Vec<u128> = rows.iter().map(|c| c.id.as_u128()).collect();
        assert_eq!(order, vec![1, 2, 3]);
    }

    /// The id tiebreaker must produce a total order even when initiative
    /// and tiebreak are identical — otherwise the GM and a player can walk
    /// the turn order in different sequences.
    #[test]
    fn sort_is_deterministic_for_fully_tied_combatants() {
        let mut ascending = vec![combatant(1, 10, 0, true), combatant(2, 10, 0, true)];
        let mut descending = vec![combatant(2, 10, 0, true), combatant(1, 10, 0, true)];

        sort_combatants(&mut ascending);
        sort_combatants(&mut descending);

        let a: Vec<u128> = ascending.iter().map(|c| c.id.as_u128()).collect();
        let d: Vec<u128> = descending.iter().map(|c| c.id.as_u128()).collect();
        assert_eq!(a, d, "input order must not affect the resulting turn order");
    }

    #[test]
    fn next_turn_walks_the_order_and_wraps_into_a_new_round() {
        let ordered = vec![
            combatant(1, 20, 0, true),
            combatant(2, 10, 0, true),
            combatant(3, 5, 0, true),
        ];

        // Entering the order for the first time is not a new round.
        let (idx, wrapped) = next_turn_index(&ordered, None).unwrap();
        assert_eq!(ordered[idx].id.as_u128(), 2);
        assert!(!wrapped);

        let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
        assert_eq!(ordered[idx].id.as_u128(), 2);
        assert!(!wrapped);

        // Past the end of the order → back to the top, new round.
        let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(3))).unwrap();
        assert_eq!(ordered[idx].id.as_u128(), 1);
        assert!(wrapped);
    }

    #[test]
    fn next_turn_skips_inactive_combatants() {
        let ordered = vec![
            combatant(1, 20, 0, true),
            combatant(2, 10, 0, false),
            combatant(3, 5, 0, true),
        ];
        let (idx, _) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
        assert_eq!(ordered[idx].id.as_u128(), 3);
    }

    /// A combat where everyone is downed has nowhere to advance to — it
    /// must report that rather than looping forever or re-selecting the
    /// current combatant.
    #[test]
    fn next_turn_is_none_when_nobody_is_active() {
        let ordered = vec![combatant(1, 20, 0, false), combatant(2, 10, 0, false)];
        assert!(next_turn_index(&ordered, Some(Uuid::from_u128(1))).is_none());
        assert!(next_turn_index(&ordered, None).is_none());
        assert!(next_turn_index(&[], None).is_none());
    }

    /// The sole active combatant keeps taking turns, and each pass counts
    /// as a new round.
    #[test]
    fn next_turn_repeats_a_lone_active_combatant_as_new_rounds() {
        let ordered = vec![combatant(1, 20, 0, true), combatant(2, 10, 0, false)];
        let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
        assert_eq!(ordered[idx].id.as_u128(), 1);
        assert!(wrapped);
    }
}
