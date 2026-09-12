//! `contentPatterns` — the manifest block that says what a system's content
//! looks like in a source book (spec 049, ADR-096).
//!
//! Split out of `lib.rs` because that file had reached the repository's
//! thousand-line ceiling, and this is a self-contained block of the manifest
//! contract: the schema types a system author writes against, and the rules
//! JSON Schema cannot express because each depends on another field's value.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One kind of content and how to find it, mirroring
/// `thunderforge_canvas_core::content_patterns::Pattern`.
///
/// Duplicated rather than imported, for the same reason `SystemVision` is —
/// see its comment above. The test below keeps the field names honest.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemContentPattern {
    /// `spell`, `creature`, `feat` — the system's own word. Shared code never
    /// switches on it.
    pub kind: String,
    /// `anchored` (a block of labelled fields) or `prose` (a name and
    /// paragraphs, with no mechanics to find).
    pub shape: String,
    /// The label that unambiguously begins an entry. Anchored kinds only.
    #[serde(default)]
    pub anchor: Option<String>,
    pub name: SystemContentName,
    #[serde(default)]
    pub fields: Option<Vec<SystemContentField>>,
}

/// How an entry's name is found. Which fields apply depends on `shape`, which
/// JSON Schema cannot express, so `validate_system_manifest` checks it.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemContentName {
    /// Anchored: `before` or `after` the anchor.
    #[serde(default)]
    pub position: Option<String>,
    /// Anchored: how many lines away to look.
    #[serde(default)]
    pub within_lines: Option<u32>,
    /// Anchored: `largest`, `bold` or `nearest`.
    #[serde(default)]
    pub prefer: Option<String>,
    /// Prose: `bold` or `heading`.
    #[serde(default)]
    pub style: Option<String>,
    /// Prose: `nextName` or `nextHeading`.
    #[serde(default)]
    pub ends_at: Option<String>,
}

/// One labelled field to read out of an anchored entry.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemContentField {
    /// What the read value is called on the entry.
    pub key: String,
    /// The label to look for in the document.
    pub label: String,
    /// `integer`, `number` or `text`. A value that will not parse as its
    /// declared kind is recorded uncertain with the text as read — never
    /// coerced, never dropped.
    #[serde(rename = "as")]
    pub value_kind: String,
}

/// Spec 049 (FR-010 to FR-016): the rules JSON Schema cannot express, because
/// each of them depends on the value of another field.
///
/// These are not style preferences. Every one of them is a way to write a
/// declaration that parses and then silently reads a book wrongly, which is
/// the failure mode this whole feature is built to avoid — so a system author
/// is told at validation rather than discovering it in an import review.
pub fn validate_content_patterns(instance: &serde_json::Value) -> Result<(), String> {
    let Some(patterns) = instance.get("contentPatterns").and_then(|v| v.as_array()) else {
        // Absent is legal here. The *import* refuses a system that declares
        // nothing (FR-015); a manifest that simply does not describe book
        // content is a perfectly good manifest.
        return Ok(());
    };

    let mut seen_kinds: Vec<&str> = Vec::new();
    let mut seen_anchors: Vec<String> = Vec::new();

    for (index, pattern) in patterns.iter().enumerate() {
        let at = format!("contentPatterns[{index}]");

        let kind = pattern
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{at}.kind must be a non-empty string"))?;
        if kind.trim().is_empty() {
            return Err(format!("{at}.kind must be a non-empty string"));
        }
        if seen_kinds.contains(&kind) {
            return Err(format!(
                "{at}.kind '{kind}' is declared twice; each kind may be declared once"
            ));
        }
        seen_kinds.push(kind);

        let shape = pattern
            .get("shape")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{at}.shape must be 'anchored' or 'prose'"))?;

        let name = pattern.get("name");
        let fields = pattern.get("fields").and_then(|v| v.as_array());

        match shape {
            "anchored" => {
                let anchor = pattern
                    .get("anchor")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                    .ok_or_else(|| {
                        format!("{at}.anchor is required for an anchored kind: it is the label that begins an entry")
                    })?;

                // Two kinds sharing an anchor cannot both be right, and the
                // reader would attribute every entry to whichever it tried
                // first. Compared case-insensitively because a document's
                // capitalisation is not something a declaration should depend
                // on.
                let folded = anchor.to_lowercase();
                if seen_anchors.contains(&folded) {
                    return Err(format!(
                        "{at}.anchor '{anchor}' is already used by another kind; an anchor must identify one kind"
                    ));
                }
                seen_anchors.push(folded);

                match fields {
                    Some(list) if !list.is_empty() => {
                        let mut seen_keys: Vec<&str> = Vec::new();
                        for (f, field) in list.iter().enumerate() {
                            let fat = format!("{at}.fields[{f}]");
                            let key = field
                                .get("key")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| format!("{fat}.key must be a string"))?;
                            if seen_keys.contains(&key) {
                                return Err(format!("{fat}.key '{key}' is declared twice"));
                            }
                            seen_keys.push(key);

                            match field.get("as").and_then(|v| v.as_str()) {
                                Some("integer") | Some("number") | Some("text") => {}
                                other => {
                                    return Err(format!(
                                        "{fat}.as must be 'integer', 'number' or 'text', not {other:?}"
                                    ));
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(format!(
                            "{at}.fields must list at least one field for an anchored kind; a kind with no fields to read is a prose kind"
                        ));
                    }
                }

                let position = name
                    .and_then(|n| n.get("position"))
                    .and_then(|v| v.as_str());
                if !matches!(position, Some("before") | Some("after")) {
                    return Err(format!(
                        "{at}.name.position must be 'before' or 'after' for an anchored kind"
                    ));
                }
                if let Some(prefer) = name.and_then(|n| n.get("prefer")).and_then(|v| v.as_str()) {
                    if !matches!(prefer, "largest" | "bold" | "nearest") {
                        return Err(format!(
                            "{at}.name.prefer must be 'largest', 'bold' or 'nearest'"
                        ));
                    }
                }
            }
            "prose" => {
                // FR-001b is a promise that prose content carries no
                // mechanics. A field list that is silently ignored is a
                // promise somebody thinks they have, so it is refused.
                if fields.is_some_and(|f| !f.is_empty()) {
                    return Err(format!(
                        "{at}.fields is not allowed on a prose kind: prose has no labelled fields to read, and declaring some would promise mechanics that cannot be found"
                    ));
                }
                if pattern.get("anchor").and_then(|v| v.as_str()).is_some() {
                    return Err(format!(
                        "{at}.anchor is not allowed on a prose kind: a prose entry is found by its name, not by a label"
                    ));
                }

                let style = name.and_then(|n| n.get("style")).and_then(|v| v.as_str());
                if !matches!(style, Some("bold") | Some("heading")) {
                    return Err(format!(
                        "{at}.name.style must be 'bold' or 'heading' for a prose kind"
                    ));
                }
                let ends_at = name.and_then(|n| n.get("endsAt")).and_then(|v| v.as_str());
                if !matches!(ends_at, Some("nextName") | Some("nextHeading")) {
                    return Err(format!(
                        "{at}.name.endsAt must be 'nextName' or 'nextHeading' for a prose kind"
                    ));
                }
            }
            other => {
                return Err(format!(
                    "{at}.shape must be 'anchored' or 'prose', not '{other}'"
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod content_pattern_validation_tests {
    use super::*;
    use crate::validate_system_manifest;

    fn manifest_with(patterns: serde_json::Value) -> String {
        serde_json::json!({
            "id": "t", "title": "T", "authors": [], "version": "1.0.0",
            "compatibility": { "minimum": "1.0.0" },
            "esmodules": [], "styles": [], "packs": [],
            "legal": { "licenseName": "n", "attributionText": "a" },
            "contentPatterns": patterns,
        })
        .to_string()
    }

    fn creature() -> serde_json::Value {
        serde_json::json!({
            "kind": "creature",
            "shape": "anchored",
            "anchor": "Armor Class",
            "name": { "position": "before", "withinLines": 6, "prefer": "largest" },
            "fields": [{ "key": "armorClass", "label": "Armor Class", "as": "integer" }]
        })
    }

    #[test]
    fn a_manifest_that_declares_nothing_is_a_perfectly_good_manifest() {
        // The *import* refuses a system with no patterns (FR-015). A system
        // that simply does not describe book content is not malformed.
        let manifest = serde_json::json!({
            "id": "t", "title": "T", "authors": [], "version": "1.0.0",
            "compatibility": { "minimum": "1.0.0" },
            "esmodules": [], "styles": [], "packs": [],
            "legal": { "licenseName": "n", "attributionText": "a" },
        })
        .to_string();
        assert!(validate_system_manifest(&manifest).is_ok());
    }

    #[test]
    fn a_well_formed_anchored_kind_passes() {
        assert!(validate_system_manifest(&manifest_with(serde_json::json!([creature()]))).is_ok());
    }

    #[test]
    fn two_kinds_may_not_share_an_anchor() {
        // Whichever the reader tried first would collect every entry, and the
        // other kind would silently come out empty.
        let mut other = creature();
        other["kind"] = "npc".into();
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([creature(), other])))
                .unwrap_err();
        assert!(error.contains("already used by another kind"), "{error}");
    }

    #[test]
    fn an_anchor_collides_regardless_of_capitalisation() {
        // A document's capitalisation is not something a declaration should
        // be allowed to depend on for uniqueness.
        let mut other = creature();
        other["kind"] = "npc".into();
        other["anchor"] = "ARMOR CLASS".into();
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([creature(), other])))
                .unwrap_err();
        assert!(error.contains("already used by another kind"), "{error}");
    }

    #[test]
    fn a_kind_may_not_be_declared_twice() {
        let mut other = creature();
        other["anchor"] = "Hit Points".into();
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([creature(), other])))
                .unwrap_err();
        assert!(error.contains("declared twice"), "{error}");
    }

    #[test]
    fn an_anchored_kind_without_an_anchor_is_refused() {
        let mut broken = creature();
        broken["anchor"] = serde_json::Value::Null;
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("anchor is required"), "{error}");
    }

    #[test]
    fn an_anchored_kind_with_no_fields_is_refused() {
        let mut broken = creature();
        broken["fields"] = serde_json::json!([]);
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("at least one field"), "{error}");
    }

    #[test]
    fn a_field_of_an_unknown_type_is_refused() {
        let mut broken = creature();
        broken["fields"] = serde_json::json!([{ "key": "ac", "label": "AC", "as": "dice" }]);
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("'integer', 'number' or 'text'"), "{error}");
    }

    #[test]
    fn prose_may_not_declare_fields() {
        // FR-001b promises prose carries no mechanics. A silently ignored
        // field list is a promise somebody thinks they have.
        let broken = serde_json::json!({
            "kind": "feat",
            "shape": "prose",
            "name": { "style": "heading", "endsAt": "nextName" },
            "fields": [{ "key": "x", "label": "X", "as": "text" }]
        });
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("not allowed on a prose kind"), "{error}");
    }

    #[test]
    fn prose_needs_a_style_and_an_end() {
        let broken = serde_json::json!({
            "kind": "feat",
            "shape": "prose",
            "name": { "style": "heading" }
        });
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("endsAt"), "{error}");
    }

    #[test]
    fn an_unknown_shape_is_refused() {
        let mut broken = creature();
        broken["shape"] = "tabular".into();
        let error =
            validate_system_manifest(&manifest_with(serde_json::json!([broken]))).unwrap_err();
        assert!(error.contains("'anchored' or 'prose'"), "{error}");
    }

    /// The shipped 5e manifest's content patterns validate.
    ///
    /// This is the test that stops the schema and the real file drifting
    /// apart: the manifest is JSON somebody edits by hand, and a rule added
    /// here that the shipped pack violates should fail *here*, not when
    /// somebody tries to import a book.
    ///
    /// It checks `validate_content_patterns` rather than the whole
    /// `validate_system_manifest`, because the shipped manifest **does not
    /// pass the whole one** — it carries `author` and `packages` where the
    /// published schema requires `authors` and `packs`. That mismatch
    /// predates this feature by a long way and says the schema has never been
    /// run against a real pack; widening this test to cover it would fail for
    /// a reason spec 049 did not cause and cannot fix here.
    #[test]
    fn the_shipped_dnd5e_content_patterns_validate() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/systems/dnd5e/system.json"
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("readable"))
                .expect("the 5e manifest is JSON");
        validate_content_patterns(&manifest).expect("the shipped 5e content patterns validate");
    }

    /// The five kinds spec 049 FR-013 requires, and the shapes real books
    /// actually have.
    #[test]
    fn the_shipped_dnd5e_manifest_declares_what_fr_013_asks_for() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/systems/dnd5e/system.json"
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let patterns = manifest["contentPatterns"].as_array().expect("declared");

        let shape_of = |kind: &str| -> String {
            patterns
                .iter()
                .find(|p| p["kind"] == kind)
                .unwrap_or_else(|| panic!("5e declares a '{kind}' pattern"))["shape"]
                .as_str()
                .unwrap()
                .to_string()
        };

        // Anchored, because the label is unambiguous and appears nowhere
        // else. Armour class is why 451 creatures read out of one directory.
        assert_eq!(shape_of("creature"), "anchored");
        assert_eq!(shape_of("spell"), "anchored");

        // Prose, and this is a correction to spec 049's prose rather than an
        // implementation shortcut. Measured against real books on 2026-09-12:
        // a magic item is a heading name followed by an *unlabelled* type and
        // rarity line ("Weapon (whip), uncommon"), and a feat is a heading
        // name followed by a prerequisite line. Neither has an anchor label,
        // so neither can be read as anchored content.
        assert_eq!(shape_of("magicItem"), "prose");
        assert_eq!(shape_of("feat"), "prose");
        assert_eq!(shape_of("classFeature"), "prose");
    }
}
