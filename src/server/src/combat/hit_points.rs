//! Damage and healing as one server operation (spec 046 research R5, R9).
//!
//! Before this, a creature's hit points changed only by somebody rewriting
//! the whole of its resource blob with `updateActorSystemData`: nothing spent
//! temporary hit points, nothing stopped at zero, two writes racing each other
//! lost one, and a creature at zero kept taking turns. This is the one path
//! damage takes — the Game Master's Damage and Heal buttons now, a taken offer
//! and auto-apply later — so each of those is the same arithmetic, the same
//! lock and the same announcement.
//!
//! # What it does, in order
//!
//! 1. Locks the record it writes (`SELECT … FOR UPDATE`), so two changes that
//!    land together are applied one after the other against what the first
//!    left (spec edge case "two attacks at once").
//! 2. Reads the pack's declared hit-point fields (contract M1: a pack that
//!    declares none has no damage operation, and says so).
//! 3. Spends temporary hit points first, bounds current at zero, and bounds
//!    healing at the maximum (C7).
//! 4. Validates the result with the pack's own validator, and writes it.
//! 5. Updates the creature's combatant when it reaches zero, or is healed from
//!    it (C8), in the same transaction.
//! 6. After the commit, records event 26 (the sheet moved) for a linked token
//!    or 14 (the token changed) for a copy, and 18 when a combatant changed.
//!
//! # Which record (ADR-102)
//!
//! A **linked** token's record is its actor's `world_actor_system_data` row,
//! and that row is what is locked and written. An **unlinked copy**'s record
//! is its own `tokens.system_data`, and the token row is locked instead; the
//! actor it was copied from, and every other copy of it, are left alone.

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::manifest::{SystemHitPoints, combat_for_system, slot_key};
use crate::schema::{
    scenes, tokens, world_actor_system_data, world_actors, world_combatants, world_combats, worlds,
};
use crate::world_events::{
    EVENT_CODE_ACTOR_SHEET_CHANGED, EVENT_CODE_COMBAT_CHANGED, EVENT_CODE_TOKEN_CHANGED,
    record_world_event,
};

/// Which way a change goes.
#[derive(async_graphql::Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "HitPointChange")]
pub enum HitPointChangeKind {
    Damage,
    Healing,
}

/// A creature's hit points, as the pack declares them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HitPoints {
    pub current: i32,
    pub max: i32,
    pub temporary: i32,
}

/// Why a combatant is out of the fight (`world_combatants.downed_by`).
pub const DOWNED_BY_HIT_POINTS: &str = "hit_points";
pub const DOWNED_BY_GAME_MASTER: &str = "game_master";

/// The arithmetic, and nothing else (C7).
///
/// Damage spends temporary hit points first and stops current at zero; the
/// excess is not carried anywhere (spec edge case "damage beyond zero").
/// Healing never exceeds the maximum and never touches temporary hit points,
/// which are granted rather than healed. A current already above the maximum
/// — which a sheet may hold — is not cut down by healing.
pub fn apply_to(before: HitPoints, kind: HitPointChangeKind, amount: i32) -> HitPoints {
    let amount = amount.max(0);
    match kind {
        HitPointChangeKind::Damage => {
            let temporary = before.temporary.max(0);
            let absorbed = temporary.min(amount);
            let rest = amount - absorbed;
            HitPoints {
                current: (before.current - rest).max(0),
                max: before.max,
                temporary: temporary - absorbed,
            }
        }
        HitPointChangeKind::Healing => HitPoints {
            current: if before.current >= before.max {
                before.current
            } else {
                before.current.saturating_add(amount).min(before.max)
            },
            max: before.max,
            temporary: before.temporary,
        },
    }
}

/// What a change did.
#[derive(Clone, Debug)]
pub struct HitPointChangeOutcome {
    pub token_id: Uuid,
    pub scene_id: Uuid,
    /// The actor whose sheet was written; `None` when a copy's own record was.
    pub actor_id: Option<Uuid>,
    pub world_id: Uuid,
    pub before: HitPoints,
    pub after: HitPoints,
    /// The combat whose combatants this change marked out or brought back.
    pub combat_changed: Option<Uuid>,
    /// The slot written, in `data_types` spelling (`resource_data`).
    pub data_type: String,
}

fn read_int(slot: &serde_json::Value, field: &str) -> Option<i32> {
    slot.get(field)
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// Read the declared fields out of a slot's JSON.
///
/// A missing `current` reads as the maximum — a creature whose sheet names
/// only its maximum is at full health, which is what 5e's validator allows.
/// A missing maximum is not guessed at.
pub fn read_hit_points(
    slot: &serde_json::Value,
    declared: &SystemHitPoints,
) -> Result<HitPoints, String> {
    let max = read_int(slot, &declared.max)
        .ok_or_else(|| "That creature has no maximum hit points recorded".to_string())?;
    Ok(HitPoints {
        current: read_int(slot, &declared.current).unwrap_or(max),
        max,
        temporary: declared
            .temporary
            .as_deref()
            .and_then(|field| read_int(slot, field))
            .unwrap_or(0),
    })
}

/// Write the declared fields back, leaving everything else in the slot alone.
pub fn write_hit_points(
    slot: &serde_json::Value,
    declared: &SystemHitPoints,
    value: HitPoints,
) -> serde_json::Value {
    let mut out = slot.as_object().cloned().unwrap_or_default();
    out.insert(declared.current.clone(), value.current.into());
    if let Some(field) = &declared.temporary
        && (out.contains_key(field) || value.temporary != 0)
    {
        out.insert(field.clone(), value.temporary.into());
    }
    serde_json::Value::Object(out)
}

/// The system data column a slot names.
pub(crate) fn slot_column(
    row: &crate::models::ActorSystemData,
    key: &str,
) -> Option<serde_json::Value> {
    match key {
        "ability_data" => row.ability_data.clone(),
        "resource_data" => row.resource_data.clone(),
        "proficiency_data" => row.proficiency_data.clone(),
        "trait_data" => row.trait_data.clone(),
        "spell_data" => row.spell_data.clone(),
        _ => None,
    }
}

fn write_slot_column(
    conn: &mut PgConnection,
    row_id: Uuid,
    key: &str,
    value: &serde_json::Value,
    user_id: Uuid,
) -> QueryResult<usize> {
    use world_actor_system_data as d;
    let target = d::table.filter(d::id.eq(row_id));
    let now = chrono::Utc::now().naive_utc();
    let stamp = (d::updated_by.eq(user_id), d::updated_at.eq(now));
    match key {
        "ability_data" => diesel::update(target)
            .set((d::ability_data.eq(value), stamp))
            .execute(conn),
        "resource_data" => diesel::update(target)
            .set((d::resource_data.eq(value), stamp))
            .execute(conn),
        "proficiency_data" => diesel::update(target)
            .set((d::proficiency_data.eq(value), stamp))
            .execute(conn),
        "trait_data" => diesel::update(target)
            .set((d::trait_data.eq(value), stamp))
            .execute(conn),
        "spell_data" => diesel::update(target)
            .set((d::spell_data.eq(value), stamp))
            .execute(conn),
        _ => Err(diesel::result::Error::NotFound),
    }
}

/// Mark the creature's combatants out at zero, or back in above it (C8).
///
/// Matched through `token_id`, or — for a **linked** token only — through
/// `actor_id` for a combatant that has no token. A copy passes `None`: its hit
/// points are its own, so a combatant that shares only its actor is a
/// different creature, and falling back to the actor would mark out every
/// goblin in the fight when one of them dropped. Only the world's running
/// combat is touched. Returns that combat's id when any combatant changed.
fn follow_zero(
    conn: &mut PgConnection,
    world_id: Uuid,
    token_id: Uuid,
    linked_actor_id: Option<Uuid>,
    after: HitPoints,
) -> Result<Option<Uuid>, String> {
    let Some(combat_id) = world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .select(world_combats::id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(|e| format!("Failed to load combat: {e}"))?
    else {
        return Ok(None);
    };

    // `actor_id = NULL` matches nothing, which is how a copy opts out of the
    // fallback without a second query shape.
    let is_this_creature = world_combatants::token_id
        .eq(token_id)
        .or(world_combatants::token_id
            .is_null()
            .and(world_combatants::actor_id.eq(linked_actor_id)));
    let now = chrono::Utc::now().naive_utc();

    let changed = if after.current <= 0 {
        // Out. A combatant the Game Master already took down stays theirs:
        // only a combatant still in the fight is marked by hit points.
        diesel::update(
            world_combatants::table
                .filter(world_combatants::combat_id.eq(combat_id))
                .filter(is_this_creature)
                .filter(world_combatants::active.eq(true)),
        )
        .set((
            world_combatants::active.eq(false),
            world_combatants::downed_by.eq(Some(DOWNED_BY_HIT_POINTS)),
            world_combatants::updated_at.eq(now),
        ))
        .execute(conn)
    } else {
        // Back in — but only a combatant hit points took out (FR-022).
        diesel::update(
            world_combatants::table
                .filter(world_combatants::combat_id.eq(combat_id))
                .filter(is_this_creature)
                .filter(world_combatants::downed_by.eq(DOWNED_BY_HIT_POINTS)),
        )
        .set((
            world_combatants::active.eq(true),
            world_combatants::downed_by.eq(None::<String>),
            world_combatants::updated_at.eq(now),
        ))
        .execute(conn)
    }
    .map_err(|e| format!("Failed to update combatant: {e}"))?;

    if changed == 0 {
        return Ok(None);
    }
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
        .set(world_combats::updated_at.eq(now))
        .execute(conn)
        .map_err(|e| format!("Failed to touch combat: {e}"))?;
    Ok(Some(combat_id))
}

/// A transaction's failure: a sentence for the caller, whichever layer it
/// came from. Diesel needs `From<diesel::result::Error>` to roll back.
struct Refusal(String);

impl From<diesel::result::Error> for Refusal {
    fn from(error: diesel::result::Error) -> Self {
        Refusal(format!("Failed to change hit points: {error}"))
    }
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Refusal(message)
    }
}

/// Change a creature's hit points.
///
/// Authorization is the caller's: this is the operation, and the Game
/// Master's button, a taken offer and auto-apply each decide for themselves
/// who may reach it. `amount` below zero is refused rather than read as the
/// opposite kind — a negative damage that heals is a bug waiting in a form.
pub fn apply_hit_point_change(
    conn: &mut PgConnection,
    systems_dir: &str,
    token_id: Uuid,
    kind: HitPointChangeKind,
    amount: i32,
    user_id: Uuid,
) -> Result<HitPointChangeOutcome, String> {
    if amount < 0 {
        return Err("An amount of hit points cannot be negative".to_string());
    }

    let outcome = conn.transaction::<_, Refusal, _>(|conn| {
        let (scene_id, actor_id, linked, world_id) = tokens::table
            .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
            .filter(tokens::token_id.eq(token_id))
            .select((
                tokens::scene_id,
                tokens::actor_id,
                tokens::linked,
                scenes::world_id,
            ))
            .first::<(Uuid, Option<Uuid>, bool, Uuid)>(conn)
            .optional()
            .map_err(|e| format!("Failed to load token: {e}"))?
            .ok_or_else(|| "Token not found".to_string())?;

        // The system is the actor's, else the world's. A copy whose actor
        // was deleted still has a world.
        let actor_system = match actor_id {
            Some(actor_id) => world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .select(world_actors::game_system_id)
                .first::<Option<String>>(conn)
                .optional()
                .map_err(|e| format!("Failed to load actor: {e}"))?
                .flatten(),
            None => None,
        };
        let system_id = match actor_system {
            Some(id) => id,
            None => worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first::<Option<String>>(conn)
                .map_err(|e| format!("Failed to load world: {e}"))?
                .ok_or_else(|| "This world has no game system".to_string())?,
        };

        let combat = combat_for_system(systems_dir, &system_id);
        let declared = combat.hit_points.clone().ok_or_else(|| {
            format!("The {system_id} game system declares no hit points, so there is nothing to change")
        })?;
        let key = slot_key(&declared.slot);

        let (before, after, written_actor) = if linked {
            let actor_id = actor_id
                .ok_or_else(|| "That token is not a creature with hit points".to_string())?;
            // The lock. A second change to the same creature waits here until
            // the first commits, then reads what the first left.
            let row = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq(actor_id))
                .select(crate::models::ActorSystemData::as_select())
                .for_update()
                .first::<crate::models::ActorSystemData>(conn)
                .optional()
                .map_err(|e| format!("Failed to load hit points: {e}"))?
                .ok_or_else(|| "That creature has no hit points recorded".to_string())?;
            let slot = slot_column(&row, &key)
                .ok_or_else(|| "That creature has no hit points recorded".to_string())?;

            let before = read_hit_points(&slot, &declared)?;
            let after = apply_to(before, kind, amount);
            let written = write_hit_points(&slot, &declared, after);

            crate::systems::validate_actor_system_data(&row.game_system_id, &key, &written)
                .map_err(|e| format!("Validation failed: {e}"))?;
            write_slot_column(conn, row.id, &key, &written, user_id)
                .map_err(|e| format!("Failed to write hit points: {e}"))?;
            (before, after, Some(actor_id))
        } else {
            // A copy holds a `resource_data`-shaped record and nothing else.
            if key != "resource_data" {
                return Err(format!(
                    "The {system_id} game system keeps hit points outside a creature's resources, which a copy does not hold"
                )
                .into());
            }
            // The lock is the token row: copies of one NPC never wait on
            // each other, and never on the NPC.
            let slot = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(tokens::system_data)
                .for_update()
                .first::<Option<serde_json::Value>>(conn)
                .map_err(|e| format!("Failed to load hit points: {e}"))?
                .ok_or_else(|| "That creature has no hit points recorded".to_string())?;

            let before = read_hit_points(&slot, &declared)?;
            let after = apply_to(before, kind, amount);
            let written = write_hit_points(&slot, &declared, after);

            crate::systems::validate_actor_system_data(&system_id, &key, &written)
                .map_err(|e| format!("Validation failed: {e}"))?;
            diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                .set((
                    tokens::system_data.eq(Some(&written)),
                    tokens::updated_at.eq(chrono::Utc::now().naive_utc()),
                ))
                .execute(conn)
                .map_err(|e| format!("Failed to write hit points: {e}"))?;
            (before, after, None)
        };

        let combat_changed = follow_zero(conn, world_id, token_id, written_actor, after)?;

        Ok(HitPointChangeOutcome {
            token_id,
            scene_id,
            actor_id: written_actor,
            world_id,
            before,
            after,
            combat_changed,
            data_type: key,
        })
    })
    .map_err(|Refusal(message)| message)?;

    // After the commit: an event is a nudge to other clients about work that
    // is done, and must never describe a change that rolled back.
    match outcome.actor_id {
        // The payload `updateActorSystemData` has sent since spec 045, so
        // every listener that re-reads on a sheet change re-reads on a hit.
        Some(actor_id) => {
            let _ = record_world_event(
                conn,
                outcome.world_id,
                EVENT_CODE_ACTOR_SHEET_CHANGED,
                Some(serde_json::json!({
                    "action": "changed",
                    "actorId": actor_id,
                    "dataType": outcome.data_type,
                })),
                user_id,
            );
        }
        // A copy is a token, not a sheet: the token-changed payload
        // `updateToken` sends, which every board re-reads status on.
        None => {
            let _ = record_world_event(
                conn,
                outcome.world_id,
                EVENT_CODE_TOKEN_CHANGED,
                Some(serde_json::json!({
                    "action": "updated",
                    "token_id": outcome.token_id,
                    "scene_id": outcome.scene_id,
                })),
                user_id,
            );
        }
    }
    if let Some(combat_id) = outcome.combat_changed {
        let _ = record_world_event(
            conn,
            outcome.world_id,
            EVENT_CODE_COMBAT_CHANGED,
            Some(serde_json::json!({ "combatId": combat_id })),
            user_id,
        );
    }

    Ok(outcome)
}

#[cfg(test)]
#[path = "hit_points_tests.rs"]
mod tests;
