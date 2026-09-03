use super::*;
use serde_json::json;

fn packs() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_system_that_counts_rounds_says_what_it_calls_them() {
    let structure = from_manifest(&json!({
        "turnStructure": { "rounds": true, "roundLabel": "Round" }
    }));
    assert_eq!(structure.round_label.as_deref(), Some("Round"));
}

#[test]
fn a_system_with_its_own_name_for_a_round_keeps_it() {
    // Fate's conflicts proceed in exchanges. Hardcoding "Round" here would be
    // the product telling a ruleset what its own vocabulary is.
    let structure = from_manifest(&json!({
        "turnStructure": { "rounds": true, "roundLabel": "Exchange" }
    }));
    assert_eq!(structure.round_label.as_deref(), Some("Exchange"));
}

#[test]
fn a_system_that_counts_rounds_without_naming_them_gets_the_ordinary_word() {
    let structure = from_manifest(&json!({ "turnStructure": { "rounds": true } }));
    assert_eq!(structure.round_label.as_deref(), Some("Round"));
}

#[test]
fn a_system_that_declines_rounds_has_no_label_to_show() {
    let structure = from_manifest(&json!({ "turnStructure": { "rounds": false } }));
    assert_eq!(structure.round_label, None);
}

/// FR-031: structure must not be *imposed*. A system that has not said it
/// counts rounds has not asked for a counter.
#[test]
fn a_system_that_says_nothing_is_not_given_rounds() {
    assert_eq!(from_manifest(&json!({})).round_label, None);
    assert_eq!(
        from_manifest(&json!({ "turnStructure": {} })).round_label,
        None
    );
}

#[test]
fn an_empty_label_falls_back_rather_than_rendering_a_bare_number() {
    for label in ["", "   "] {
        let structure = from_manifest(&json!({
            "turnStructure": { "rounds": true, "roundLabel": label }
        }));
        assert_eq!(structure.round_label.as_deref(), Some("Round"));
    }
}

#[test]
fn a_system_this_build_does_not_have_is_not_given_rounds() {
    assert_eq!(for_system(&packs(), "no-such-system").round_label, None);
}

/// SC-011, against the shipping manifests rather than a fixture.
///
/// Blades in the Dark is the case the requirement is written for: its research
/// digest records "no strict turn order or initiative; the fiction determines
/// who acts". If it ever gains a round counter, this fails.
#[test]
fn the_bundled_systems_declare_what_their_own_rules_say() {
    let dir = packs();

    for system in ["dnd5e", "pathfinder2e", "cypher_system", "year_zero_engine"] {
        assert!(
            for_system(&dir, system).round_label.is_some(),
            "{system} plays in rounds and must declare so"
        );
    }

    assert_eq!(
        for_system(&dir, "fate_core").round_label.as_deref(),
        Some("Exchange"),
        "Fate counts exchanges, not rounds"
    );

    assert_eq!(
        for_system(&dir, "blades_in_the_dark").round_label,
        None,
        "Blades has no turn order at all — this is SC-011's case"
    );
}
