//! A system's `appearance` block (spec 044 FR-007a, research R7).
//!
//! The hero builder rolls a face narrowed to a creature's race. Which field of
//! a sheet holds that race is the pack's to say — 5e keeps free text in
//! `traitData.race`; a system with no races declares nothing, and its
//! creatures roll as "any". Shared code reads this declaration and never
//! names a system's field itself.
//!
//! A race read this way narrows a roll only. Nothing writes it back to a
//! sheet (contract B5a).

use crate::combat::{SystemFieldRef, require_field};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `appearance` block.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemAppearance {
    /// Where a creature's race is written. Absent means the system has none.
    #[serde(default)]
    pub race: Option<SystemAppearanceRace>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemAppearanceRace {
    pub source: SystemFieldRef,
}

/// An `appearance` block that is present must point at a field the pack
/// declares, as a `combat` block must (spec 046 M2).
pub fn validate_appearance_content(instance: &serde_json::Value) -> Result<(), String> {
    let Some(block) = instance.get("appearance").filter(|b| !b.is_null()) else {
        return Ok(());
    };
    let appearance: SystemAppearance =
        serde_json::from_value(block.clone()).map_err(|e| format!("appearance: {e}"))?;
    if let Some(race) = &appearance.race {
        require_field(
            instance.get("data_types"),
            "appearance.race.source",
            &race.source.slot,
            &race.source.field,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod tests;
