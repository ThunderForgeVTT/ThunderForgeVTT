use super::*;

fn packs() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .to_string_lossy()
        .into_owned()
}

fn genie_actor(level: Option<i64>) -> ActorSlots {
    let mut trait_data = serde_json::json!({ "active_conditions": ["bound"] });
    if let Some(level) = level {
        trait_data["level"] = serde_json::json!(level);
    }
    ActorSlots {
        ability_data: Some(serde_json::json!({
            "might": 3, "cunning": 2, "spirit": 4
        })),
        resource_data: Some(serde_json::json!({
            "current_health": 8, "max_health": 10,
            "current_wish_points": 1, "max_wish_points": 5
        })),
        proficiency_data: None,
        trait_data: Some(trait_data),
    }
}

fn find<'a>(values: &'a [DeclaredValue], id: &str) -> Option<&'a DeclaredValue> {
    values.iter().find(|v| v.id == id)
}

/// The end of the wire: a manifest declares, a pack computes, and one set
/// comes out with each value saying which half it came from.
#[test]
fn a_genie_actor_reports_stored_and_derived_values_through_one_path() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    let might = find(&values, "might").expect("a declared attribute");
    assert_eq!(might.value.as_integer(), Some(3));
    assert_eq!(might.origin, Origin::Stored);

    let wish = find(&values, "wishPointsForLevel").expect("genie derives this");
    assert_eq!(
        wish.value.as_integer(),
        Some(5),
        "the manifest's ladder gives a level-4 Genie five Wish Points"
    );
    assert_eq!(wish.origin, Origin::Derived);
}

/// The rule's input lives in the trait slot, which is not where a sheet's
/// attributes come from. This is the case `resolve`'s two arguments exist for.
#[test]
fn a_rule_reads_a_slot_the_attribute_list_never_touches() {
    let with_level = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));
    let without = declared_values_for_actor(&packs(), "genie", &genie_actor(None));

    assert!(find(&with_level, "wishPointsForLevel").is_some());
    assert!(
        find(&without, "wishPointsForLevel").is_none(),
        "no level recorded, nothing to look up — omitted rather than defaulted"
    );
    assert!(
        find(&without, "might").is_some(),
        "and the stored half is unaffected"
    );
}

/// A raw stored field is legible to a rule and is not thereby on the sheet.
#[test]
fn stored_fields_a_system_never_declared_stay_out_of_the_answer() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    for hidden in ["level", "active_conditions", "current_health"] {
        assert!(
            find(&values, hidden).is_none(),
            "{hidden} is readable by a rule, not published as an attribute"
        );
    }
}

/// FR-019, on the values side: a system this build does not have costs the
/// stored half nothing.
#[test]
fn an_unknown_system_still_reports_nothing_rather_than_failing() {
    let values = declared_values_for_actor(&packs(), "no_such_system", &genie_actor(Some(4)));
    assert!(
        values.is_empty(),
        "no declarations, so nothing to publish — and no panic"
    );
}

/// A system that computes nothing is not a broken one.
///
/// Fate Core, which declares eighteen skills, **zero** abilities and no rules
/// implementation. Originally written against 5e, which derived nothing at the
/// time; 5e now derives most of its sheet, so the assertion moved to a system
/// where "nothing derived" is a fact about the ruleset rather than a fact
/// about how far the work had got.
#[test]
fn a_system_with_no_rules_reports_its_stored_values_unchanged() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "athletics": 3 })),
        resource_data: Some(serde_json::json!({ "fate_points": 3, "refresh": 3 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "fate_core", &slots);

    assert!(
        values.iter().all(|v| v.origin == Origin::Stored),
        "Fate Core computes nothing, which is a property of the ruleset"
    );
}

// ---------------------------------------------------------------------------
// T019a: a pool arrives as two numbers, not as text to be parsed
// ---------------------------------------------------------------------------

/// The regression this exists to prevent: a bar recovered by parsing `"4 / 7"`
/// back apart. That is branching on what a value means, and a system writing
/// `"4 of 7"` would have lost its bar with nothing failing anywhere.
#[test]
fn a_genie_actors_pools_arrive_with_their_maximums_intact() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    let health = find(&values, "health").expect("genie declares a health pool");
    assert_eq!(
        health.value,
        DeclaredValueKind::Fraction {
            current: 8,
            max: Some(10)
        },
        "both halves together, as numbers"
    );
    assert_eq!(
        health.origin,
        Origin::Stored,
        "a pool is typed in, not derived"
    );

    let wish = find(&values, "wishPoints").expect("genie declares Wish Points");
    assert_eq!(
        wish.value,
        DeclaredValueKind::Fraction {
            current: 1,
            max: Some(5)
        }
    );
}

/// A system that declares no pools gets none — which is correct for a ruleset
/// that tracks none, not a gap to fill with a default.
#[test]
fn a_system_with_no_declared_resources_publishes_none() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "athletics": 2 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "fate_core", &slots);
    assert!(
        values
            .iter()
            .all(|v| !matches!(v.value, DeclaredValueKind::Fraction { .. })),
        "Fate Core declares no pools"
    );
}

/// An actor whose sheet has no resource slot is not an actor with empty pools.
#[test]
fn an_actor_with_no_resource_data_publishes_no_pools() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "might": 3 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "genie", &slots);
    assert!(find(&values, "health").is_none());
    assert!(
        find(&values, "might").is_some(),
        "and the attributes are unaffected"
    );
}

/// 5e derives, now — the second system to do so, and the one whose derived
/// half is most of its sheet.
#[test]
fn a_5e_actor_derives_its_modifiers_saves_and_skills() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({
            "strength": 10, "dexterity": 16, "constitution": 14,
            "intelligence": 8, "wisdom": 12, "charisma": 7
        })),
        resource_data: Some(serde_json::json!({ "current_hp": 22, "max_hp": 38 })),
        proficiency_data: Some(serde_json::json!({
            "skill_proficiencies": ["stealth", "perception"],
            "saving_throw_proficiencies": ["dexterity"]
        })),
        trait_data: Some(serde_json::json!({ "level": 5 })),
    };
    let values = declared_values_for_actor(&packs(), "dnd5e", &slots);

    let modifier = find(&values, "dexterityMod").expect("modifiers are derived");
    assert_eq!(modifier.value.as_integer(), Some(3));
    assert_eq!(modifier.origin, Origin::Derived);

    // The charisma 7 case: floor, not truncation. -2, never -1.
    assert_eq!(
        find(&values, "charismaMod").and_then(|v| v.value.as_integer()),
        Some(-2)
    );

    assert_eq!(
        find(&values, "passivePerception").and_then(|v| v.value.as_integer()),
        Some(14)
    );

    // And the stored half is untouched, pools included.
    let hp = find(&values, "hitPoints").expect("5e declares a hit point pool");
    assert_eq!(
        hp.value,
        DeclaredValueKind::Fraction {
            current: 22,
            max: Some(38)
        }
    );
    assert_eq!(
        find(&values, "strength").map(|v| v.origin),
        Some(Origin::Stored)
    );
}
