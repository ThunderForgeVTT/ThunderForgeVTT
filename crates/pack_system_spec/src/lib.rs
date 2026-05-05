use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json;
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
    if compiled.is_valid(&instance) {
        return Ok(());
    }

    // 6. Detailed errors
    let mut errors = Vec::new();
    for err in compiled.iter_errors(&instance) {
        errors.push(err.to_string());
    }

    Err(errors.join("\n"))
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
            "download": "https://example.com/system.zip"
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
        let schema = get_system_manifest_schema();
        let schema_json = serde_json::to_string_pretty(&schema).unwrap();
        // You can print the schema to see it
        // println!("{}", schema_json);
        assert!(schema_json.contains(r#""title":"SystemManifest""#));
        assert!(schema_json.contains(r#""properties":{"id":"#));
    }
}
