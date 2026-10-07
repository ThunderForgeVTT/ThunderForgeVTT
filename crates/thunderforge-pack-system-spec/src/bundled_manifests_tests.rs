//! The manifests this repository ships, held to the validation an installed
//! pack must pass.
//!
//! Until spec 066 none of them did. The schema required four keys in the
//! shape of another product's module manifest, the bundled packs were served
//! off disk without it, and the two were never compared — so the contract an
//! author reads and the check an upload meets had drifted apart with nothing
//! to say so.

use std::path::PathBuf;

use crate::{SystemManifest, validate_system_manifest};

fn bundled_manifests() -> Vec<(String, String)> {
    let systems = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs/systems");
    let mut found: Vec<(String, String)> = std::fs::read_dir(&systems)
        .expect("packs/systems must exist")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let manifest = std::fs::read_to_string(entry.path().join("system.json"))
                .unwrap_or_else(|e| panic!("{id} has no readable system.json: {e}"));
            (id, manifest)
        })
        .collect();
    found.sort();
    found
}

/// Every bundled manifest, the template included, is one the install path
/// would accept.
#[test]
fn every_bundled_manifest_passes_the_validation_an_installed_one_must() {
    let manifests = bundled_manifests();
    assert!(
        manifests.len() >= 2,
        "found {} manifests; the walk is looking in the wrong place",
        manifests.len()
    );
    for (id, manifest) in manifests {
        if let Err(problems) = validate_system_manifest(&manifest) {
            panic!("{id}/system.json is refused:\n{problems}");
        }
        let parsed: SystemManifest = serde_json::from_str(&manifest)
            .unwrap_or_else(|e| panic!("{id}/system.json validates but does not parse: {e}"));
        assert_eq!(parsed.id, id, "a pack's id is its directory name");
    }
}

/// No bundled manifest carries a key nothing reads (spec 066, FR-008).
#[test]
fn no_bundled_manifest_declares_a_key_nothing_reads() {
    for (id, manifest) in bundled_manifests() {
        let value: serde_json::Value = serde_json::from_str(&manifest).expect("parsed above");
        for key in ["esmodules", "styles", "packages", "packs", "authors"] {
            assert!(
                value.get(key).is_none(),
                "{id}/system.json declares `{key}`, which nothing reads"
            );
        }
    }
}

fn minimal(extra: serde_json::Value) -> String {
    let mut manifest = serde_json::json!({
        "id": "plain",
        "title": "Plain",
        "version": "0.1.0",
        "compatibility": { "minimum": "0.1.0" },
        "legal": { "licenseName": "CC0", "attributionText": "Nobody." }
    });
    manifest
        .as_object_mut()
        .expect("an object")
        .extend(extra.as_object().expect("an object").clone());
    manifest.to_string()
}

/// The contract's required keys are enough.
#[test]
fn a_manifest_with_only_what_the_contract_requires_validates() {
    let manifest = minimal(serde_json::json!({}));
    assert_eq!(validate_system_manifest(&manifest), Ok(()));
    let parsed: SystemManifest = serde_json::from_str(&manifest).expect("parses");
    assert!(parsed.authors.is_empty());
    assert!(parsed.esmodules.is_empty());
    assert!(parsed.styles.is_empty());
    assert!(parsed.packs.is_empty());
}

/// A manifest written against the old schema still installs.
#[test]
fn a_manifest_that_still_carries_the_old_keys_is_accepted() {
    let manifest = minimal(serde_json::json!({
        "authors": [{ "name": "Somebody" }],
        "esmodules": ["module/main.mjs"],
        "styles": ["styles/main.css"],
        "packs": [],
        "packages": { "server": "./server" }
    }));
    assert_eq!(validate_system_manifest(&manifest), Ok(()));
}
