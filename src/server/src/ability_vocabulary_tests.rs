//! Spec 033, Increment A — what a system calls its abilities.
//!
//! In a sibling file rather than an inline `#[cfg(test)]` module because
//! `scripts/check-system-registry.mjs` strips inline test modules but excludes
//! `*_tests.rs` wholesale, and these tests have to name systems and types to
//! assert anything about them.

use super::*;
use serde_json::json;

fn none() -> Vec<String> {
    Vec::new()
}

fn held(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

fn ids(vocabulary: &AbilityVocabulary) -> Vec<&str> {
    vocabulary.types.iter().map(|k| k.id.as_str()).collect()
}

/// A system that says nothing gets a complete, correctly-labelled tab set.
///
/// SC-013's case, and the one most likely to break under a change only ever
/// tested against a system that declares plenty.
#[test]
fn a_system_declaring_nothing_yields_every_builtin_correctly_labelled() {
    let vocabulary = from_manifest(&json!({}), &held(&["spell", "feat", "power", "talent"]));

    assert_eq!(ids(&vocabulary), vec!["spell", "feat", "power", "talent"]);
    assert_eq!(vocabulary.umbrella.label, "Ability");
    assert_eq!(vocabulary.umbrella.plural_label, "Abilities");
    assert!(vocabulary.types.iter().all(|k| !k.label.is_empty()));
    assert!(vocabulary.types.iter().all(|k| !k.plural_label.is_empty()));
}

/// An absent manifest is not an error. A world whose pack is missing still has
/// abilities, and they still have names.
#[test]
fn an_absent_manifest_still_names_everything() {
    let vocabulary = from_manifest(&serde_json::Value::Null, &held(&["spell"]));

    assert_eq!(ids(&vocabulary), vec!["spell"]);
    assert_eq!(vocabulary.get("spell").unwrap().label, "Spell");
}

/// FR-014: declaring a built-in's id re-labels it and produces one type.
#[test]
fn declaring_a_builtin_id_relabels_it_rather_than_duplicating_it() {
    let manifest = json!({
        "abilityVocabulary": {
            "types": [{ "id": "spell", "label": "Scroll", "pluralLabel": "Scrolls" }]
        }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(
        vocabulary.types.iter().filter(|k| k.id == "spell").count(),
        1,
        "a re-label must not create a second type"
    );
    let spell = vocabulary.get("spell").unwrap();
    assert_eq!(spell.label, "Scroll");
    assert_eq!(spell.plural_label, "Scrolls");
    assert!(
        spell.builtin,
        "re-labelling does not stop it being built in"
    );
}

/// The legacy block genie ships today keeps working, untouched.
#[test]
fn the_legacy_ability_facets_block_still_relabels() {
    let manifest = json!({
        "abilityFacets": {
            "spell": { "label": "Scroll", "pluralLabel": "Scrolls" },
            "talent": { "label": "Knack", "pluralLabel": "Knacks" }
        }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(vocabulary.get("spell").unwrap().plural_label, "Scrolls");
    assert_eq!(vocabulary.get("talent").unwrap().plural_label, "Knacks");
}

/// Where both blocks speak, the newer one wins; the older fills the gaps.
#[test]
fn the_newer_block_wins_and_the_older_fills_in() {
    let manifest = json!({
        "abilityVocabulary": {
            "types": [{ "id": "spell", "label": "Incantation", "pluralLabel": "Incantations" }]
        },
        "abilityFacets": {
            "spell": { "label": "Scroll", "pluralLabel": "Scrolls" },
            "talent": { "label": "Knack", "pluralLabel": "Knacks" }
        }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(vocabulary.get("spell").unwrap().label, "Incantation");
    assert_eq!(vocabulary.get("talent").unwrap().label, "Knack");
}

/// FR-016: a malformed entry loses only itself.
#[test]
fn a_malformed_entry_does_not_take_the_rest_of_the_vocabulary_with_it() {
    let manifest = json!({
        "abilityVocabulary": {
            "types": [
                "not an object",
                { "noId": true },
                { "id": "", "label": "Blank" },
                { "id": "enchantment", "label": "Enchantment", "pluralLabel": "Enchantments" }
            ]
        }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert!(vocabulary.recognises("enchantment"));
    assert_eq!(vocabulary.get("enchantment").unwrap().label, "Enchantment");
}

/// FR-016: no declaration can produce a blank label.
#[test]
fn a_missing_or_empty_label_falls_back_to_the_id_never_to_blank() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "hex" },
            { "id": "ward", "label": "   " }
        ]}
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(vocabulary.get("hex").unwrap().label, "hex");
    assert_eq!(vocabulary.get("hex").unwrap().plural_label, "hex");
    assert_eq!(vocabulary.get("ward").unwrap().label, "ward");
    assert!(vocabulary.types.iter().all(|k| !k.label.trim().is_empty()));
}

/// FR-003: the umbrella term replaces the application's word.
#[test]
fn a_declared_umbrella_replaces_the_default_word() {
    let manifest = json!({
        "abilityVocabulary": { "umbrella": { "label": "Spell", "pluralLabel": "Spells" } }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(vocabulary.umbrella.label, "Spell");
    assert_eq!(vocabulary.umbrella.plural_label, "Spells");
}

/// An umbrella with only a singular still reads correctly in both places.
#[test]
fn an_umbrella_without_a_plural_falls_back_to_its_singular() {
    let manifest = json!({ "abilityVocabulary": { "umbrella": { "label": "Art" } } });

    assert_eq!(
        from_manifest(&manifest, &none()).umbrella.plural_label,
        "Art"
    );
}

// ---------------------------------------------------------------------------
// FR-011a — presence follows use
// ---------------------------------------------------------------------------

/// The rule in all four combinations, which is the only way to be sure it is
/// a rule rather than two coincidences.
#[test]
fn a_builtin_is_present_when_the_system_uses_it_or_the_world_holds_one() {
    let declares_spell = json!({
        "abilityVocabulary": { "types": [{ "id": "spell", "label": "Scroll" }] }
    });

    // Declared and held.
    assert!(from_manifest(&declares_spell, &held(&["spell"])).recognises("spell"));
    // Declared, not held — a system that uses the type keeps its tab.
    assert!(from_manifest(&declares_spell, &none()).recognises("spell"));
    // Not declared, but held — content is never hidden.
    assert!(from_manifest(&json!({}), &held(&["talent"])).recognises("talent"));
    // Neither: no tab. This is the case that stops a 5e world carrying empty
    // "Powers" and "Talents" forever.
    assert!(!from_manifest(&declares_spell, &none()).recognises("power"));
}

/// The whole point of the rule, stated as the scenario that motivated it.
#[test]
fn a_system_that_uses_two_builtins_does_not_carry_the_other_two() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "spell", "label": "Spell", "pluralLabel": "Spells", "order": 0 },
            { "id": "feat", "label": "Feat", "pluralLabel": "Feats", "order": 1 },
            { "id": "enchantment", "label": "Enchantment", "pluralLabel": "Enchantments", "order": 2 }
        ]}
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(ids(&vocabulary), vec!["spell", "feat", "enchantment"]);
    assert!(!vocabulary.recognises("power"));
    assert!(!vocabulary.recognises("talent"));
}

/// Holding one is enough. No ability can be hidden by the presence rule.
#[test]
fn holding_an_ability_of_an_unused_builtin_brings_its_tab_back() {
    let manifest = json!({
        "abilityVocabulary": { "types": [{ "id": "spell", "label": "Spell" }] }
    });

    assert!(!from_manifest(&manifest, &none()).recognises("power"));
    assert!(from_manifest(&manifest, &held(&["power"])).recognises("power"));
}

/// A system's own type is never dropped for want of content — it is what the
/// system says it has, and an empty tab a GM can fill is not clutter.
#[test]
fn a_declared_type_is_present_even_with_nothing_in_it() {
    let manifest = json!({
        "abilityVocabulary": { "types": [{ "id": "enchantment", "label": "Enchantment" }] }
    });

    assert!(from_manifest(&manifest, &none()).recognises("enchantment"));
}

// ---------------------------------------------------------------------------
// Ordering, facets
// ---------------------------------------------------------------------------

/// FR-004: the system's declared order is honoured, not our alphabet.
#[test]
fn the_declared_order_is_honoured() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "enchantment", "label": "Enchantment", "order": 0 },
            { "id": "spell", "label": "Spell", "order": 1 },
            { "id": "feat", "label": "Feat", "order": 2 }
        ]}
    });

    assert_eq!(
        ids(&from_manifest(&manifest, &none())),
        vec!["enchantment", "spell", "feat"]
    );
}

/// FR-018 as clarified: exactly one binding, defaulting to character.
#[test]
fn a_type_binds_to_exactly_one_subject_and_defaults_to_character() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "enchantment", "label": "Enchantment", "binds": "item" },
            { "id": "aura", "label": "Aura", "binds": "nothing" },
            { "id": "hex", "label": "Hex" }
        ]}
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert_eq!(vocabulary.get("enchantment").unwrap().binds, Binds::Item);
    assert_eq!(vocabulary.get("aura").unwrap().binds, Binds::Nothing);
    assert_eq!(vocabulary.get("hex").unwrap().binds, Binds::Character);
}

/// An unreadable `binds` is `character` rather than an error — the total rule
/// again, applied to a facet.
#[test]
fn an_unrecognised_binds_value_falls_back_rather_than_dropping_the_type() {
    let manifest = json!({
        "abilityVocabulary": { "types": [{ "id": "hex", "label": "Hex", "binds": "the moon" }] }
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert!(vocabulary.recognises("hex"));
    assert_eq!(vocabulary.get("hex").unwrap().binds, Binds::Character);
}

/// FR-021: a grade is the system's word plus a range.
#[test]
fn a_declared_grade_carries_the_systems_word_and_its_range() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "spell", "label": "Spell", "grade": { "label": "Level", "min": 0, "max": 9 } }
        ]}
    });

    let grade = from_manifest(&manifest, &none())
        .get("spell")
        .unwrap()
        .grade
        .clone()
        .expect("a declared grade must survive assembly");

    assert_eq!(grade.label, "Level");
    assert_eq!((grade.min, grade.max), (0, 9));
}

/// A type with no grade shows none anywhere (FR-022), so absence has to
/// survive assembly as absence rather than as a zero range.
#[test]
fn a_type_without_a_grade_has_none() {
    let manifest = json!({
        "abilityVocabulary": { "types": [{ "id": "feat", "label": "Feat" }] }
    });

    assert!(
        from_manifest(&manifest, &none())
            .get("feat")
            .unwrap()
            .grade
            .is_none()
    );
}

/// A grade that cannot contain a value leaves the type ungraded rather than
/// refusing everything a GM types into it.
#[test]
fn an_impossible_grade_range_is_ignored_rather_than_enforced() {
    let manifest = json!({
        "abilityVocabulary": { "types": [
            { "id": "hex", "label": "Hex", "grade": { "label": "Rank", "min": 9, "max": 0 } }
        ]}
    });

    let vocabulary = from_manifest(&manifest, &none());

    assert!(vocabulary.recognises("hex"));
    assert!(vocabulary.get("hex").unwrap().grade.is_none());
}

/// `recognises` is the question FR-013 and FR-034 both turn on.
#[test]
fn an_undeclared_identity_is_not_recognised() {
    let vocabulary = from_manifest(&json!({}), &held(&["spell"]));

    assert!(vocabulary.recognises("spell"));
    assert!(!vocabulary.recognises("enchantment"));
    assert!(vocabulary.get("enchantment").is_none());
}
