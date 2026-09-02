//! One actor's values — what its system stores, and what its system computes.
//!
//! The two halves have always been asymmetric. Stored values need no code: a
//! system's manifest declares them and
//! `thunderforge_canvas_core::attributes` reads them. Derived values had
//! nowhere to live at all until spec 032 — the 5e sheet stores an ability
//! score *and* its modifier because paper cannot compute, and this product
//! stored neither computation nor a place to put one.
//!
//! This module joins them, and it is deliberately thin: the rules belong to
//! the pack that owns the ruleset, the reading belongs to canvas-core where
//! tests execute, and what is left here is loading the pieces.

use thunderforge_canvas_core::attributes::{AttributeDeclaration, attributes_from};
use thunderforge_canvas_core::system_rules::{
    DeclaredValue, DeclaredValueKind, DeclaredValues, Origin, resolve,
};

/// The stored slots of one actor's sheet.
///
/// Named rather than passed as a bag, because which slot a field is in is a
/// fact about the system's data model, and a rule reading `level` should not
/// have to guess where it lives.
#[derive(Debug, Default, Clone)]
pub struct ActorSlots {
    pub ability_data: Option<serde_json::Value>,
    pub resource_data: Option<serde_json::Value>,
    pub proficiency_data: Option<serde_json::Value>,
    pub trait_data: Option<serde_json::Value>,
}

/// Everything legible about an actor, for a rule to read.
///
/// Flattened across every slot, because a rule's inputs are not confined to
/// one: Genie's by-level Wish Points rule reads `level` out of the trait slot
/// while the Wish Points themselves live in the resource slot.
///
/// This is the `context` half of
/// [`thunderforge_canvas_core::system_rules::resolve`] — legible to a rule,
/// not automatically shown on a sheet. What a surface presents is the
/// system's declared attributes, and nothing here puts a raw stored field in
/// front of anyone.
fn context_from(slots: &ActorSlots) -> DeclaredValues {
    let mut values = Vec::new();

    for slot in [
        &slots.ability_data,
        &slots.resource_data,
        &slots.proficiency_data,
        &slots.trait_data,
    ]
    .into_iter()
    .flatten()
    {
        let Some(object) = slot.as_object() else {
            continue;
        };
        for (field, raw) in object {
            // Objects and nulls are skipped rather than flattened. A system
            // storing `{"strength": {"value": 14, "proficient": true}}` is
            // describing one score with a note attached, and inventing
            // `strength.proficient` as an identifier here would put a name in
            // the vocabulary that no manifest declared.
            let value = match raw {
                serde_json::Value::Bool(b) => DeclaredValueKind::Boolean(*b),
                serde_json::Value::String(s) => DeclaredValueKind::Text(s.clone()),
                serde_json::Value::Number(n) => {
                    match n.as_i64().and_then(|v| i32::try_from(v).ok()) {
                        Some(int) => DeclaredValueKind::Integer(int),
                        None => match n.as_f64() {
                            Some(float) => DeclaredValueKind::Number(float),
                            None => continue,
                        },
                    }
                }
                serde_json::Value::Array(items) => DeclaredValueKind::List(
                    items
                        .iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect(),
                ),
                _ => continue,
            };

            values.push(DeclaredValue {
                id: field.clone(),
                label: field.clone(),
                abbreviation: None,
                value,
                group: None,
                group_label: None,
                headline: false,
                origin: Origin::Stored,
            });
        }
    }

    DeclaredValues::new(values)
}

/// The resources a system declares, as declared values.
///
/// # Why resources are published here at all
///
/// They were not, and the omission was invisible: a layout could say
/// `barStack of resources` and the renderer would find nothing to draw. Worse,
/// the shape it *would* have found was a rendered string, so the only way to
/// recover a bar was to parse `"4 / 7"` back apart — the exact
/// branching-on-meaning the declared-value contract exists to prevent, and a
/// system writing `"4 of 7"` would have lost its bar with nothing failing.
///
/// So a resource arrives as a [`DeclaredValueKind::Fraction`], with its
/// maximum intact and absent when the system gives none.
///
/// Only the base entry is published. A stacking resource — a shield over a
/// health pool — is more than one bar, and flattening its layers into one
/// identifier would misreport the character. That is worth doing properly
/// rather than approximately, and the canvas already draws the stack from its
/// own path.
fn resources_from(slots: &ActorSlots, systems_dir: &str, system_id: &str) -> Vec<DeclaredValue> {
    crate::status_display::declarations_for_system(systems_dir, system_id)
        .into_iter()
        .filter_map(|declared| {
            let group = declared.group.clone();
            let slot = match declared.source.slot.as_str() {
                "resourceData" | "resource_data" => slots.resource_data.as_ref(),
                "traitData" | "trait_data" => slots.trait_data.as_ref(),
                "abilityData" | "ability_data" => slots.ability_data.as_ref(),
                "proficiencyData" | "proficiency_data" => slots.proficiency_data.as_ref(),
                _ => None,
            }?;

            let entry =
                thunderforge_canvas_core::resource_display::entries_from(slot, &declared.source)
                    .into_iter()
                    .next()?;

            Some(DeclaredValue {
                id: declared.definition.id,
                label: declared.definition.label,
                abbreviation: None,
                value: DeclaredValueKind::Fraction {
                    current: entry.current,
                    max: entry.max,
                },
                group,
                group_label: None,
                headline: false,
                origin: Origin::Stored,
            })
        })
        .collect()
}

/// The attributes a system declares, as declared values.
fn visible_from(slots: &ActorSlots, declarations: &[AttributeDeclaration]) -> Vec<DeclaredValue> {
    let Some(abilities) = slots.ability_data.as_ref() else {
        return Vec::new();
    };

    attributes_from(abilities, declarations)
        .into_iter()
        .map(|attribute| DeclaredValue {
            id: attribute.id,
            label: attribute.label,
            abbreviation: attribute.abbreviation,
            value: DeclaredValueKind::Integer(attribute.value),
            group: None,
            group_label: None,
            headline: false,
            origin: Origin::Stored,
        })
        .collect()
}

/// Everything else the system's sheet declares (FR-031).
///
/// The aspects, the tracks, the ladders and the player-named slots — the parts
/// of a character sheet that are not a score, a skill, a pool or a speed, and
/// which for two of the shipping systems are most of it.
fn sheet_from(slots: &ActorSlots, systems_dir: &str, system_id: &str) -> Vec<DeclaredValue> {
    crate::sheet::declarations_for_system(systems_dir, system_id)
        .into_iter()
        .flat_map(|declaration| {
            let slot = match declaration.slot.as_str() {
                "resourceData" | "resource_data" => slots.resource_data.as_ref(),
                "abilityData" | "ability_data" => slots.ability_data.as_ref(),
                "proficiencyData" | "proficiency_data" => slots.proficiency_data.as_ref(),
                _ => slots.trait_data.as_ref(),
            };
            match slot {
                Some(slot) => crate::sheet::values_from(&declaration, slot),
                // A slot the actor has nothing in. A track and a ladder still
                // exist — an empty stress track is the truth — so they are
                // resolved against an empty object rather than skipped.
                None => crate::sheet::values_from(&declaration, &serde_json::json!({})),
            }
        })
        .collect()
}

/// Every value one actor publishes, stored and derived, in one set.
///
/// A system this build does not have, or one that computes nothing, yields
/// exactly the stored half — which is the whole of FR-019's promise on the
/// values side: a world whose pack is missing still shows what it stored.
pub fn declared_values_for_actor(
    systems_dir: &str,
    system_id: &str,
    slots: &ActorSlots,
) -> Vec<DeclaredValue> {
    let declarations = crate::attributes::attribute_declarations_for_system(systems_dir, system_id);
    let mut visible = visible_from(slots, &declarations);
    visible.extend(resources_from(slots, systems_dir, system_id));
    visible.extend(sheet_from(slots, systems_dir, system_id));
    let context = context_from(slots);

    // Rules are built from the pack's own manifest, so tables like Genie's
    // by-level Wish Points ladder stay in the file that owns them.
    let manifest = read_manifest(systems_dir, system_id);
    let rules = manifest
        .as_ref()
        .and_then(|m| crate::systems::rules_for_system(system_id, m));

    let mut values = resolve(rules.as_deref(), visible, &context);

    // After `resolve`, not before: a derived value can belong to a group too,
    // and stamping the stored half only would give one member of a group a
    // name and its neighbour none (T019g).
    if let Some(manifest) = manifest.as_ref() {
        crate::sheet::apply_groups(&mut values, &crate::sheet::groups_from_manifest(manifest));
    }

    values
}

fn read_manifest(systems_dir: &str, system_id: &str) -> Option<serde_json::Value> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
#[path = "declared_values_tests.rs"]
mod tests;
