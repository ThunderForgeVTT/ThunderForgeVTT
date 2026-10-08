//! Spec 084 (research R8, R9): who may reroll a roll, by what, and until when.
//!
//! The rules here are the host's: the maker's own d20 test, made from a
//! sheet they can still act for, not yet replaced, inside the window, with
//! a spend the chain has not used. Whether the sheet can pay is the pack's
//! `reroll`, asked through [`offers_for`] with its plan thrown away, so the
//! button a roller is offered and the refusal they would get cannot
//! disagree.

use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use serde_json::{Map, Value};
use thunderforge_canvas_core::roll_facets::{RerollInput, RollFacets, RollKind};
use uuid::Uuid;

use crate::auth::world_membership::actor_in_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::models::RollRecord;
use crate::rolls::facets::{facet_labels, kind_of};
use crate::rolls::visibility::Viewer;
use crate::schema::{
    world_actor_permissions, world_actor_system_data, world_actors, world_attacks,
    world_roll_records,
};
use crate::world_system_settings;

/// How long after a roll its maker may still reroll it.
pub fn reroll_window() -> Duration {
    Duration::minutes(2)
}

pub const ONLY_MAKER: &str = "Only the person who made a roll may reroll it.";
pub const NO_LONGER_ACTS: &str = "You can no longer act for this character.";
pub const NOT_A_D20_TEST: &str = "Only a d20 test can be rerolled.";
pub const ALREADY_REROLLED: &str = "This roll has already been rerolled.";
pub const TOO_LATE: &str = "It is too late to reroll this roll.";
pub const A_HIT: &str = "A hit cannot be rerolled.";

/// Every roll kind a spend can reroll.
const D20_TESTS: [RollKind; 2] = [RollKind::Check, RollKind::ToHit];

/// What the host knows of a roll when it is asked to reroll it.
pub struct Standing<'a> {
    pub row: &'a RollRecord,
    /// The roll that already replaced it, if one has.
    pub rerolled_by: Option<Uuid>,
    /// Whether the asker still has Editor on the roll's actor.
    pub may_act: bool,
    /// Every spend used anywhere in its chain.
    pub spent: &'a [String],
    /// For a to-hit: whether the attack it was rolled for hit.
    pub hit: bool,
}

/// The sheet a spend is paid from, as the pack is shown it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub actor_name: String,
    /// Every slot of the actor's system data, by slot name.
    pub slots: Value,
    /// The world's effective system settings, by id.
    pub settings: Value,
}

/// When a roll stops being rerollable, or `None` for a roll that never was:
/// one with no sheet, not a d20 test, or already replaced.
pub fn reroll_until(row: &RollRecord, rerolled_by: Option<Uuid>) -> Option<DateTime<Utc>> {
    let kind = row.roll_kind.as_deref().and_then(kind_of)?;
    if row.actor_id.is_none() || !D20_TESTS.contains(&kind) || rerolled_by.is_some() {
        return None;
    }
    Some(row.created_at + reroll_window())
}

/// The host's refusals, in the contract's order (3 to 9), for `caller`
/// spending `spend` (named `label`) at `now`. The roll's kind when none
/// applies; the pack decides the rest.
pub fn may_reroll(
    caller: Uuid,
    standing: &Standing<'_>,
    spend: &str,
    label: &str,
    now: DateTime<Utc>,
) -> Result<RollKind, String> {
    let row = standing.row;
    if row.triggered_by != caller || row.actor_id.is_none() {
        return Err(ONLY_MAKER.to_string());
    }
    if !standing.may_act {
        return Err(NO_LONGER_ACTS.to_string());
    }
    let kind = row
        .roll_kind
        .as_deref()
        .and_then(kind_of)
        .filter(|kind| D20_TESTS.contains(kind))
        .ok_or_else(|| NOT_A_D20_TEST.to_string())?;
    if standing.rerolled_by.is_some() {
        return Err(ALREADY_REROLLED.to_string());
    }
    if now > row.created_at + reroll_window() {
        return Err(TOO_LATE.to_string());
    }
    if standing.spent.iter().any(|used| used == spend) {
        return Err(format!("{label} has already been spent on this roll."));
    }
    if kind == RollKind::ToHit && standing.hit {
        return Err(A_HIT.to_string());
    }
    Ok(kind)
}

/// What the pack is shown to plan `spend` on this roll.
pub fn reroll_input<'a>(
    spend: &'a str,
    kind: RollKind,
    row: &'a RollRecord,
    facets: &'a [String],
    sheet: &'a Sheet,
) -> RerollInput<'a> {
    RerollInput {
        spend,
        kind,
        formula: &row.formula,
        facets,
        actor_name: &sheet.actor_name,
        sheet: &sheet.slots,
        settings: &sheet.settings,
    }
}

/// The spends `viewer` could reroll this roll with now, as `(id, label)`.
/// Empty for anyone but its maker, and for a system with no spends.
pub fn offers_for(
    viewer: Uuid,
    standing: &Standing<'_>,
    system: Option<&RollFacets>,
    sheet: &Sheet,
    now: DateTime<Utc>,
) -> Vec<(String, String)> {
    let Some(system) = system else {
        return Vec::new();
    };
    let facets = standing.row.facet_ids();
    system
        .spends
        .iter()
        .filter(|spend| {
            may_reroll(viewer, standing, spend.id, spend.label, now).is_ok_and(|kind| {
                let input = reroll_input(spend.id, kind, standing.row, &facets, sheet);
                (system.reroll)(&input).is_ok()
            })
        })
        .map(|spend| (spend.id.to_string(), spend.label.to_string()))
        .collect()
}

/// The name `spend` is shown by in `system_id`, or the id itself.
pub fn spend_label(system_id: &str, spend: &str) -> String {
    facet_labels(system_id, &[spend.to_string()])
        .pop()
        .map_or_else(|| spend.to_string(), |(_, label)| label)
}

// ---------------------------------------------------------------------------
// What the rules need from the database
// ---------------------------------------------------------------------------

/// `row`, then each roll it replaced, back to the first. A chain is at most
/// three rolls long; the bound only guards against a corrupt loop.
pub fn chain_of(conn: &mut PgConnection, row: &RollRecord) -> QueryResult<Vec<RollRecord>> {
    let mut chain = vec![row.clone()];
    while let Some(parent) = chain.last().and_then(|roll| roll.reroll_of) {
        if chain.len() >= 16 || chain.iter().any(|roll| roll.id == parent) {
            break;
        }
        let Some(older) = world_roll_records::table
            .find(parent)
            .select(RollRecord::as_select())
            .first::<RollRecord>(conn)
            .optional()?
        else {
            break;
        };
        chain.push(older);
    }
    Ok(chain)
}

/// Every roll in `row`'s chain, older and newer, oldest first: a roll and
/// what replaced it are one roll to the table.
pub fn whole_chain(conn: &mut PgConnection, row: &RollRecord) -> QueryResult<Vec<RollRecord>> {
    let mut chain = chain_of(conn, row)?;
    chain.reverse();
    while let Some(newer) = rerolled_by(conn, chain[chain.len() - 1].id)? {
        if chain.len() >= 32 || chain.iter().any(|roll| roll.id == newer) {
            break;
        }
        let Some(newer) = world_roll_records::table
            .find(newer)
            .select(RollRecord::as_select())
            .first::<RollRecord>(conn)
            .optional()?
        else {
            break;
        };
        chain.push(newer);
    }
    Ok(chain)
}

/// Every spend used in a chain.
pub fn spent_in(chain: &[RollRecord]) -> Vec<String> {
    chain
        .iter()
        .filter_map(|roll| roll.reroll_spent.clone())
        .collect()
}

/// The roll that replaced `roll_id`, if one has.
pub fn rerolled_by(conn: &mut PgConnection, roll_id: Uuid) -> QueryResult<Option<Uuid>> {
    world_roll_records::table
        .filter(world_roll_records::reroll_of.eq(roll_id))
        .select(world_roll_records::id)
        .first::<Uuid>(conn)
        .optional()
}

/// Whether the attack a to-hit was rolled for hit.
pub fn attack_hit(conn: &mut PgConnection, roll_id: Uuid) -> QueryResult<bool> {
    Ok(world_attacks::table
        .filter(world_attacks::to_hit_roll_id.eq(roll_id))
        .select(world_attacks::outcome)
        .first::<String>(conn)
        .optional()?
        .is_some_and(|outcome| outcome == "hit"))
}

/// Whether `user_id` has Editor on `actor_id`, as
/// `require_actor_permission` decides it: the world's GM always, anyone
/// else by their grant.
pub fn may_act(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    actor_id: Uuid,
) -> QueryResult<bool> {
    if actor_in_world(conn, user_id, is_admin, world_id).runs_the_world() {
        return Ok(true);
    }
    let level = world_actor_permissions::table
        .filter(world_actor_permissions::actor_id.eq(actor_id))
        .filter(world_actor_permissions::user_id.eq(user_id))
        .select(world_actor_permissions::level)
        .first::<String>(conn)
        .optional()?;
    Ok(level
        .and_then(|level| ActorPermissionLevel::from_db_str(&level))
        .is_some_and(|level| level.rank() >= ActorPermissionLevel::Editor.rank()))
}

/// The actor's name, slots and the world's settings, for the pack.
pub fn load_sheet(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    system_id: &str,
    actor_id: Uuid,
) -> QueryResult<Sheet> {
    let actor_name = world_actors::table
        .find(actor_id)
        .select(world_actors::label)
        .first::<String>(conn)
        .optional()?
        .unwrap_or_default();
    type Slots = (
        Option<Value>,
        Option<Value>,
        Option<Value>,
        Option<Value>,
        Option<Value>,
    );
    let slots = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select((
            world_actor_system_data::ability_data,
            world_actor_system_data::resource_data,
            world_actor_system_data::proficiency_data,
            world_actor_system_data::trait_data,
            world_actor_system_data::spell_data,
        ))
        .first::<Slots>(conn)
        .optional()?
        .unwrap_or_default();
    let (ability, resource, proficiency, traits, spell) = slots;
    let mut by_name = Map::new();
    for (name, value) in [
        ("ability_data", ability),
        ("resource_data", resource),
        ("proficiency_data", proficiency),
        ("trait_data", traits),
        ("spell_data", spell),
    ] {
        by_name.insert(name.to_string(), value.unwrap_or(Value::Null));
    }
    let declarations = world_system_settings::declarations_for_system(systems_dir, system_id);
    let settings = world_system_settings::read_effective(conn, world_id, system_id, declarations)?
        .into_iter()
        .map(|setting| (setting.declaration.id, setting.value))
        .collect::<Map<_, _>>();
    Ok(Sheet {
        actor_name,
        slots: Value::Object(by_name),
        settings: Value::Object(settings),
    })
}

/// `offers_for` one row as `viewer` sees it, read from the database. Only
/// the viewer's own rolls still inside their window cost a query.
pub fn offers_from_db(
    conn: &mut PgConnection,
    systems_dir: &str,
    system_id: &str,
    viewer: Viewer,
    row: &RollRecord,
    rerolled_by: Option<Uuid>,
    now: DateTime<Utc>,
) -> QueryResult<Vec<(String, String)>> {
    let system = crate::rolls::facets::facets_for(system_id);
    let open = reroll_until(row, rerolled_by).is_some_and(|until| now <= until);
    let (Some(actor_id), true, Some(_)) = (row.actor_id, open, system) else {
        return Ok(Vec::new());
    };
    // An attack is offered nothing until `rerollRoll` re-judges attacks
    // (tasks.md T057); until then the mutation refuses one.
    if row.triggered_by != viewer.user_id || row.roll_kind.as_deref() == Some("to_hit") {
        return Ok(Vec::new());
    }
    let chain = chain_of(conn, row)?;
    let spent = spent_in(&chain);
    let hit = row.roll_kind.as_deref() == Some("to_hit") && attack_hit(conn, row.id)?;
    let standing = Standing {
        row,
        rerolled_by,
        may_act: may_act(
            conn,
            viewer.user_id,
            viewer.is_admin,
            row.world_id,
            actor_id,
        )?,
        spent: &spent,
        hit,
    };
    let sheet = load_sheet(conn, systems_dir, row.world_id, system_id, actor_id)?;
    Ok(offers_for(viewer.user_id, &standing, system, &sheet, now))
}

#[cfg(test)]
#[path = "reroll_tests.rs"]
mod tests;
