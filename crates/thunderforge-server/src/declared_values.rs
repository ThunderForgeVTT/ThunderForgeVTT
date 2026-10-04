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

/// The speeds a system declares.
///
/// Every bundled manifest has a `movement` block — Genie's `stride`, 5e's
/// walk/fly/swim/climb — and until now nothing read one. The values existed in
/// the manifest, the layout format had a `movement` set to lay them out, Forge
/// has a section titled for them, and the set arrived empty every time because
/// no code turned the block into values. A sheet section that is always empty
/// is indistinguishable from a system that has no speeds.
///
/// `default` is used when the actor stores nothing, which is the one place in
/// this module where a default is right rather than an omission: a manifest
/// saying `"default": 30` is the system stating a speed every character has
/// until it says otherwise, not a gap being filled in.
fn movement_from(slots: &ActorSlots, systems_dir: &str, system_id: &str) -> Vec<DeclaredValue> {
    let Some(manifest) = read_manifest(systems_dir, system_id) else {
        return Vec::new();
    };
    let Some(block) = manifest.get("movement").and_then(|m| m.as_object()) else {
        return Vec::new();
    };

    let stored = slots.trait_data.as_ref();
    let mut declared: Vec<(i64, DeclaredValue)> = block
        .iter()
        .filter_map(|(id, entry)| {
            let source = entry
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or(id.as_str());
            let value = stored
                .and_then(|slot| slot.get(source))
                .and_then(|v| v.as_i64())
                .or_else(|| entry.get("default").and_then(|d| d.as_i64()))?;

            Some((
                entry.get("order").and_then(|o| o.as_i64()).unwrap_or(0),
                DeclaredValue {
                    id: id.clone(),
                    label: entry
                        .get("label")
                        .and_then(|l| l.as_str())
                        .unwrap_or(id.as_str())
                        .to_string(),
                    abbreviation: None,
                    value: DeclaredValueKind::Integer(i32::try_from(value).ok()?),
                    group: None,
                    group_label: None,
                    headline: false,
                    origin: Origin::Stored,
                },
            ))
        })
        .collect();

    // The system's own order. A manifest object has no inherent one — serde
    // gives them alphabetically — so `order` is what keeps 5e's walk ahead of
    // its climb rather than the alphabet deciding.
    declared.sort_by_key(|(order, _)| *order);
    declared.into_iter().map(|(_, value)| value).collect()
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
/// One actor's values, already sorted into the sets a sheet lays out.
///
/// # Why the server does this and not the client
///
/// Because the server is the only side that knows. A value's set is a fact
/// about which block of the manifest declared it — `abilities`, `resources`,
/// `movement`, `sheet` — and that block is read here. A flat list on the wire
/// left the renderer with six lists to reconstruct and no information to do it
/// with, so everything a system published landed in `other` and Forge's
/// Attributes and Resources sections were empty for every system that has
/// them.
///
/// This is also T019h's real fix rather than its detector. The named sets and
/// `all` cannot disagree when one function produces both from one pass.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActorSheetValues {
    pub attributes: Vec<DeclaredValue>,
    pub resources: Vec<DeclaredValue>,
    pub skills: Vec<DeclaredValue>,
    pub movement: Vec<DeclaredValue>,
    pub derived: Vec<DeclaredValue>,
    /// Everything a named set did not claim, in declaration order.
    pub other: Vec<DeclaredValue>,
    /// Every value, in the order the system declared them.
    pub all: Vec<DeclaredValue>,
}

/// Every value one actor publishes, in its sets.
pub fn actor_sheet_values(
    systems_dir: &str,
    system_id: &str,
    slots: &ActorSlots,
) -> ActorSheetValues {
    let declarations = crate::attributes::attribute_declarations_for_system(systems_dir, system_id);

    let attributes = visible_from(slots, &declarations);
    let resources = resources_from(slots, systems_dir, system_id);
    let movement = movement_from(slots, systems_dir, system_id);

    let ids = |values: &[DeclaredValue]| -> std::collections::BTreeSet<String> {
        values.iter().map(|v| v.id.clone()).collect()
    };
    let (attribute_ids, resource_ids, movement_ids) =
        (ids(&attributes), ids(&resources), ids(&movement));

    let all = declared_values_for_actor(systems_dir, system_id, slots);

    // Sets are read back off `all` rather than off the pieces above, so a value
    // that `resolve` dropped, renamed or replaced is absent from its set too.
    // Building the sets from the inputs would let a sheet show a value the
    // resolved list no longer contains.
    let mut out = ActorSheetValues {
        all: all.clone(),
        ..Default::default()
    };
    for value in all {
        match &value {
            // Derived first: a system may compute a value that shares an
            // identifier with a stored declaration, and what it *is* on the
            // sheet is the computed one.
            v if v.origin == Origin::Derived => out.derived.push(value),
            v if attribute_ids.contains(&v.id) => out.attributes.push(value),
            v if resource_ids.contains(&v.id) => out.resources.push(value),
            v if movement_ids.contains(&v.id) => out.movement.push(value),
            _ => out.other.push(value),
        }
    }

    // `skills` stays empty deliberately, and the reason is a fact about the
    // shipping systems rather than an omission. 5e computes its eighteen from
    // its abilities, so they arrive as derived values; Fate and Cypher have
    // no fixed skill list at all — theirs are player-named slots declared in
    // `sheet`, which is where the player's own names can live. A manifest
    // `skills` block naming a fixed list has no system using it, and inventing
    // a reader for it now would be shaping a construct against no consumer.
    out
}

pub fn declared_values_for_actor(
    systems_dir: &str,
    system_id: &str,
    slots: &ActorSlots,
) -> Vec<DeclaredValue> {
    let declarations = crate::attributes::attribute_declarations_for_system(systems_dir, system_id);
    let mut visible = visible_from(slots, &declarations);
    visible.extend(resources_from(slots, systems_dir, system_id));
    visible.extend(movement_from(slots, systems_dir, system_id));
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
