//! A system's `combat` block and its turn budget (spec 046), as the manifest
//! declares them.
//!
//! The platform holds the concepts — hit points, a defence, a size, a
//! legendary pool, what a turn affords — and the pack says which of its own
//! fields they are. Shared code reads these declarations and never names a
//! system's fields itself (contract M5, `check-system-registry.mjs`).
//!
//! Every block is optional (M1). A pack that declares no `hitPoints` has no
//! damage operation; that is a correct answer for a ruleset without hit
//! points, not a gap to fill with a default.
//!
//! The shapes moved here from `pack_system_spec::combat` (ADR-113), which
//! re-exports them and keeps the install-time checks that say a declaration
//! points at something. Reading a manifest is the same answer on the server
//! and in the browser, so it is here.

use serde::{Deserialize, Serialize};

/// The smallest footprint a size may declare, in cells.
///
/// Mirrors `thunderforge_canvas_core`'s `MIN_FOOTPRINT`; a test in
/// `pack_system_spec`'s `combat_tests.rs` keeps the two equal.
pub const MIN_SIZE_FOOTPRINT: f32 = 0.5;

/// The `combat` block.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemCombat {
    /// Which fields are a creature's hit points. Phase 1 of spec 046.
    #[serde(default)]
    pub hit_points: Option<SystemHitPoints>,
    /// What an attack is rolled against — 5e's armour class.
    #[serde(default)]
    pub defence: Option<SystemDefence>,
    /// How big a creature is, and how many cells that fills.
    #[serde(default)]
    pub sizes: Option<SystemSizes>,
    /// Where a creature's legendary actions per round are stored.
    #[serde(default)]
    pub legendary: Option<SystemFieldRef>,
}

/// Where a creature's hit points live.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemHitPoints {
    /// `resourceData`, and so on — the manifest's slot vocabulary.
    pub slot: String,
    pub current: String,
    pub max: String,
    /// Spent before `current` when damage lands. Absent for a system with no
    /// temporary hit points.
    #[serde(default)]
    pub temporary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemDefence {
    pub slot: String,
    pub field: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub abbrev: Option<String>,
}

/// One field in one slot.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemFieldRef {
    pub slot: String,
    pub field: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemSizes {
    pub source: SystemFieldRef,
    pub categories: Vec<SystemSizeCategory>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemSizeCategory {
    pub id: String,
    pub label: String,
    /// Cells per side. At least [`MIN_SIZE_FOOTPRINT`].
    pub footprint: f32,
}

/// The `turnStructure` block, typed.
///
/// `rounds` and `roundLabel` predate spec 046 (spec 031, read by the server's
/// `turn_structure.rs`); `budget` is new.
//
// The doc comments on these types are the install-time schema's descriptions
// (`schema` feature): they moved verbatim and stay so.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemTurnStructure {
    #[serde(default)]
    pub rounds: Option<bool>,
    #[serde(default)]
    pub round_label: Option<String>,
    #[serde(default)]
    pub budget: Option<SystemTurnBudget>,
}

/// What one turn affords. Shown and spent, never refused (spec 046 decision 2).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemTurnBudget {
    #[serde(default)]
    pub action: Option<u32>,
    #[serde(default)]
    pub bonus_action: Option<u32>,
    #[serde(default)]
    pub reaction: Option<u32>,
    #[serde(default)]
    pub movement: Option<SystemBudgetMovement>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemBudgetMovement {
    /// A key of the pack's `movement` block — 5e's `walk`.
    pub speed: String,
}

/// `resourceData` → `resource_data`; a snake-case name is returned unchanged.
///
/// Declarations use the manifest's camel-case slot vocabulary (as `vision`
/// and `resources` do); `data_types` is keyed in snake case.
pub fn slot_key(slot: &str) -> String {
    let mut out = String::with_capacity(slot.len() + 4);
    for ch in slot.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// A system's `combat` block from its manifest. Absent, unreadable or
/// malformed all read as "no combat block" (M1).
pub fn combat_from_manifest(manifest: &serde_json::Value) -> SystemCombat {
    manifest
        .get("combat")
        .and_then(|block| serde_json::from_value::<SystemCombat>(block.clone()).ok())
        .unwrap_or_default()
}

/// A system's turn budget, if it declares one, from its manifest.
pub fn turn_budget_from_manifest(manifest: &serde_json::Value) -> Option<SystemTurnBudget> {
    manifest
        .get("turnStructure")
        .and_then(|t| t.get("budget"))
        .and_then(|b| serde_json::from_value::<SystemTurnBudget>(b.clone()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_slot_is_keyed_in_snake_case() {
        assert_eq!(slot_key("resourceData"), "resource_data");
        assert_eq!(slot_key("trait_data"), "trait_data");
    }

    #[test]
    fn a_malformed_block_reads_as_absent() {
        assert!(
            combat_from_manifest(&json!({ "combat": { "hitPoints": 7 } }))
                .hit_points
                .is_none()
        );
        assert!(turn_budget_from_manifest(&json!({ "turnStructure": {} })).is_none());
    }
}
