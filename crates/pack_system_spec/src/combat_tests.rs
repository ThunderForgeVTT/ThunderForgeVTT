//! Spec 046 T004: the `combat` block's install-time checks, M1–M4.

use super::*;
use crate::validate_system_manifest;
use serde_json::{Value, json};

/// A manifest that passes everything else, with a 5e-shaped `data_types` and
/// `movement`, so each test varies only the block it is about.
fn manifest(extra: Value) -> String {
    let mut base = json!({
        "id": "t", "title": "T", "authors": [], "version": "1.0.0",
        "compatibility": { "minimum": "1.0.0" },
        "esmodules": [], "styles": [], "packs": [],
        "legal": { "licenseName": "n", "attributionText": "a" },
        "data_types": {
            "resource_data": { "properties": {
                "current_hp": { "type": "integer" },
                "max_hp": { "type": "integer" },
                "temporary_hp": { "type": "integer" }
            }},
            "ability_data": { "properties": { "armor_class": { "type": "integer" } } },
            "trait_data": { "properties": {
                "size": { "type": "string" },
                "legendary_actions": { "type": "integer" }
            }}
        },
        "movement": { "walk": { "label": "Walk", "source": "speed_walk" } },
    });
    for (key, value) in extra.as_object().expect("an object") {
        base[key] = value.clone();
    }
    base.to_string()
}

fn hit_points() -> Value {
    json!({ "slot": "resourceData", "current": "current_hp", "max": "max_hp",
            "temporary": "temporary_hp" })
}

#[test]
fn m1_a_manifest_with_no_combat_block_is_accepted() {
    assert!(validate_system_manifest(&manifest(json!({}))).is_ok());
    assert!(validate_system_manifest(&manifest(json!({ "combat": {} }))).is_ok());
    assert!(validate_system_manifest(&manifest(json!({ "combat": null }))).is_ok());
}

#[test]
fn a_complete_combat_block_is_accepted() {
    let json = manifest(json!({
        "combat": {
            "hitPoints": hit_points(),
            "defence": { "slot": "abilityData", "field": "armor_class",
                         "label": "Armour Class", "abbrev": "AC" },
            "sizes": {
                "source": { "slot": "traitData", "field": "size" },
                "categories": [
                    { "id": "tiny", "label": "Tiny", "footprint": 0.5 },
                    { "id": "large", "label": "Large", "footprint": 2 }
                ]
            },
            "legendary": { "slot": "traitData", "field": "legendary_actions" }
        },
        "turnStructure": { "rounds": true, "roundLabel": "Round",
            "budget": { "action": 1, "bonusAction": 1, "reaction": 1,
                        "movement": { "speed": "walk" } } }
    }));
    validate_system_manifest(&json).expect("accepted");
}

#[test]
fn m2_a_hit_point_field_the_pack_does_not_declare_is_refused_by_name() {
    for (key, field) in [
        ("current", "curent_hp"),
        ("max", "maximum"),
        ("temporary", "temp"),
    ] {
        let mut hp = hit_points();
        hp[key] = json!(field);
        let error = validate_system_manifest(&manifest(json!({ "combat": { "hitPoints": hp } })))
            .expect_err("refused");
        assert!(
            error.contains(&format!("combat.hitPoints.{key}")),
            "{error}"
        );
        assert!(error.contains(field), "{error}");
    }
}

#[test]
fn m2_a_slot_the_pack_does_not_declare_is_refused_by_name() {
    let mut hp = hit_points();
    hp["slot"] = json!("spellData");
    let error = validate_system_manifest(&manifest(json!({ "combat": { "hitPoints": hp } })))
        .expect_err("refused");
    assert!(error.contains("spellData"), "{error}");

    for (block, value) in [
        ("defence", json!({ "slot": "abilityData", "field": "ac" })),
        (
            "legendary",
            json!({ "slot": "traitData", "field": "legendary" }),
        ),
        (
            "sizes",
            json!({ "source": { "slot": "traitData", "field": "bulk" }, "categories": [] }),
        ),
    ] {
        let error = validate_system_manifest(&manifest(json!({ "combat": { block: value } })))
            .expect_err("refused");
        assert!(error.contains(&format!("combat.{block}")), "{error}");
    }
}

#[test]
fn m3_a_footprint_smaller_than_the_canvas_draws_is_refused() {
    let json = manifest(json!({ "combat": { "sizes": {
        "source": { "slot": "traitData", "field": "size" },
        "categories": [ { "id": "speck", "label": "Speck", "footprint": 0.25 } ]
    }}}));
    let error = validate_system_manifest(&json).expect_err("refused");
    assert!(error.contains("footprint"), "{error}");
    assert!(error.contains("speck"), "{error}");
}

#[test]
fn m3_a_size_id_declared_twice_is_refused() {
    let json = manifest(json!({ "combat": { "sizes": {
        "source": { "slot": "traitData", "field": "size" },
        "categories": [
            { "id": "large", "label": "Large", "footprint": 2 },
            { "id": "large", "label": "Huge", "footprint": 3 }
        ]
    }}}));
    let error = validate_system_manifest(&json).expect_err("refused");
    assert!(error.contains("large"), "{error}");
    assert!(error.contains("twice"), "{error}");
}

#[test]
fn m4_a_budget_speed_the_movement_block_does_not_declare_is_refused() {
    let json = manifest(json!({ "turnStructure": { "rounds": true,
        "budget": { "action": 1, "movement": { "speed": "teleport" } } } }));
    let error = validate_system_manifest(&json).expect_err("refused");
    assert!(
        error.contains("turnStructure.budget.movement.speed"),
        "{error}"
    );
    assert!(error.contains("teleport"), "{error}");
}

#[test]
fn slot_names_are_read_in_either_spelling() {
    assert_eq!(slot_key("resourceData"), "resource_data");
    assert_eq!(slot_key("resource_data"), "resource_data");
    assert_eq!(slot_key("traitData"), "trait_data");
}

#[test]
fn the_minimum_footprint_is_the_canvas_cores() {
    assert_eq!(
        MIN_SIZE_FOOTPRINT,
        thunderforge_canvas_core::grid::MIN_FOOTPRINT
    );
}

#[test]
fn the_shipped_dnd5e_combat_block_is_usable() {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packs/systems/dnd5e/system.json"
    ))
    .expect("the 5e manifest is readable");
    let parsed: Value = serde_json::from_str(&json).expect("valid JSON");
    let combat: SystemCombat =
        serde_json::from_value(parsed["combat"].clone()).expect("5e declares a combat block");
    assert_eq!(
        combat.hit_points.map(|hp| hp.current),
        Some("current_hp".to_string())
    );
    validate_combat_content(&parsed).expect("its combat block is usable");

    // Spec 046 Phase 7: Large is two squares, and reach is not in it.
    let sizes = combat.sizes.expect("5e declares its sizes");
    assert_eq!(sizes.source.field, "size");
    let footprint = |id: &str| {
        sizes
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.footprint)
    };
    assert_eq!(footprint("tiny"), Some(0.5));
    assert_eq!(footprint("medium"), Some(1.0));
    assert_eq!(footprint("large"), Some(2.0));
    assert_eq!(footprint("gargantuan"), Some(4.0));
}

#[test]
fn the_shipped_genie_sizes_live_in_its_combat_block() {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packs/systems/genie/system.json"
    ))
    .expect("the Genie manifest is readable");
    let parsed: Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(
        parsed.get("sizeCategories").is_none(),
        "sizeCategories moved under combat.sizes"
    );
    validate_combat_content(&parsed).expect("its combat block is usable");
    let combat: SystemCombat =
        serde_json::from_value(parsed["combat"].clone()).expect("Genie declares a combat block");
    let sizes = combat.sizes.expect("Genie declares its sizes");
    assert_eq!(sizes.source.field, "size_category");
    assert_eq!(sizes.categories.len(), 6);
    assert!(
        combat.hit_points.is_none(),
        "Genie declares no hit points (M1)"
    );
}
