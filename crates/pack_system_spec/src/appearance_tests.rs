//! Spec 044 T039: the `appearance` block's install-time checks.

use super::validate_appearance_content;
use crate::validate_system_manifest;
use serde_json::{Value, json};

fn manifest(extra: Value) -> String {
    let mut base = json!({
        "id": "t", "title": "T", "authors": [], "version": "1.0.0",
        "compatibility": { "minimum": "1.0.0" },
        "esmodules": [], "styles": [], "packs": [],
        "legal": { "licenseName": "n", "attributionText": "a" },
        "data_types": {
            "trait_data": { "properties": { "race": { "type": "string" } } }
        },
    });
    for (key, value) in extra.as_object().expect("an object") {
        base[key] = value.clone();
    }
    base.to_string()
}

fn race(slot: &str, field: &str) -> Value {
    json!({ "appearance": { "race": { "source": { "slot": slot, "field": field } } } })
}

#[test]
fn a_manifest_with_no_appearance_block_is_accepted() {
    assert!(validate_system_manifest(&manifest(json!({}))).is_ok());
    assert!(validate_system_manifest(&manifest(json!({ "appearance": {} }))).is_ok());
    assert!(validate_system_manifest(&manifest(json!({ "appearance": null }))).is_ok());
}

#[test]
fn a_declared_race_field_is_accepted() {
    validate_system_manifest(&manifest(race("traitData", "race"))).expect("accepted");
}

#[test]
fn an_undeclared_slot_is_refused_with_its_path() {
    let error =
        validate_system_manifest(&manifest(race("lineageData", "race"))).expect_err("refused");
    assert!(error.contains("appearance.race.source"), "{error}");
    assert!(error.contains("lineageData"), "{error}");
}

#[test]
fn an_undeclared_field_is_refused_with_its_path() {
    let error =
        validate_system_manifest(&manifest(race("traitData", "ancestry"))).expect_err("refused");
    assert!(error.contains("appearance.race.source"), "{error}");
    assert!(error.contains("ancestry"), "{error}");
}

#[test]
fn an_unknown_key_inside_appearance_is_refused() {
    let unknown =
        json!({ "appearance": { "eyeColour": { "slot": "traitData", "field": "race" } } });
    assert!(validate_system_manifest(&manifest(unknown)).is_err());
    let nested = json!({ "appearance": { "race": {
        "source": { "slot": "traitData", "field": "race" }, "fallback": "human" } } });
    assert!(validate_system_manifest(&manifest(nested)).is_err());
}

fn shipped(system: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../packs/systems/{system}/system.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the manifest is readable")
}

/// Bundled manifests are served block by block rather than through the
/// upload schema (`src/server/src/systems.rs`), so they are held to the
/// block's own check, as `combat_tests.rs` holds Genie's sizes.
#[test]
fn the_shipped_manifests_pass() {
    let parsed: Value = serde_json::from_str(&shipped("dnd5e")).expect("JSON");
    validate_appearance_content(&parsed).expect("dnd5e's appearance block is usable");
    assert_eq!(
        parsed["appearance"]["race"]["source"],
        json!({ "slot": "traitData", "field": "race" }),
        "5e names where a race is written"
    );

    let parsed: Value = serde_json::from_str(&shipped("genie")).expect("JSON");
    validate_appearance_content(&parsed).expect("genie validates");
    assert!(parsed.get("appearance").is_none(), "Genie has no races");
}
