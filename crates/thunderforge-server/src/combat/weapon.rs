//! What an attack is made with (spec 046 research R2): the ability or item,
//! whether the caller may use it, and the parts it makes.
//!
//! Beside `attack.rs`, which decides what an attack does; this decides what
//! it is. A multiattack (FR-044) is one weapon of several parts.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::attack::{ActionCost, FightRefusal};
use crate::combat::reach::Reach;
use crate::models::{WorldAbility, WorldItem};
use crate::schema::{
    world_abilities, world_ability_effects, world_actor_abilities, world_actor_inventory,
    world_item_abilities, world_item_effects, world_items,
};

/// One attack an ability or item makes: its name and its formulas.
#[derive(Clone, Debug)]
pub(crate) struct Part {
    pub(crate) ability_id: Option<Uuid>,
    pub(crate) item_id: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) to_hit: String,
    pub(crate) damage: Vec<String>,
    pub(crate) reach: Reach,
}

/// What the attack is made with, once found.
pub(crate) enum Weapon {
    Ability(WorldAbility),
    Item(WorldItem),
}

impl Weapon {
    pub(crate) fn action_cost(&self) -> ActionCost {
        ActionCost::from_db_str(match self {
            Weapon::Ability(a) => &a.action_cost,
            Weapon::Item(i) => &i.action_cost,
        })
    }

    /// What a legendary action with it costs from the pool.
    pub(crate) fn legendary_cost(&self) -> i32 {
        match self {
            Weapon::Ability(a) => a.legendary_cost,
            Weapon::Item(i) => i.legendary_cost,
        }
    }

    fn multiattack(&self) -> Vec<Uuid> {
        match self {
            Weapon::Ability(a) => &a.multiattack,
            Weapon::Item(i) => &i.multiattack,
        }
        .iter()
        .flatten()
        .copied()
        .collect()
    }
}

fn effects_of_ability(
    conn: &mut PgConnection,
    ability_id: Uuid,
) -> QueryResult<Vec<(String, String)>> {
    world_ability_effects::table
        .filter(world_ability_effects::ability_id.eq(ability_id))
        .order((
            world_ability_effects::sort_order,
            world_ability_effects::created_at,
        ))
        .select((
            world_ability_effects::effect_type,
            world_ability_effects::formula,
        ))
        .load(conn)
}

fn effects_of_item(conn: &mut PgConnection, item_id: Uuid) -> QueryResult<Vec<(String, String)>> {
    world_item_effects::table
        .filter(world_item_effects::item_id.eq(item_id))
        .order((
            world_item_effects::sort_order,
            world_item_effects::created_at,
        ))
        .select((world_item_effects::effect_type, world_item_effects::formula))
        .load(conn)
}

/// What an ability says about its reach, range and line of sight.
fn reach_of_ability(a: &WorldAbility) -> Reach {
    Reach {
        reach: a.reach,
        range_normal: a.range_normal,
        range_long: a.range_long,
        needs_line_of_sight: a.needs_line_of_sight,
    }
}

fn reach_of_item(i: &WorldItem) -> Reach {
    Reach {
        reach: i.reach,
        range_normal: i.range_normal,
        range_long: i.range_long,
        needs_line_of_sight: i.needs_line_of_sight,
    }
}

fn part_from(
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
    name: String,
    reach: Reach,
    effects: Vec<(String, String)>,
) -> Result<Part, FightRefusal> {
    let to_hit = effects
        .iter()
        .find(|(kind, formula)| kind == "attack_roll" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .ok_or_else(|| FightRefusal::Invalid(format!("{name} has no attack roll to make")))?;
    let damage = effects
        .iter()
        .filter(|(kind, formula)| kind == "damage" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .collect();
    Ok(Part {
        ability_id,
        item_id,
        name,
        to_hit,
        damage,
        reach,
    })
}

/// Find what the attack is made with, and whether this caller may use it.
///
/// A Game Master may use anything in the world. A player uses what the
/// attacker has: an ability its actor knows, an item in its actor's
/// inventory, or an ability one of those items carries. A Game-Master-only
/// ability is not there for a player at all (spec 025 FR-024b).
pub(crate) fn find_weapon(
    conn: &mut PgConnection,
    world_id: Uuid,
    actor_id: Option<Uuid>,
    runs_the_world: bool,
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
) -> Result<Weapon, FightRefusal> {
    let not_known = || FightRefusal::NotFound("That creature has no such attack".to_string());
    match (ability_id, item_id) {
        (Some(ability_id), None) => {
            let ability = world_abilities::table
                .filter(world_abilities::id.eq(ability_id))
                .filter(world_abilities::world_id.eq(world_id))
                .select(WorldAbility::as_select())
                .first::<WorldAbility>(conn)
                .optional()?
                .ok_or_else(not_known)?;
            if !runs_the_world {
                if ability.gm_only {
                    return Err(not_known());
                }
                let Some(actor_id) = actor_id else {
                    return Err(not_known());
                };
                let known = diesel::select(diesel::dsl::exists(
                    world_actor_abilities::table
                        .filter(world_actor_abilities::actor_id.eq(actor_id))
                        .filter(world_actor_abilities::ability_id.eq(ability_id)),
                ))
                .get_result::<bool>(conn)?;
                let carried = known
                    || diesel::select(diesel::dsl::exists(
                        world_item_abilities::table
                            .inner_join(
                                world_actor_inventory::table.on(world_actor_inventory::item_id
                                    .eq(world_item_abilities::item_id.nullable())),
                            )
                            .filter(world_actor_inventory::actor_id.eq(actor_id))
                            .filter(world_item_abilities::ability_id.eq(ability_id)),
                    ))
                    .get_result::<bool>(conn)?;
                if !carried {
                    return Err(not_known());
                }
            }
            Ok(Weapon::Ability(ability))
        }
        (None, Some(item_id)) => {
            let item = world_items::table
                .filter(world_items::id.eq(item_id))
                .filter(world_items::world_id.eq(world_id))
                .select(WorldItem::as_select())
                .first::<WorldItem>(conn)
                .optional()?
                .ok_or_else(not_known)?;
            if !runs_the_world {
                let Some(actor_id) = actor_id else {
                    return Err(not_known());
                };
                let held = diesel::select(diesel::dsl::exists(
                    world_actor_inventory::table
                        .filter(world_actor_inventory::actor_id.eq(actor_id))
                        .filter(world_actor_inventory::item_id.eq(item_id)),
                ))
                .get_result::<bool>(conn)?;
                if !held {
                    return Err(not_known());
                }
            }
            Ok(Weapon::Item(item))
        }
        _ => Err(FightRefusal::Invalid(
            "Choose one ability or one item to attack with".to_string(),
        )),
    }
}

/// The parts one use of `weapon` makes (FR-044).
pub(crate) fn parts_of(
    conn: &mut PgConnection,
    world_id: Uuid,
    weapon: &Weapon,
) -> Result<Vec<Part>, FightRefusal> {
    let named = weapon.multiattack();
    if named.is_empty() {
        return Ok(vec![match weapon {
            Weapon::Ability(a) => part_from(
                Some(a.id),
                None,
                a.name.clone(),
                reach_of_ability(a),
                effects_of_ability(conn, a.id)?,
            )?,
            Weapon::Item(i) => part_from(
                None,
                Some(i.id),
                i.name.clone(),
                reach_of_item(i),
                effects_of_item(conn, i.id)?,
            )?,
        }]);
    }
    let abilities: HashMap<Uuid, WorldAbility> = world_abilities::table
        .filter(world_abilities::id.eq_any(&named))
        .filter(world_abilities::world_id.eq(world_id))
        .select(WorldAbility::as_select())
        .load::<WorldAbility>(conn)?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    named
        .iter()
        .map(|id| {
            let ability = abilities.get(id).ok_or_else(|| {
                FightRefusal::NotFound("One of that multiattack's attacks is gone".to_string())
            })?;
            part_from(
                Some(ability.id),
                None,
                ability.name.clone(),
                reach_of_ability(ability),
                effects_of_ability(conn, ability.id)?,
            )
        })
        .collect()
}
