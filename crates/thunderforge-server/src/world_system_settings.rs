//! A world's answers to the settings its game system declares (spec 067).
//!
//! # The shape
//!
//! The *system* says what may be chosen: its manifest's `settings` block,
//! read by `pack_system_spec::settings`. The *world* holds what was chosen:
//! rows in `world_system_settings`, keyed by world, system and key. This
//! module joins the two, and it is the only code that touches the table —
//! server code reads a setting through [`effective_value`], and a pack's web
//! code through the `worldSystemSettings` query.
//!
//! # The invariant everything here rests on
//!
//! **A value that is not both declared and allowed does not exist.** A world
//! with no row reads as the declared default. So does a world whose row names
//! a key the manifest no longer declares, and one whose stored value the
//! declaration has since stopped allowing. None of those is an error, and
//! none of those rows is deleted: the read is total, and a pack that drops a
//! setting and restores it finds the world's answer where it was left.
//!
//! This is `settings::registry`'s posture at instance scope (ADR-091), and
//! Roll for Shoes' "a world with no row is not an error" (ADR-108), made the
//! rule for every system at once.

use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use pack_system_spec::settings::{SystemSetting, settings_from_manifest};
use thunderforge_canvas_core::system_contribution::contribution_for;

use crate::schema::{world_system_setting_changes, world_system_settings, worlds};

/// One declared setting and what this world plays by.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveSetting {
    pub declaration: SystemSetting,
    pub value: Value,
    /// Whether `value` is the declared default because nothing usable is
    /// stored — not whether it happens to equal the default.
    pub is_default: bool,
}

/// The settings a system declares, in form order. A system with no manifest,
/// or no `settings` block, declares none.
pub fn declarations_for_system(systems_dir: &str, system_id: &str) -> Vec<SystemSetting> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    settings_from_manifest(&manifest)
}

/// Whether `value` may be stored for this setting: the declaration first,
/// then the pack's own validator if it contributes one (FR-007).
pub fn validate(system_id: &str, declaration: &SystemSetting, value: &Value) -> Result<(), String> {
    declaration.check(value)?;
    if let Some(check) = contribution_for(system_id).and_then(|c| c.world_setting) {
        check(&declaration.id, value)?;
    }
    Ok(())
}

/// The game system a world plays, if it has chosen one.
pub fn system_of_world(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<Option<String>> {
    worlds::table
        .find(world_id)
        .select(worlds::game_system_id)
        .first::<Option<String>>(conn)
}

/// Every declared setting with this world's effective value.
pub fn read_effective(
    conn: &mut PgConnection,
    world_id: Uuid,
    system_id: &str,
    declarations: Vec<SystemSetting>,
) -> QueryResult<Vec<EffectiveSetting>> {
    let stored: Vec<(String, Value)> = world_system_settings::table
        .filter(world_system_settings::world_id.eq(world_id))
        .filter(world_system_settings::system_id.eq(system_id))
        .select((world_system_settings::key, world_system_settings::value))
        .load(conn)?;

    Ok(declarations
        .into_iter()
        .map(|declaration| {
            let usable = stored
                .iter()
                .find(|(key, _)| *key == declaration.id)
                .map(|(_, value)| value)
                .filter(|value| validate(system_id, &declaration, value).is_ok());
            match usable {
                Some(value) => EffectiveSetting {
                    value: value.clone(),
                    is_default: false,
                    declaration,
                },
                None => EffectiveSetting {
                    value: declaration.default.clone(),
                    is_default: true,
                    declaration,
                },
            }
        })
        .collect())
}

/// What this world plays by for one setting of its current system.
///
/// `None` when the world has no system or the system declares no such
/// setting — a caller asking for a key nothing declares has a bug, and a
/// made-up answer would hide it.
pub fn effective_value(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    key: &str,
) -> QueryResult<Option<Value>> {
    let Some(system_id) = system_of_world(conn, world_id)? else {
        return Ok(None);
    };
    let declarations = declarations_for_system(systems_dir, &system_id)
        .into_iter()
        .filter(|declaration| declaration.id == key)
        .collect();
    Ok(read_effective(conn, world_id, &system_id, declarations)?
        .into_iter()
        .next()
        .map(|setting| setting.value))
}

/// Store one already-validated value and record the change.
///
/// One transaction, so a stored value always has the change that put it
/// there. The change's `old_value` is what the row held, or `NULL` when the
/// setting had never been set.
pub fn write(
    conn: &mut PgConnection,
    world_id: Uuid,
    system_id: &str,
    key: &str,
    value: &Value,
    user_id: Uuid,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        let old: Option<Value> = world_system_settings::table
            .find((world_id, system_id, key))
            .select(world_system_settings::value)
            .first(conn)
            .optional()?;

        diesel::insert_into(world_system_settings::table)
            .values((
                world_system_settings::world_id.eq(world_id),
                world_system_settings::system_id.eq(system_id),
                world_system_settings::key.eq(key),
                world_system_settings::value.eq(value),
                world_system_settings::updated_by.eq(user_id),
            ))
            .on_conflict((
                world_system_settings::world_id,
                world_system_settings::system_id,
                world_system_settings::key,
            ))
            .do_update()
            .set((
                world_system_settings::value.eq(value),
                world_system_settings::updated_by.eq(user_id),
                world_system_settings::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;

        diesel::insert_into(world_system_setting_changes::table)
            .values((
                world_system_setting_changes::world_id.eq(world_id),
                world_system_setting_changes::system_id.eq(system_id),
                world_system_setting_changes::key.eq(key),
                world_system_setting_changes::old_value.eq(old),
                world_system_setting_changes::new_value.eq(value),
                world_system_setting_changes::changed_by.eq(user_id),
            ))
            .execute(conn)?;
        Ok(())
    })
}
