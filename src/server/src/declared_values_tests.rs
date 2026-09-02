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
#[test]
fn a_system_with_no_rules_reports_its_stored_values_unchanged() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({
            "strength": 14, "dexterity": 12, "constitution": 13,
            "intelligence": 10, "wisdom": 8, "charisma": 15
        })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "dnd5e", &slots);

    assert_eq!(
        find(&values, "strength").and_then(|v| v.value.as_integer()),
        Some(14)
    );
    assert!(
        values.iter().all(|v| v.origin == Origin::Stored),
        "5e derives nothing yet — that is T051, not a failure here"
    );
}
