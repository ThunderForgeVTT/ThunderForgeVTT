//! Legendary actions (spec 046 US6, FR-050–FR-052, research R14).
//!
//! A creature may take legendary actions at the end of other creatures'
//! turns. How many it has each round is the pack's to say, through
//! `combat.legendary` (5e: `traitData.legendary_actions`); shared code never
//! names the field (contract M5).
//!
//! # When the pool is read
//!
//! Once, when the creature joins the turn order (`addCombatant`): its sheet's
//! number is copied to `legendary_per_round`, and `legendary_remaining` starts
//! full. A sheet edited mid-fight does not change a pool already in the
//! tracker; the Game Master removes and re-adds the creature for that, the way
//! initiative is re-rolled. A creature with no number recorded, or 0, has no
//! pool: both columns stay null and the tracker shows no legendary line.
//!
//! # Spent and refilled
//!
//! `makeAttack` with an ability or item whose `action_cost` is `legendary`
//! spends its `legendary_cost` from the pool (`budget::spend_for_attack`), and
//! the start of the creature's own turn refills it (`budget::start_turn`).
//! Nothing is refused (decision 2): a pool spent past zero goes negative and
//! the attack is flagged `overspent`; one taken on the creature's own turn is
//! flagged `legendary_on_own_turn`.

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::hit_points::slot_column;
use crate::combat::manifest::{combat_for_system, slot_key};
use crate::models::ActorSystemData;
use crate::schema::{tokens, world_actor_system_data, world_combatant_budgets};

/// The legendary actions a creature has each round, as its sheet records them
/// through the world's system's `combat.legendary`. `None` when the system
/// declares no pool, the creature has no sheet, or the number is absent or 0.
///
/// The creature is the token's actor (a copy reads its NPC's sheet), else the
/// combatant's own actor.
pub fn per_round_for(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    token_id: Option<Uuid>,
    actor_id: Option<Uuid>,
) -> QueryResult<Option<i32>> {
    let Some(system_id) = crate::combat::budget::world_system(conn, world_id)? else {
        return Ok(None);
    };
    let Some(declared) = combat_for_system(systems_dir, &system_id).legendary.clone() else {
        return Ok(None);
    };
    let token_actor = match token_id {
        Some(token_id) => tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select(tokens::actor_id)
            .first::<Option<Uuid>>(conn)
            .optional()?
            .flatten(),
        None => None,
    };
    let Some(actor_id) = token_actor.or(actor_id) else {
        return Ok(None);
    };
    let sheets = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(ActorSystemData::as_select())
        .load::<ActorSystemData>(conn)?;
    let Some(sheet) = sheets
        .iter()
        .max_by_key(|row| row.game_system_id == system_id)
    else {
        return Ok(None);
    };
    Ok(slot_column(sheet, &slot_key(&declared.slot))
        .as_ref()
        .and_then(|slot| slot.get(&declared.field))
        .and_then(|value| value.as_i64())
        .filter(|n| *n > 0)
        .map(|n| n.min(i32::MAX as i64) as i32))
}

/// A new combatant's budget, nothing spent and its legendary pool full. A
/// second call is harmless.
pub fn create_budget_for(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    combatant_id: Uuid,
    token_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    user_id: Uuid,
) -> QueryResult<()> {
    let per_round = per_round_for(conn, systems_dir, world_id, token_id, actor_id)?;
    diesel::insert_into(world_combatant_budgets::table)
        .values((
            world_combatant_budgets::combatant_id.eq(combatant_id),
            world_combatant_budgets::legendary_per_round.eq(per_round),
            world_combatant_budgets::legendary_remaining.eq(per_round),
            world_combatant_budgets::created_by.eq(user_id),
            world_combatant_budgets::updated_by.eq(user_id),
        ))
        .on_conflict(world_combatant_budgets::combatant_id)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

#[cfg(test)]
#[path = "legendary_tests.rs"]
mod tests;
