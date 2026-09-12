use super::*;

fn slots_with_traits(traits: serde_json::Value) -> ActorSlots {
    ActorSlots {
        ability_data: None,
        resource_data: None,
        proficiency_data: None,
        trait_data: Some(traits),
    }
}

/// The manifest D&D 5e actually ships, read from disk rather than retyped —
/// a test asserting against a copy would keep passing after the real one
/// changed.
fn dnd5e_manifest() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packs/systems/dnd5e/system.json"
    ))
    .expect("the 5e manifest is readable");
    serde_json::from_str(&text).expect("valid JSON")
}

#[test]
fn a_dwarf_with_sixty_feet_of_darkvision_sees_twelve_cells() {
    let declaration = vision_from_manifest(&dnd5e_manifest());
    let resolved = resolve(
        &slots_with_traits(serde_json::json!({ "darkvision": 60 })),
        &declaration,
    );
    assert_eq!(resolved.darkvision, 12.0);
}

#[test]
fn a_human_with_no_darkvision_sees_by_the_default_rules() {
    let declaration = vision_from_manifest(&dnd5e_manifest());
    let resolved = resolve(
        &slots_with_traits(serde_json::json!({ "class": "fighter" })),
        &declaration,
    );
    assert!(resolved.is_ordinary());
}

#[test]
fn a_system_declaring_no_vision_gives_every_token_ordinary_sight() {
    // FR-066, and Genie today. The key is present and suggestively named;
    // nothing reads it, because nothing declared it.
    let declaration = vision_from_manifest(&serde_json::json!({ "id": "genie" }));
    assert_eq!(declaration, VisionDeclaration::default());
    let resolved = resolve(
        &slots_with_traits(serde_json::json!({ "darkvision": 120 })),
        &declaration,
    );
    assert!(resolved.is_ordinary());
}

#[test]
fn a_malformed_vision_block_is_absent_rather_than_an_error() {
    // Install-time validation is where a pack author hears about this. By the
    // time a scene is being drawn, refusing to show anybody anything would be
    // the worse answer.
    let declaration = vision_from_manifest(&serde_json::json!({ "vision": "yes please" }));
    assert_eq!(declaration, VisionDeclaration::default());
}

#[test]
fn a_torch_carried_by_a_character_resolves_both_reaches() {
    let declaration = vision_from_manifest(&dnd5e_manifest());
    let resolved = resolve(
        &slots_with_traits(serde_json::json!({ "light_bright": 20, "light_dim": 40 })),
        &declaration,
    );
    assert_eq!(resolved.carried_bright, 4.0);
    assert_eq!(resolved.carried_dim, 8.0);
}

#[test]
fn a_declaration_reaches_past_the_trait_slot_when_it_says_to() {
    // `slot_data` has to offer every slot under the manifest's own names, or
    // a declaration naming one of the others silently reads nothing.
    let declaration: VisionDeclaration = serde_json::from_value(serde_json::json!({
        "darkvision": { "slot": "abilityData", "source": "darkvision" }
    }))
    .expect("parses");
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "darkvision": 60 })),
        resource_data: None,
        proficiency_data: None,
        trait_data: None,
    };
    assert_eq!(resolve(&slots, &declaration).darkvision, 12.0);
}

#[test]
fn every_slot_is_offered_under_the_name_a_manifest_uses() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({})),
        resource_data: Some(serde_json::json!({})),
        proficiency_data: Some(serde_json::json!({})),
        trait_data: Some(serde_json::json!({})),
    };
    let offered = slot_data(&slots);
    let mut names: Vec<&str> = offered.keys().map(|k| k.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "abilityData",
            "proficiencyData",
            "resourceData",
            "traitData"
        ],
        "camelCase, as a pack author writes it — not the column names"
    );
}

#[test]
fn an_actor_storing_nothing_offers_no_slots_rather_than_empty_ones() {
    let slots = ActorSlots {
        ability_data: None,
        resource_data: None,
        proficiency_data: None,
        trait_data: None,
    };
    assert!(slot_data(&slots).is_empty());
}

#[test]
fn cells_become_pixels_through_the_scenes_own_grid() {
    // `grid_size` is pixels per cell and always has been, whatever it reads
    // like. Twelve cells of darkvision on a 64px grid is 768px of sight.
    assert_eq!(cells_to_world(12.0, 64), 768.0);
    assert_eq!(cells_to_world(12.0, 32), 384.0);
    assert_eq!(cells_to_world(0.0, 64), 0.0);
}

#[test]
fn a_scene_with_a_nonsense_grid_does_not_collapse_sight_to_nothing() {
    // A zero grid size would multiply every distance to zero, which reads as
    // "nobody can see" rather than as a broken scene.
    assert_eq!(cells_to_world(12.0, 0), 12.0);
    assert_eq!(cells_to_world(12.0, -5), 12.0);
}

#[test]
fn the_shipped_5e_declaration_quotes_its_distances_in_five_foot_squares() {
    let declaration = vision_from_manifest(&dnd5e_manifest());
    let units = units_of(&declaration);
    assert_eq!(units.per_cell, 5.0);
    assert_eq!(units.label, "ft");
}
