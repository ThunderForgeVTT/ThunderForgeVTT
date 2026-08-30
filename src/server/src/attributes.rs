//! Reading a system's declared attributes out of its manifest.
//!
//! The companion to `status_display::declarations_for_system`, and
//! deliberately the same shape: the manifest is the authority on what a
//! system has, this file only parses it, and the rules for turning stored
//! data into values live in `thunderforge-canvas-core::attributes` where
//! tests execute.
//!
//! # Why the manifest and not a Rust trait
//!
//! `src/engine/src/systems/core.rs` declares a `GameSystem` trait with an
//! `ability_names() -> Vec<&'static str>`, which is the other way this could
//! have gone: one compiled-in implementation per ruleset. It has a single
//! stub implementation, nothing depends on it, and it duplicates a list the
//! manifests already carry — every shipping system declares its own
//! attributes in `system.json` today.
//!
//! Two sources of truth for the same list is the failure worth avoiding, and
//! between them the manifest is the one that ships with a system pack rather
//! than requiring a rebuild of the engine. Where system *rules* should
//! execute is a larger question and is not settled here.

use thunderforge_canvas_core::attributes::AttributeDeclaration;
use thunderforge_canvas_core::movement_budget::MovementDeclaration;

/// The attributes a system declares, in declaration order.
///
/// An unreadable or absent manifest yields none, which is correct rather than
/// defensive: a system that declares no attributes has none, and inventing a
/// D&D-shaped set for it would put six scores on a three-attribute character
/// sheet.
pub fn attribute_declarations_for_system(
    systems_dir: &str,
    system_id: &str,
) -> Vec<AttributeDeclaration> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");

    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    declarations_from_manifest(&manifest)
}

/// Split out so it can be tested without a filesystem.
pub fn declarations_from_manifest(manifest: &serde_json::Value) -> Vec<AttributeDeclaration> {
    let Some(block) = manifest.get("abilities").and_then(|a| a.as_object()) else {
        return Vec::new();
    };

    block
        .iter()
        .enumerate()
        .map(|(index, (id, raw))| {
            AttributeDeclaration {
                id: id.clone(),
                // A declaration with no label falls back to its id rather than
                // being dropped: the system clearly means the attribute to
                // exist, and a missing label is a cosmetic defect where
                // dropping it would silently remove a score from a sheet.
                label: raw
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or(id)
                    .to_string(),
                abbreviation: raw
                    .get("abbreviation")
                    .and_then(|a| a.as_str())
                    .map(str::to_string),
                source: raw
                    .get("source")
                    .and_then(|s| s.as_str())
                    .unwrap_or(id)
                    .to_string(),
                // `order` is required in practice, and every shipping
                // manifest now carries it.
                //
                // The fallback is the *iteration* index, which is not the
                // manifest's own key order: serde_json is built here without
                // `preserve_order`, so its objects are BTreeMaps and iterate
                // alphabetically. A system relying on the fallback therefore
                // gets its attributes alphabetised — charisma before
                // strength, which is nobody's character sheet. This was a
                // live bug, and it hid because the one system whose
                // attributes happen to be alphabetical (Blades: insight,
                // prowess, resolve) passed while the others did not.
                order: raw
                    .get("order")
                    .and_then(|o| o.as_u64())
                    .map(|o| o as usize)
                    .unwrap_or(index),
            }
        })
        .collect()
}

/// The movement types a system declares, in declaration order.
///
/// A system that declares none has none — Blades in the Dark measures no
/// movement at all, because position there is fictional rather than gridded.
/// Inventing a walk speed for it would be this file deciding another
/// ruleset's rules.
pub fn movement_declarations_for_system(
    systems_dir: &str,
    system_id: &str,
) -> Vec<MovementDeclaration> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");

    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    movement_from_manifest(&manifest)
}

/// Split out so it can be tested without a filesystem.
pub fn movement_from_manifest(manifest: &serde_json::Value) -> Vec<MovementDeclaration> {
    let Some(block) = manifest.get("movement").and_then(|m| m.as_object()) else {
        return Vec::new();
    };

    block
        .iter()
        .enumerate()
        .map(|(index, (id, raw))| MovementDeclaration {
            id: id.clone(),
            label: raw
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or(id)
                .to_string(),
            source: raw
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or(id)
                .to_string(),
            default: raw
                .get("default")
                .and_then(|d| d.as_f64())
                .map(|d| d as f32),
            // Same trap as attributes: without an explicit order these
            // alphabetise, because serde_json objects are BTreeMaps here.
            order: raw
                .get("order")
                .and_then(|o| o.as_u64())
                .map(|o| o as usize)
                .unwrap_or(index),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The real `packs/systems`, located from the crate rather than the
    /// working directory — `cargo test` runs with the crate root as cwd, so a
    /// bare relative path silently finds nothing and every system looks like
    /// it declares no attributes.
    fn packs() -> String {
        format!("{}/../../packs/systems", env!("CARGO_MANIFEST_DIR"))
    }

    /// Every shipping system, read from its real manifest.
    ///
    /// Not a fixture: the point is that four rulesets with disjoint attribute
    /// sets all parse, and a change to any manifest that broke one would be
    /// caught here rather than at a table.
    #[test]
    fn every_shipping_system_declares_its_own_attributes() {
        let expected = [
            ("genie", vec!["might", "cunning", "spirit"]),
            (
                "dnd5e",
                vec![
                    "strength",
                    "dexterity",
                    "constitution",
                    "intelligence",
                    "wisdom",
                    "charisma",
                ],
            ),
            (
                "pathfinder2e",
                vec![
                    "strength",
                    "dexterity",
                    "constitution",
                    "intelligence",
                    "wisdom",
                    "charisma",
                ],
            ),
            ("blades_in_the_dark", vec!["insight", "prowess", "resolve"]),
        ];

        for (system, ids) in expected {
            let declared = attribute_declarations_for_system(&packs(), system);
            let mut got: Vec<&str> = declared.iter().map(|d| d.id.as_str()).collect();
            let mut want = ids.clone();
            got.sort_unstable();
            want.sort_unstable();
            assert_eq!(got, want, "{system} declared the wrong attribute set");

            for declaration in &declared {
                assert!(
                    !declaration.label.is_empty(),
                    "{system}/{} has no label",
                    declaration.id
                );
                assert_eq!(
                    declaration.source, declaration.id,
                    "{system}/{} relies on the source defaulting to its id",
                    declaration.id
                );
            }
        }
    }

    /// The two systems that are not D&D-shaped, stated explicitly.
    ///
    /// This is the assertion that would have failed against the six-field
    /// struct this work replaces, so it is worth being unmissable rather than
    /// folded into the loop above.
    #[test]
    fn a_three_attribute_system_is_not_forced_into_six() {
        for system in ["genie", "blades_in_the_dark"] {
            let declared = attribute_declarations_for_system(&packs(), system);
            assert_eq!(declared.len(), 3, "{system} has three attributes");
            assert!(
                !declared.iter().any(|d| d.id == "dexterity"),
                "{system} has no dexterity, and nothing may invent one for it"
            );
        }
    }

    /// Every shipping manifest must carry explicit `order`.
    ///
    /// Without it they alphabetise, because serde_json objects are BTreeMaps
    /// here. That is silent, plausible-looking, and wrong — so this asserts
    /// the manifests do not rely on the fallback rather than asserting the
    /// fallback behaves.
    #[test]
    fn shipping_manifests_declare_their_order_explicitly() {
        for system in ["genie", "dnd5e", "pathfinder2e", "blades_in_the_dark"] {
            let declared = attribute_declarations_for_system(&packs(), system);
            let mut orders: Vec<usize> = declared.iter().map(|d| d.order).collect();
            orders.sort_unstable();
            let expected: Vec<usize> = (0..declared.len()).collect();
            assert_eq!(
                orders, expected,
                "{system} must number its attributes 0..n, or they alphabetise"
            );
        }
    }

    /// The order a book uses, which is not alphabetical.
    #[test]
    fn dnd_attributes_come_back_in_sheet_order_not_alphabetical() {
        let declared = attribute_declarations_for_system(&packs(), "dnd5e");
        let mut by_order: Vec<&AttributeDeclaration> = declared.iter().collect();
        by_order.sort_by_key(|d| d.order);
        let ids: Vec<&str> = by_order.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "strength",
                "dexterity",
                "constitution",
                "intelligence",
                "wisdom",
                "charisma"
            ]
        );
    }

    #[test]
    fn a_missing_or_unreadable_manifest_declares_nothing() {
        assert!(attribute_declarations_for_system(&packs(), "no_such_system").is_empty());
        assert!(declarations_from_manifest(&json!({})).is_empty());
        assert!(declarations_from_manifest(&json!({ "abilities": [] })).is_empty());
    }

    #[test]
    fn a_declaration_may_name_its_own_storage_field_and_order() {
        let declared = declarations_from_manifest(&json!({
            "abilities": {
                "might": { "label": "Might", "source": "mgt", "order": 5 }
            }
        }));
        assert_eq!(declared[0].source, "mgt");
        assert_eq!(declared[0].order, 5);
    }

    /// A label is cosmetic; an attribute is not.
    #[test]
    fn an_attribute_without_a_label_keeps_its_id_rather_than_vanishing() {
        let declared = declarations_from_manifest(&json!({ "abilities": { "grit": {} } }));
        assert_eq!(declared.len(), 1);
        assert_eq!(declared[0].label, "grit");
        assert_eq!(declared[0].source, "grit");
    }

    /// What each shipping system says about movement, including the one that
    /// says nothing.
    #[test]
    fn movement_declarations_match_each_system() {
        let genie = movement_declarations_for_system(&packs(), "genie");
        assert_eq!(genie.len(), 1, "Genie measures one abstract stride");
        assert_eq!(genie[0].id, "stride");
        assert_eq!(genie[0].default, Some(6.0));

        for d20 in ["dnd5e", "pathfinder2e"] {
            let declared = movement_declarations_for_system(&packs(), d20);
            let mut ids: Vec<&str> = declared.iter().map(|d| d.id.as_str()).collect();
            ids.sort_unstable();
            assert_eq!(ids, vec!["burrow", "climb", "fly", "swim", "walk"]);

            // Only the ground speed defaults. Anything else would hand wings
            // to every creature whose sheet omits them.
            for declaration in &declared {
                if declaration.id == "walk" {
                    assert!(declaration.default.is_some(), "{d20} walk needs a default");
                } else {
                    assert!(
                        declaration.default.is_none(),
                        "{d20}/{} must not default",
                        declaration.id
                    );
                }
            }
        }

        assert!(
            movement_declarations_for_system(&packs(), "blades_in_the_dark").is_empty(),
            "Blades measures no movement, and that is a statement rather than a gap"
        );
    }

    /// Pathfinder's ground speed is 25 and it does not call it "Walk".
    #[test]
    fn a_system_may_name_and_scale_its_ground_speed_its_own_way() {
        let pf = movement_declarations_for_system(&packs(), "pathfinder2e");
        let walk = pf.iter().find(|d| d.id == "walk").expect("a ground speed");
        assert_eq!(walk.label, "Speed");
        assert_eq!(walk.default, Some(25.0));

        let dnd = movement_declarations_for_system(&packs(), "dnd5e");
        let dnd_walk = dnd.iter().find(|d| d.id == "walk").expect("a ground speed");
        assert_eq!(dnd_walk.label, "Walk");
        assert_eq!(dnd_walk.default, Some(30.0));
    }

    #[test]
    fn movement_declarations_are_ordered_explicitly() {
        for system in ["genie", "dnd5e", "pathfinder2e"] {
            let declared = movement_declarations_for_system(&packs(), system);
            let mut orders: Vec<usize> = declared.iter().map(|d| d.order).collect();
            orders.sort_unstable();
            assert_eq!(orders, (0..declared.len()).collect::<Vec<_>>());
        }
    }
}
