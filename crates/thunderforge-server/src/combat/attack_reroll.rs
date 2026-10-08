//! Spec 084 US4 (research R7): a missed attack, rerolled.
//!
//! `rerollRoll` on a `to_hit` roll records the new to-hit as any reroll does,
//! then comes here, on the same transaction. The reroll is the same attack:
//! it is judged against the defence stored with the first one (whatever the
//! target's armour is now), it copies the first row's target, labels,
//! distance, flags and cost into a new row that points back at it, and it
//! spends nothing from the action budget. A hit rolls its damage and offers
//! or applies it through `combat::attack_hit`, exactly as a first-time hit.

use diesel::PgConnection;
use diesel::prelude::*;
use rand::Rng;
use thunderforge_combat::attack::judge;
use thunderforge_dice::PlaceholderBindings;
use uuid::Uuid;

use crate::combat::attack::FightRefusal;
use crate::combat::attack_hit::{HitContext, roll_hit_damage, settle_hit};
use crate::combat::records::*;
use crate::combat::turn::running_combat;
use crate::combat::weapon::{Part, find_weapon, parts_of};
use crate::models::RollRecord;
use crate::rolls::facets::RollMeta;
use crate::rolls::reroll::{A_HIT, NOT_A_D20_TEST};
use crate::schema::{world_attacks, worlds};
use crate::world_events::{EVENT_CODE_ATTACK_MADE, record_world_event};
use thunderforge_canvas_core::roll_facets::RollKind;

/// What a rerolled attack wrote.
#[derive(Clone, Debug)]
pub struct RerolledAttack {
    pub attack_id: Uuid,
    pub outcome: String,
    pub offer_id: Option<Uuid>,
}

/// Re-judge the attack `old_roll_id` was the to-hit of, with `new_roll` as
/// its to-hit. Refused for a hit, and for a to-hit no attack was made with.
pub(crate) fn reroll_attack<R: Rng>(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    old_roll_id: Uuid,
    new_roll: &RollRecord,
    rng: &mut R,
) -> Result<RerolledAttack, FightRefusal> {
    let first = world_attacks::table
        .filter(world_attacks::to_hit_roll_id.eq(old_roll_id))
        .select(AttackRecord::as_select())
        .for_update()
        .first::<AttackRecord>(conn)
        .optional()?
        .ok_or_else(|| FightRefusal::Invalid(NOT_A_D20_TEST.to_string()))?;
    if first.outcome == OUTCOME_HIT {
        return Err(FightRefusal::Invalid(A_HIT.to_string()));
    }
    let world_id = first.world_id;
    let target = first.target_token_id;
    let outcome = judge(target.is_some(), first.defence, new_roll.result_value);

    let bindings: PlaceholderBindings = new_roll
        .bindings
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let part = if outcome == OUTCOME_HIT {
        Some(part_of(conn, &first)?)
    } else {
        None
    };
    let damage = match &part {
        Some(part) => roll_hit_damage(
            conn,
            world_id,
            user_id,
            part,
            &bindings,
            RollMeta {
                actor_id: new_roll.actor_id,
                roll_kind: Some(RollKind::Damage),
                ..RollMeta::default()
            },
            rng,
        )?,
        None => None,
    };

    let now = chrono::Utc::now().naive_utc();
    let attack_id = Uuid::now_v7();
    diesel::insert_into(world_attacks::table)
        .values(&AttackRecord {
            id: attack_id,
            to_hit_roll_id: Some(new_roll.id),
            damage_roll_id: damage.as_ref().map(|(id, _, _)| *id),
            outcome: outcome.to_string(),
            created_by: user_id,
            updated_by: user_id,
            created_at: now,
            updated_at: now,
            reroll_of: Some(first.id),
            ..first.clone()
        })
        .execute(conn)?;
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_ATTACK_MADE,
        Some(serde_json::json!({ "attackId": attack_id })),
        user_id,
    );

    let mut rerolled = RerolledAttack {
        attack_id,
        outcome: outcome.to_string(),
        offer_id: None,
    };
    let (Some(target), Some(part), Some((_, _, amount))) = (target, part, damage) else {
        return Ok(rerolled);
    };
    let (world_system, world_auto_apply) = worlds::table
        .filter(worlds::id.eq(world_id))
        .select((worlds::game_system_id, worlds::auto_apply_npc_damage))
        .first::<(Option<String>, bool)>(conn)?;
    let auto_apply = running_combat(conn, world_id, first.scene_id)?
        .and_then(|(_, override_)| override_)
        .unwrap_or(world_auto_apply);
    let flags: Vec<String> = first.flags.iter().flatten().cloned().collect();
    rerolled.offer_id = settle_hit(
        conn,
        systems_dir,
        user_id,
        &HitContext {
            world_id,
            scene_id: first.scene_id,
            world_system: world_system.as_deref(),
            auto_apply,
            now,
        },
        &part,
        target,
        attack_id,
        amount,
        &flags,
    )?;
    Ok(rerolled)
}

/// The part the first attack was made with, as it is declared now. The
/// attack was allowed when it was made, so whoever runs the world's view of
/// the weapon is the one rebuilt: a multiattack's part need not be on the
/// attacker's own sheet.
fn part_of(conn: &mut PgConnection, first: &AttackRecord) -> Result<Part, FightRefusal> {
    let weapon = find_weapon(
        conn,
        first.world_id,
        None,
        true,
        first
            .item_id
            .is_none()
            .then_some(first.ability_id)
            .flatten(),
        first.item_id,
    )?;
    let mut parts = parts_of(conn, first.world_id, &weapon)?;
    if parts.is_empty() {
        return Err(FightRefusal::NotFound(
            "That creature has no such attack".to_string(),
        ));
    }
    let at = parts
        .iter()
        .position(|part| part.name == first.ability_name)
        .unwrap_or(0);
    Ok(parts.swap_remove(at))
}

#[cfg(test)]
#[path = "attack_reroll_tests.rs"]
mod tests;
