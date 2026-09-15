//! What an ability or item is as an attack, carried when it is copied (spec
//! 046 data-model.md "Carried through collection copies (spec 026) and exports
//! like every other ability field").
//!
//! Phase 6 added `reach`, `range_normal`, `range_long`, `needs_line_of_sight`,
//! `action_cost`, `legendary_cost` and `multiattack` to `world_abilities` and
//! `world_items`, but every copy path inserts a new row from a named list of
//! fields, and none of those lists knew the new ones: a longsword with a
//! five-foot reach arrived in a collection's destination world with none, and
//! a teleport that ignores walls arrived needing line of sight. Found by the
//! test Phase 7 was asked to write. One place now says what the attack fields
//! are, and every copy path writes them through it.
//!
//! A multiattack names abilities by id, in the source world. A copy that
//! brings those abilities along points at their copies; one that does not
//! (a single shared ability) cannot point at anything in the destination
//! world, and keeps no parts rather than ids that name nothing there.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::models::{WorldAbility, WorldItem};
use crate::schema::{world_abilities, world_items};

#[derive(Clone, Debug, PartialEq)]
pub struct AttackFields {
    pub reach: Option<f64>,
    pub range_normal: Option<f64>,
    pub range_long: Option<f64>,
    pub needs_line_of_sight: bool,
    pub action_cost: String,
    pub legendary_cost: i32,
    pub multiattack: Vec<Uuid>,
}

impl From<&WorldAbility> for AttackFields {
    fn from(a: &WorldAbility) -> Self {
        AttackFields {
            reach: a.reach,
            range_normal: a.range_normal,
            range_long: a.range_long,
            needs_line_of_sight: a.needs_line_of_sight,
            action_cost: a.action_cost.clone(),
            legendary_cost: a.legendary_cost,
            multiattack: a.multiattack.iter().flatten().copied().collect(),
        }
    }
}

impl From<&WorldItem> for AttackFields {
    fn from(i: &WorldItem) -> Self {
        AttackFields {
            reach: i.reach,
            range_normal: i.range_normal,
            range_long: i.range_long,
            needs_line_of_sight: i.needs_line_of_sight,
            action_cost: i.action_cost.clone(),
            legendary_cost: i.legendary_cost,
            multiattack: i.multiattack.iter().flatten().copied().collect(),
        }
    }
}

impl AttackFields {
    /// The same attack in a world where `copies` maps source abilities to
    /// their copies; a part with no copy there is dropped.
    pub fn in_destination(mut self, copies: &HashMap<Uuid, Uuid>) -> Self {
        self.multiattack = self
            .multiattack
            .iter()
            .filter_map(|id| copies.get(id).copied())
            .collect();
        self
    }

    fn multiattack_column(&self) -> Vec<Option<Uuid>> {
        self.multiattack.iter().copied().map(Some).collect()
    }

    pub fn write_to_ability(&self, conn: &mut PgConnection, ability_id: Uuid) -> QueryResult<()> {
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set((
                world_abilities::reach.eq(self.reach),
                world_abilities::range_normal.eq(self.range_normal),
                world_abilities::range_long.eq(self.range_long),
                world_abilities::needs_line_of_sight.eq(self.needs_line_of_sight),
                world_abilities::action_cost.eq(&self.action_cost),
                world_abilities::legendary_cost.eq(self.legendary_cost),
                world_abilities::multiattack.eq(self.multiattack_column()),
            ))
            .execute(conn)
            .map(|_| ())
    }

    pub fn write_to_item(&self, conn: &mut PgConnection, item_id: Uuid) -> QueryResult<()> {
        diesel::update(world_items::table.filter(world_items::id.eq(item_id)))
            .set((
                world_items::reach.eq(self.reach),
                world_items::range_normal.eq(self.range_normal),
                world_items::range_long.eq(self.range_long),
                world_items::needs_line_of_sight.eq(self.needs_line_of_sight),
                world_items::action_cost.eq(&self.action_cost),
                world_items::legendary_cost.eq(self.legendary_cost),
                world_items::multiattack.eq(self.multiattack_column()),
            ))
            .execute(conn)
            .map(|_| ())
    }
}

/// After a copy: give every copied ability and item its source's attack
/// fields, with multiattack parts pointing at the copies made alongside.
pub fn carry_copied_attack_fields(
    conn: &mut PgConnection,
    abilities: &HashMap<Uuid, Uuid>,
    items: &HashMap<Uuid, Uuid>,
) -> QueryResult<()> {
    if !abilities.is_empty() {
        let sources: Vec<Uuid> = abilities.keys().copied().collect();
        let rows = world_abilities::table
            .filter(world_abilities::id.eq_any(&sources))
            .select(WorldAbility::as_select())
            .load::<WorldAbility>(conn)?;
        for source in &rows {
            AttackFields::from(source)
                .in_destination(abilities)
                .write_to_ability(conn, abilities[&source.id])?;
        }
    }
    if !items.is_empty() {
        let sources: Vec<Uuid> = items.keys().copied().collect();
        let rows = world_items::table
            .filter(world_items::id.eq_any(&sources))
            .select(WorldItem::as_select())
            .load::<WorldItem>(conn)?;
        for source in &rows {
            AttackFields::from(source)
                .in_destination(abilities)
                .write_to_item(conn, items[&source.id])?;
        }
    }
    Ok(())
}
