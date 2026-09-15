//! How many squares a creature fills (spec 046 FR-030, research R10).
//!
//! A pack declares its sizes in `combat.sizes`: where a creature's size is
//! kept (`source`) and how many squares a side each size fills. This resolves
//! a token to that number, for the board (`tokenGrid`) and for measuring an
//! attack (`combat::reach`), so the square an ogre is drawn over and the
//! square its reach is measured from are one answer.
//!
//! - A **linked** token is its actor, so its size is its actor's.
//! - An unlinked **copy** is its NPC in every respect but hit points
//!   (ADR-102), so its size is its NPC's too — unless the pack keeps size
//!   among a creature's resources, which is the one slot a copy holds itself.
//! - A token with **no actor**, a system with **no sizes** (M1), a sheet with
//!   **no size**, or a size the system does not declare: one square.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::manifest::{combat_for_system, slot_key};
use crate::schema::{scenes, tokens, world_actor_system_data, world_actors, worlds};
use pack_system_spec::combat::SystemSizes;

/// A creature of no known size fills one square.
pub const DEFAULT_FOOTPRINT: f32 = 1.0;

/// The footprint a slot's value names, by a system's declared sizes.
pub fn footprint_from(sizes: &SystemSizes, slot: Option<&serde_json::Value>) -> f32 {
    slot.and_then(|slot| slot.get(&sizes.source.field))
        .and_then(|value| value.as_str())
        .and_then(|id| sizes.categories.iter().find(|c| c.id == id))
        .map(|category| category.footprint)
        .filter(|footprint| footprint.is_finite() && *footprint > 0.0)
        .unwrap_or(DEFAULT_FOOTPRINT)
}

type TokenRow = (Uuid, Option<Uuid>, bool, Option<serde_json::Value>);

/// The footprint of every token in a scene, one query for the tokens and one
/// for their actors' sheets however many there are.
pub fn footprints_in_scene(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
) -> QueryResult<HashMap<Uuid, f32>> {
    let rows: Vec<TokenRow> = tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .select((
            tokens::token_id,
            tokens::actor_id,
            tokens::linked,
            tokens::system_data,
        ))
        .load(conn)?;
    resolve_rows(conn, systems_dir, scene_id, rows)
}

/// The footprint of the named tokens, all on one scene.
pub fn footprints_of(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    token_ids: &[Uuid],
) -> QueryResult<HashMap<Uuid, f32>> {
    let rows: Vec<TokenRow> = tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .filter(tokens::token_id.eq_any(token_ids))
        .select((
            tokens::token_id,
            tokens::actor_id,
            tokens::linked,
            tokens::system_data,
        ))
        .load(conn)?;
    resolve_rows(conn, systems_dir, scene_id, rows)
}

fn resolve_rows(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    rows: Vec<TokenRow>,
) -> QueryResult<HashMap<Uuid, f32>> {
    let mut out: HashMap<Uuid, f32> = rows
        .iter()
        .map(|(token_id, ..)| (*token_id, DEFAULT_FOOTPRINT))
        .collect();

    let world_system: Option<String> = scenes::table
        .inner_join(worlds::table.on(worlds::id.eq(scenes::world_id)))
        .filter(scenes::scene_id.eq(scene_id))
        .select(worlds::game_system_id)
        .first::<Option<String>>(conn)
        .optional()?
        .flatten();

    let actor_ids: Vec<Uuid> = rows.iter().filter_map(|row| row.1).collect();
    if actor_ids.is_empty() {
        return Ok(out);
    }
    let actor_systems: HashMap<Uuid, Option<String>> = world_actors::table
        .filter(world_actors::id.eq_any(&actor_ids))
        .select((world_actors::id, world_actors::game_system_id))
        .load::<(Uuid, Option<String>)>(conn)?
        .into_iter()
        .collect();
    let sheets: Vec<crate::models::ActorSystemData> = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
        .select(crate::models::ActorSystemData::as_select())
        .load(conn)?;

    for (token_id, actor_id, linked, system_data) in rows {
        let Some(actor_id) = actor_id else {
            continue;
        };
        let Some(system_id) = actor_systems
            .get(&actor_id)
            .cloned()
            .flatten()
            .or_else(|| world_system.clone())
        else {
            continue;
        };
        let combat = combat_for_system(systems_dir, &system_id);
        let Some(sizes) = combat.sizes.as_ref() else {
            continue;
        };
        let key = slot_key(&sizes.source.slot);
        let footprint = if key == "resource_data" && !linked {
            footprint_from(sizes, system_data.as_ref())
        } else {
            // The sheet in this system, else whichever the actor has.
            let sheet = sheets
                .iter()
                .filter(|row| row.actor_id == actor_id)
                .max_by_key(|row| row.game_system_id == system_id);
            let slot = sheet.and_then(|row| crate::combat::hit_points::slot_column(row, &key));
            footprint_from(sizes, slot.as_ref())
        };
        out.insert(token_id, footprint);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pack_system_spec::combat::{SystemFieldRef, SystemSizeCategory};
    use serde_json::json;

    fn sizes() -> SystemSizes {
        SystemSizes {
            source: SystemFieldRef {
                slot: "traitData".into(),
                field: "size".into(),
            },
            categories: vec![
                SystemSizeCategory {
                    id: "tiny".into(),
                    label: "Tiny".into(),
                    footprint: 0.5,
                },
                SystemSizeCategory {
                    id: "large".into(),
                    label: "Large".into(),
                    footprint: 2.0,
                },
            ],
        }
    }

    #[test]
    fn a_declared_size_is_its_footprint() {
        assert_eq!(
            footprint_from(&sizes(), Some(&json!({ "size": "large" }))),
            2.0
        );
        assert_eq!(
            footprint_from(&sizes(), Some(&json!({ "size": "tiny" }))),
            0.5
        );
    }

    #[test]
    fn anything_else_fills_one_square() {
        for slot in [
            None,
            Some(json!({})),
            Some(json!({ "size": "colossal" })),
            Some(json!({ "size": 2 })),
            Some(json!({ "other": "large" })),
        ] {
            assert_eq!(
                footprint_from(&sizes(), slot.as_ref()),
                DEFAULT_FOOTPRINT,
                "{slot:?}"
            );
        }
    }
}
