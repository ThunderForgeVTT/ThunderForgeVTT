//! A piece's attack, written onto the world's row when it is adopted (spec
//! 048 T064).
//!
//! The 5e hook puts an attack row on the piece as spec 046 holds an attack:
//! `fields.attack.effects` (the printed to-hit as `ATTACK_ROLL`, the damage
//! as `DAMAGE`) and its reach and ranges. The world's item or ability is
//! what the attack flow reads, so without these an adopted longsword has
//! nothing to roll.

use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::schema::{world_abilities, world_ability_effects, world_item_effects, world_items};

/// The effect kinds the attack flow and the compendium know, as stored.
fn effect_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "attack_roll" => Some("attack_roll"),
        "damage" => Some("damage"),
        "heal" => Some("heal"),
        "modifier" => Some("modifier"),
        _ => None,
    }
}

fn distance(attack: &Value, key: &str) -> Option<f64> {
    attack
        .get(key)
        .and_then(Value::as_f64)
        .filter(|feet| feet.is_finite() && *feet >= 0.0)
}

/// The (kind, formula) pairs the piece names, in order, skipping any the
/// world could not roll.
fn effects(attack: &Value) -> Vec<(&'static str, String)> {
    attack
        .get("effects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|effect| {
            let kind = effect_type(effect.get("effect_type")?.as_str()?)?;
            let formula = effect.get("formula")?.as_str()?.trim();
            (!formula.is_empty() && formula.len() <= 200).then(|| (kind, formula.to_string()))
        })
        .take(16)
        .collect()
}

/// Write `fields.attack`, when the piece has one, onto the world's row.
pub fn write(
    conn: &mut PgConnection,
    fields: &Value,
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
) -> QueryResult<()> {
    let Some(attack) = fields.get("attack").filter(|a| a.is_object()) else {
        return Ok(());
    };
    let (reach, normal, long) = (
        distance(attack, "reach"),
        distance(attack, "range_normal"),
        distance(attack, "range_long"),
    );
    let effects = effects(attack);
    if let Some(id) = item_id {
        diesel::update(world_items::table.find(id))
            .set((
                world_items::reach.eq(reach),
                world_items::range_normal.eq(normal),
                world_items::range_long.eq(long),
            ))
            .execute(conn)?;
        for (order, (kind, formula)) in effects.into_iter().enumerate() {
            diesel::insert_into(world_item_effects::table)
                .values((
                    world_item_effects::id.eq(Uuid::now_v7()),
                    world_item_effects::item_id.eq(id),
                    world_item_effects::effect_type.eq(kind),
                    world_item_effects::formula.eq(formula),
                    world_item_effects::target.eq("one creature"),
                    world_item_effects::sort_order.eq(order as i32),
                ))
                .execute(conn)?;
        }
    } else if let Some(id) = ability_id {
        diesel::update(world_abilities::table.find(id))
            .set((
                world_abilities::reach.eq(reach),
                world_abilities::range_normal.eq(normal),
                world_abilities::range_long.eq(long),
            ))
            .execute(conn)?;
        for (order, (kind, formula)) in effects.into_iter().enumerate() {
            diesel::insert_into(world_ability_effects::table)
                .values((
                    world_ability_effects::id.eq(Uuid::now_v7()),
                    world_ability_effects::ability_id.eq(id),
                    world_ability_effects::effect_type.eq(kind),
                    world_ability_effects::formula.eq(formula),
                    world_ability_effects::target.eq("one creature"),
                    world_ability_effects::sort_order.eq(order as i32),
                ))
                .execute(conn)?;
        }
    }
    Ok(())
}
