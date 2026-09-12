use super::*;

/// One actor's traits, in the slot a declaration reads by default.
fn traits(sheet: serde_json::Value) -> ActorSlotData {
    ActorSlotData::from([(DEFAULT_SLOT.to_string(), sheet)])
}

/// D&D 5e's block, as its manifest declares it.
fn dnd5e() -> VisionDeclaration {
    VisionDeclaration {
        units_per_cell: None,
        unit_label: None,
        darkvision: Some(DistanceSource {
            slot: None,
            source: "darkvision".to_string(),
            unit: Some("feet".to_string()),
            default: None,
        }),
        carried_light: Some(CarriedLightDeclaration {
            bright: Some(DistanceSource {
                slot: None,
                source: "light_bright".to_string(),
                unit: Some("feet".to_string()),
                default: None,
            }),
            dim: Some(DistanceSource {
                slot: None,
                source: "light_dim".to_string(),
                unit: Some("feet".to_string()),
                default: None,
            }),
        }),
    }
}

/// A 5-foot square, which is what a 5e scene uses.
fn feet() -> GridUnits {
    GridUnits::new(5.0, "ft")
}

#[test]
fn a_dwarfs_sixty_feet_of_darkvision_is_twelve_cells() {
    let sheet = serde_json::json!({ "darkvision": 60 });
    let resolved = vision_from(&traits(sheet.clone()), &dnd5e(), &feet());
    assert_eq!(resolved.darkvision, 12.0);
}

#[test]
fn the_same_sixty_feet_is_six_cells_on_a_ten_foot_grid() {
    // The whole reason units are converted per scene rather than stored: the
    // ruleset's number does not change, the board's meaning of it does.
    let sheet = serde_json::json!({ "darkvision": 60 });
    let resolved = vision_from(
        &traits(sheet.clone()),
        &dnd5e(),
        &GridUnits::new(10.0, "ft"),
    );
    assert_eq!(resolved.darkvision, 6.0);
}

#[test]
fn a_sheet_that_says_nothing_gives_ordinary_sight() {
    let resolved = vision_from(&traits(serde_json::json!({})), &dnd5e(), &feet());
    assert_eq!(resolved, ResolvedVision::default());
    assert!(resolved.is_ordinary());
}

#[test]
fn a_system_that_declares_nothing_gives_ordinary_sight() {
    // FR-066. Genie today, and every system that never takes this up.
    let sheet = serde_json::json!({ "darkvision": 60, "light_bright": 20 });
    let resolved = vision_from(
        &traits(sheet.clone()),
        &VisionDeclaration::default(),
        &feet(),
    );
    assert!(
        resolved.is_ordinary(),
        "a field nobody declared is not read, however suggestively it is named"
    );
}

#[test]
fn a_torch_resolves_both_reaches() {
    let sheet = serde_json::json!({ "light_bright": 20, "light_dim": 40 });
    let resolved = vision_from(&traits(sheet.clone()), &dnd5e(), &feet());
    assert_eq!(resolved.carried_bright, 4.0);
    assert_eq!(resolved.carried_dim, 8.0);
    assert!(!resolved.is_ordinary());
}

#[test]
fn a_sheet_storing_a_value_object_reads_the_same_as_a_bare_number() {
    // The shape a real sheet uses, and the one `movement_budget` already
    // accepts — these are the same sheets.
    let sheet = serde_json::json!({ "darkvision": { "value": 60, "source": "race" } });
    assert_eq!(
        vision_from(&traits(sheet.clone()), &dnd5e(), &feet()).darkvision,
        12.0
    );
}

#[test]
fn broken_data_is_absent_rather_than_a_creature_that_sees_backwards() {
    for poison in [
        serde_json::json!({ "darkvision": -60 }),
        serde_json::json!({ "darkvision": "sixty" }),
        serde_json::json!({ "darkvision": null }),
    ] {
        assert_eq!(
            vision_from(&traits(poison.clone()), &dnd5e(), &feet()).darkvision,
            0.0,
            "{poison} must read as no darkvision"
        );
    }
}

#[test]
fn a_declared_default_applies_only_when_the_sheet_is_silent() {
    let declaration = VisionDeclaration {
        units_per_cell: None,
        unit_label: None,
        darkvision: Some(DistanceSource {
            slot: None,
            source: "darkvision".to_string(),
            unit: None,
            default: Some(30.0),
        }),
        carried_light: None,
    };
    assert_eq!(
        vision_from(&traits(serde_json::json!({})), &declaration, &feet()).darkvision,
        6.0,
        "silent: the default"
    );
    assert_eq!(
        vision_from(
            &traits(serde_json::json!({ "darkvision": 60 })),
            &declaration,
            &feet()
        )
        .darkvision,
        12.0,
        "stated: the sheet"
    );
    assert_eq!(
        vision_from(
            &traits(serde_json::json!({ "darkvision": 0 })),
            &declaration,
            &feet()
        )
        .darkvision,
        0.0,
        "an explicit zero is a creature that cannot see in the dark, and must \
         not be overwritten by the default"
    );
}

#[test]
fn a_nonsense_grid_falls_back_rather_than_dividing_by_zero() {
    let sheet = serde_json::json!({ "darkvision": 60 });
    // A scene configured with a zero or negative scale: `safe_per_cell`
    // stands in, so this is 60ft at the 5ft default rather than an infinity.
    assert_eq!(
        vision_from(&traits(sheet.clone()), &dnd5e(), &GridUnits::new(0.0, "ft")).darkvision,
        12.0
    );
}

#[test]
fn the_engines_own_profile_carries_the_darkvision_and_nothing_else() {
    let resolved = vision_from(
        &traits(serde_json::json!({ "darkvision": 60 })),
        &dnd5e(),
        &feet(),
    );
    let profile = resolved.profile();
    assert_eq!(profile.darkvision, 12.0);
    assert_eq!(profile.facing, VisionProfile::default().facing);
    assert_eq!(profile.fov, VisionProfile::default().fov);
    assert_eq!(profile.max_range, VisionProfile::default().max_range);
}

#[test]
fn a_block_declares_the_scale_its_numbers_are_quoted_in() {
    // The gap this closes: a scene's `grid_size` is pixels per cell, and no
    // manifest has ever recorded feet per square — `movement` declares "30"
    // and leaves it implicit. A distance that must be *drawn* cannot.
    let mut declaration = dnd5e();
    declaration.units_per_cell = Some(5.0);
    declaration.unit_label = Some("ft".to_string());
    let units = declaration.grid_units();
    assert_eq!(units.per_cell, 5.0);
    assert_eq!(
        vision_from(
            &traits(serde_json::json!({ "darkvision": 60 })),
            &declaration,
            &units
        )
        .darkvision,
        12.0
    );

    // A system measuring in metres, with three-metre squares.
    declaration.units_per_cell = Some(3.0);
    declaration.unit_label = Some("m".to_string());
    assert_eq!(
        vision_from(
            &traits(serde_json::json!({ "darkvision": 18 })),
            &declaration,
            &declaration.grid_units()
        )
        .darkvision,
        6.0
    );
}

#[test]
fn a_block_that_declares_no_scale_falls_back_to_the_five_foot_square() {
    let declaration = dnd5e();
    assert_eq!(declaration.units_per_cell, None);
    assert_eq!(declaration.grid_units().per_cell, 5.0);
}

#[test]
fn a_declaration_reads_the_slot_it_names() {
    // Senses live in `traitData` by default, but a system is free to keep
    // them elsewhere and say so — the same `slot` + `source` pair a `sheet`
    // entry uses.
    let declaration = VisionDeclaration {
        units_per_cell: None,
        unit_label: None,
        darkvision: Some(DistanceSource {
            slot: Some("abilityData".to_string()),
            source: "darkvision".to_string(),
            unit: None,
            default: None,
        }),
        carried_light: None,
    };
    let slots = ActorSlotData::from([
        (
            DEFAULT_SLOT.to_string(),
            serde_json::json!({ "darkvision": 120 }),
        ),
        (
            "abilityData".to_string(),
            serde_json::json!({ "darkvision": 60 }),
        ),
    ]);
    assert_eq!(
        vision_from(&slots, &declaration, &feet()).darkvision,
        12.0,
        "the named slot, not the default one that also happens to carry the key"
    );
}
