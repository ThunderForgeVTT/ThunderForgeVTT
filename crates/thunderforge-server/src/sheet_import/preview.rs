//! The plan for bringing a sheet onto an actor, made without writing
//! anything (contracts/graphql-sheet-import.md, `sheetImportPreview`).
//!
//! The browser reads the sheet and sends its reading; the server maps it,
//! resolves content against the world and diffs it against the actor. The
//! same [`plan_for`] runs again when the file is applied, on the server's
//! own reading, so the two plans' hashes can be compared.

use diesel::prelude::*;
use serde_json::Value;
use thunderforge_canvas_core::system_contribution::contribution_for;
use thunderforge_sheet_import::{
    ActorSnapshot, Corrections, ImportPlan, ImportedCharacter, SheetMapping, plan_with,
};
use uuid::Uuid;

use super::error::SheetImportError;
use super::index::WorldIndex;
use super::mapping::mapping_for_system;
use super::snapshot::{self, ActorContext};
use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;

/// How much of a reading the server will look at. A real sheet is a few
/// hundred values; these leave room for a long backstory and spell list.
pub const MAX_DEPTH: usize = 16;
pub const MAX_STRING: usize = 20_000;
pub const MAX_LIST: usize = 2_000;
pub const MAX_NODES: usize = 50_000;

/// Refuse a reading past the bounds before it is deserialised.
pub fn check_bounds(value: &Value) -> Result<(), SheetImportError> {
    fn walk(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if *nodes > MAX_NODES {
            return Err(format!("more than {MAX_NODES} values"));
        }
        if depth > MAX_DEPTH {
            return Err(format!("nested deeper than {MAX_DEPTH}"));
        }
        match value {
            Value::String(text) if text.chars().count() > MAX_STRING => {
                Err(format!("a text longer than {MAX_STRING} characters"))
            }
            Value::Array(items) if items.len() > MAX_LIST => {
                Err(format!("a list longer than {MAX_LIST}"))
            }
            Value::Array(items) => items.iter().try_for_each(|v| walk(v, depth + 1, nodes)),
            Value::Object(map) if map.len() > MAX_LIST => {
                Err(format!("an object with more than {MAX_LIST} keys"))
            }
            Value::Object(map) => map.iter().try_for_each(|(key, v)| {
                if key.len() > MAX_STRING {
                    return Err("a key that is too long".to_string());
                }
                walk(v, depth + 1, nodes)
            }),
            _ => Ok(()),
        }
    }
    walk(value, 0, &mut 0)
        .map_err(|why| SheetImportError::Invalid(format!("the reading has {why}")))
}

/// The browser's reading, strictly: an unknown field is an error.
pub fn parse_reading(value: &Value) -> Result<ImportedCharacter, SheetImportError> {
    check_bounds(value)?;
    serde_json::from_value(value.clone())
        .map_err(|e| SheetImportError::Invalid(format!("the reading does not parse: {e}")))
}

/// Corrections are a map of neutral path to value.
pub fn parse_corrections(value: Option<&Value>) -> Result<Corrections, SheetImportError> {
    match value {
        None | Some(Value::Null) => Ok(Corrections::new()),
        Some(value) => {
            check_bounds(value)?;
            serde_json::from_value(value.clone()).map_err(|e| {
                SheetImportError::Invalid(format!("the corrections do not parse: {e}"))
            })
        }
    }
}

/// The actor's system's declaration, or `SYSTEM_HAS_NO_MAPPING`.
pub fn mapping_for(
    systems_dir: &str,
    actor: &crate::models::WorldActor,
) -> Result<(String, SheetMapping), SheetImportError> {
    let system = actor
        .game_system_id
        .clone()
        .ok_or(SheetImportError::NoMapping)?;
    let mapping = mapping_for_system(systems_dir, &system)
        .map_err(|e| SheetImportError::Invalid(e.to_string()))?
        .ok_or(SheetImportError::NoMapping)?;
    Ok((system, mapping))
}

/// Map, resolve and diff a reading onto an actor, with the pack's refine
/// hook adapted from JSON.
pub fn plan_for(
    conn: &mut PgConnection,
    systems_dir: &str,
    ctx: &ActorContext,
    reading: &ImportedCharacter,
    corrections: &Corrections,
) -> Result<(SheetMapping, ImportPlan), SheetImportError> {
    let (system, mapping) = mapping_for(systems_dir, &ctx.actor)?;
    if !mapping.readers.contains(&reading.reader.id) {
        return Err(SheetImportError::sheet(
            "SHEET_NOT_RECOGNISED",
            "This game system does not take sheets from that reader.",
        ));
    }
    let index = WorldIndex::load(conn, ctx.actor.world_id, &mapping)
        .map_err(|e| SheetImportError::Database(e.to_string()))?;
    let current = ctx.snapshot(&mapping);
    let refine = contribution_for(&system)
        .and_then(|contribution| contribution.sheet_import)
        .and_then(|slot| slot.refine);
    let hook = refine.map(|refine| {
        move |reading: &ImportedCharacter, current: &ActorSnapshot, plan: &mut ImportPlan| {
            let parts = (
                serde_json::to_value(reading),
                serde_json::to_value(current),
                serde_json::to_value(&*plan),
            );
            if let (Ok(reading), Ok(current), Ok(mut value)) = parts {
                refine(&reading, &current, &mut value);
                if let Ok(refined) = serde_json::from_value(value) {
                    *plan = refined;
                }
            }
        }
    });
    let hook_ref: Option<&thunderforge_sheet_import::RefineHook> = hook
        .as_ref()
        .map(|hook| hook as &thunderforge_sheet_import::RefineHook);
    let plan = plan_with(&mapping, hook_ref, reading, corrections, &current, &index);
    Ok((mapping, plan))
}

/// The flag, then Editor or above on the actor (the GM always qualifies).
pub async fn require_may_import(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<(), SheetImportError> {
    let enabled = crate::settings::flag_on(state, crate::settings::features::SHEET_IMPORT)
        .await
        .map_err(SheetImportError::Database)?;
    require_enabled(enabled)?;
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(SheetImportError::from_permission)
}

pub fn require_enabled(enabled: bool) -> Result<(), SheetImportError> {
    if enabled {
        Ok(())
    } else {
        Err(SheetImportError::Disabled)
    }
}

/// `sheetImportPreview`: the plan, and nothing written.
///
/// `systems_dir` is a parameter, as for the book import, so a test can point
/// it at the checked-in packs.
pub async fn sheet_import_preview_impl(
    state: &AppState,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    reading: &Value,
    corrections: Option<&Value>,
) -> Result<ImportPlan, SheetImportError> {
    require_may_import(state, user_id, is_admin, actor_id).await?;
    let reading = parse_reading(reading)?;
    let corrections = parse_corrections(corrections)?;
    let systems_dir = systems_dir.to_string();
    let pool = state.db_pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| SheetImportError::Database(e.to_string()))?;
        let ctx = snapshot::load(&mut conn, actor_id)
            .map_err(|e| SheetImportError::Database(e.to_string()))?;
        plan_for(&mut conn, &systems_dir, &ctx, &reading, &corrections).map(|(_, plan)| plan)
    })
    .await
    .map_err(|e| SheetImportError::Database(e.to_string()))?
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
