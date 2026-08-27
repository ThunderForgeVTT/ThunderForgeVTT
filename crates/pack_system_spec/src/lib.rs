use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub minimum: String,
    pub verified: Option<String>,
    pub maximum: Option<String>,
}

/// Spec 016 (FR-001, contracts/manifest-legal-schema.md): a system pack's
/// required, structured legal/attribution metadata. Required as a
/// non-`Option` field on `SystemManifest` below, so schemars marks it
/// `required` in the generated JSON Schema and a manifest omitting it
/// fails `validate_system_manifest` on structural grounds alone; the two
/// required sub-fields' non-emptiness is checked explicitly in
/// `validate_system_manifest` (schemars' derived schema enforces presence
/// and type, not string content).
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManifestLegal {
    pub license_name: String,
    pub attribution_text: String,
    #[serde(default)]
    pub required_notice: Option<String>,
    #[serde(default)]
    pub disclaimer: Option<String>,
    #[serde(default)]
    pub trademark_restrictions: Vec<String>,
    #[serde(default)]
    pub required_ui_placement: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManifest {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub authors: Vec<Author>,
    pub version: String,
    pub compatibility: Compatibility,
    pub esmodules: Vec<String>,
    pub styles: Vec<String>,
    pub packs: Vec<String>,
    pub media: Option<HashMap<String, String>>,
    pub manifest: Option<String>,
    pub download: Option<String>,
    pub legal: SystemManifestLegal,
}

/// Generates the JSON schema for the `SystemManifest` struct.
pub fn get_system_manifest_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(SystemManifest)).expect("Failed to serialize schema")
}

pub fn validate_system_manifest(json_string: &str) -> Result<(), String> {
    // 1. Generate Schemars RootSchema
    let root_schema = schemars::schema_for!(SystemManifest);

    // 2. Convert RootSchema → serde_json::Value
    let schema_json = serde_json::to_value(&root_schema)
        .map_err(|e| format!("Failed to serialize schema: {}", e))?;

    // 3. Compile validator
    let compiled = jsonschema::validator_for(&schema_json)
        .map_err(|e| format!("Failed to compile schema: {}", e))?;

    // 4. Parse input JSON
    let instance: serde_json::Value =
        serde_json::from_str(json_string).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // 5. Fast path
    if !compiled.is_valid(&instance) {
        // 6. Detailed errors
        let mut errors = Vec::new();
        for err in compiled.iter_errors(&instance) {
            errors.push(err.to_string());
        }
        return Err(errors.join("\n"));
    }

    // Spec 016 (FR-007): the schema above already requires `legal` to be
    // present with string-typed `licenseName`/`attributionText`, but
    // schemars' derived schema doesn't enforce non-empty string content —
    // a manifest with `"legal": {"licenseName": "", "attributionText": ""}`
    // would otherwise pass. Checked explicitly here so this one function
    // stays the single "is this manifest compliant" entry point.
    validate_legal_content(&instance)
}

/// Spec 016 (FR-007, data-model.md's validation rules): rejects a `legal`
/// object whose required `licenseName`/`attributionText` are empty or
/// whitespace-only, even though the JSON Schema above already guarantees
/// they're present and string-typed. `pub` (not just used internally by
/// `validate_system_manifest`) because `src/server/src/systems.rs`'s
/// `get_system_manifest` handler serves bundled packs' `system.json`
/// straight off disk as untyped JSON — it never runs the full
/// `SystemManifest` schema (bundled packs like `dnd5e` don't conform to
/// that schema's `authors`/`packs` shape, which was designed for the
/// admin-upload/install flow), but still needs to enforce the `legal`
/// requirement on the path that actually delivers manifests to a GM.
pub fn validate_legal_content(instance: &serde_json::Value) -> Result<(), String> {
    let legal = instance
        .get("legal")
        .ok_or_else(|| "legal: manifest is missing the required `legal` object".to_string())?;

    let license_name = legal
        .get("licenseName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if license_name.trim().is_empty() {
        return Err("legal.licenseName: must not be empty".to_string());
    }

    let attribution_text = legal
        .get("attributionText")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if attribution_text.trim().is_empty() {
        return Err("legal.attributionText: must not be empty".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_manifest() {
        let manifest_json = r#"{
            "id": "basic-game-system",
            "title": "Basic Game System",
            "description": "A minimal game system for ThunderForge VTT.",
            "authors": [
                {
                    "name": "ThunderForge Team",
                    "email": "contact@thunderforge.com"
                }
            ],
            "version": "0.1.0",
            "compatibility": {
                "minimum": "0.1.0",
                "verified": "0.1.0"
            },
            "esmodules": [
                "module/main.mjs"
            ],
            "styles": [
                "styles/main.css"
            ],
            "packs": [],
            "manifest": "https://example.com/system.json",
            "download": "https://example.com/system.zip",
            "legal": {
                "licenseName": "CC-BY-4.0",
                "attributionText": "Built from an open reference document."
            }
        }"#;
        assert!(validate_system_manifest(manifest_json).is_ok());
    }

    /// Spec 016 (FR-007, SC-003): a manifest with no `legal` object at all
    /// must fail validation, not silently load without attribution.
    #[test]
    fn test_invalid_manifest_missing_legal() {
        let manifest_json = r#"{
            "id": "basic-game-system",
            "title": "Basic Game System",
            "authors": [],
            "version": "0.1.0",
            "compatibility": { "minimum": "0.1.0" },
            "esmodules": [],
            "styles": [],
            "packs": []
        }"#;
        let result = validate_system_manifest(manifest_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("legal"));
    }

    /// Spec 016 (FR-007, data-model.md): `legal` present but with an empty
    /// `attributionText` must still fail — schemars' derived schema only
    /// guarantees the field is a present string, not a non-empty one.
    #[test]
    fn test_invalid_manifest_empty_legal_attribution_text() {
        let manifest_json = r#"{
            "id": "basic-game-system",
            "title": "Basic Game System",
            "authors": [],
            "version": "0.1.0",
            "compatibility": { "minimum": "0.1.0" },
            "esmodules": [],
            "styles": [],
            "packs": [],
            "legal": {
                "licenseName": "CC-BY-4.0",
                "attributionText": ""
            }
        }"#;
        let result = validate_system_manifest(manifest_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("attributionText"));
    }

    /// Spec 016 (SC-001): a fully-populated `legal` object (all optional
    /// fields present, matching the shape a real license like Cypher
    /// System's would need) validates cleanly.
    #[test]
    fn test_valid_manifest_with_full_legal_object() {
        let manifest_json = r#"{
            "id": "cypher-system",
            "title": "Cypher System",
            "authors": [],
            "version": "0.1.0",
            "compatibility": { "minimum": "0.1.0" },
            "esmodules": [],
            "styles": [],
            "packs": [],
            "legal": {
                "licenseName": "Cypher System Open License",
                "attributionText": "Compatible with the Cypher System.",
                "requiredNotice": "Compatible with the Cypher System",
                "disclaimer": "Not affiliated with Monte Cook Games.",
                "trademarkRestrictions": ["Do not use the Cypher System logo."],
                "requiredUiPlacement": "Must appear on the system-selection screen.",
                "sourceUrl": "https://csol.montecookgames.com/"
            }
        }"#;
        assert!(validate_system_manifest(manifest_json).is_ok());
    }

    #[test]
    fn test_invalid_manifest_missing_id() {
        let manifest_json = r#"{
            "title": "Basic Game System",
            "description": "A minimal game system for ThunderForge VTT.",
            "authors": [
                {
                    "name": "ThunderForge Team"
                }
            ],
            "version": "0.1.0",
            "compatibility": {
                "minimum": "0.1.0"
            },
            "esmodules": [],
            "styles": [],
            "packs": []
        }"#;
        assert!(validate_system_manifest(manifest_json).is_err());
    }

    #[test]
    fn test_invalid_manifest_bad_compatibility() {
        let manifest_json = r#"{
            "id": "basic-game-system",
            "title": "Basic Game System",
            "authors": [],
            "version": "0.1.0",
            "compatibility": {
                "minimum": 123
            },
            "esmodules": [],
            "styles": [],
            "packs": []
        }"#;
        assert!(validate_system_manifest(manifest_json).is_err());
    }

    #[test]
    fn test_get_schema() {
        // Asserted against the parsed schema, not against its serialised
        // text. The previous version searched the *pretty-printed* JSON for
        // compact substrings (`"title":"SystemManifest"`, no space), which
        // cannot match at any indentation — so it was testing the formatter
        // rather than the schema, and a dependency upgrade that changed the
        // spacing turned it red while the schema itself was correct.
        let schema = get_system_manifest_schema();
        let schema_json: serde_json::Value =
            serde_json::to_value(&schema).expect("the schema should serialise");

        assert_eq!(
            schema_json.get("title").and_then(|t| t.as_str()),
            Some("SystemManifest"),
            "the schema must name the type it describes"
        );
        assert!(
            schema_json
                .get("properties")
                .and_then(|p| p.get("id"))
                .is_some(),
            "a manifest is addressed by `id`, so the schema must declare it"
        );
    }
}
