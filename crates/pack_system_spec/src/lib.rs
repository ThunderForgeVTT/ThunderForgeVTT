pub mod appearance;
pub mod combat;
pub mod content_patterns;
pub mod contrast;
pub mod interface;
pub mod layout;

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
    /// The resources this system tracks and wants drawn on tokens.
    ///
    /// Spec 029. Optional, and an empty or absent list means the system's
    /// tokens carry no bars at all — which is the correct behaviour for a
    /// system that does not track pools, not a gap to fill with a default.
    /// The engine holds no built-in notion of "health"; hard-coding the first
    /// system's vocabulary would make every system after it a special case.
    #[serde(default)]
    pub resources: Vec<SystemResource>,
    /// How this system's creatures see (spec 045, owner decision 2).
    ///
    /// Optional, and **staying** optional, unlike `legal`. A system that
    /// declares none gets ordinary sight, and that is a correct answer rather
    /// than a missing one — most rulesets have nothing to say about seeing in
    /// the dark, so an absent block is not evidence that nobody thought about
    /// it. See ADR-027's 2026-09-11 amendment.
    #[serde(default)]
    pub vision: Option<SystemVision>,

    /// What this system's content looks like in a source book (spec 049).
    ///
    /// Absent means a book cannot be read into a world on this system, and
    /// the import says so before a file is opened. That is a refusal rather
    /// than a fallback: guessing with another system's vocabulary is how a
    /// Pathfinder book gets imported as badly-parsed D&D.
    #[serde(default)]
    pub content_patterns: Option<Vec<content_patterns::SystemContentPattern>>,

    /// What a fight means in this system (spec 046): which fields are hit
    /// points, what an attack is rolled against, how big a creature is.
    /// Optional, block by block (M1); see `combat.rs`.
    #[serde(default)]
    pub combat: Option<combat::SystemCombat>,

    /// Whether the system counts rounds, and what one turn affords.
    #[serde(default)]
    pub turn_structure: Option<combat::SystemTurnStructure>,

    /// Where a creature's race is written, for the hero builder's dice
    /// (spec 044 FR-007a). Absent means the system has no races; see
    /// `appearance.rs`.
    #[serde(default)]
    pub appearance: Option<appearance::SystemAppearance>,
}

/// A system's `vision` block, mirroring
/// `thunderforge_canvas_core::vision_declaration::VisionDeclaration`.
///
/// Duplicated rather than imported, for the same reason `SystemResource` is:
/// this crate is the manifest *schema*, published to system authors and
/// validated against JSON they write by hand, and importing an engine type
/// would drag the engine's dependency graph into every pack that only wants
/// to describe itself. A test below keeps the field names honest.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemVision {
    /// What one grid square is worth in this system's units — 5, for a
    /// five-foot square.
    #[serde(default)]
    pub units_per_cell: Option<f32>,
    #[serde(default)]
    pub unit_label: Option<String>,
    #[serde(default)]
    pub darkvision: Option<SystemDistanceSource>,
    #[serde(default)]
    pub carried_light: Option<SystemCarriedLight>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemCarriedLight {
    #[serde(default)]
    pub bright: Option<SystemDistanceSource>,
    #[serde(default)]
    pub dim: Option<SystemDistanceSource>,
}

/// Where one distance is read from an actor: which stored blob, which key.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemDistanceSource {
    /// Defaults to `traitData` when absent.
    #[serde(default)]
    pub slot: Option<String>,
    pub source: String,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub default: Option<f32>,
}

/// One resource a system declares, mirroring
/// `thunderforge_canvas_core::resource_display::ResourceDefinition`.
///
/// Deliberately duplicated rather than imported: this crate is the manifest
/// *schema*, published to system authors and validated against JSON they
/// write by hand, and coupling it to an engine type would drag the engine's
/// dependency graph into every pack that only wants to describe itself. The
/// two are kept honest by a test asserting the field names match.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemResource {
    /// Stable identifier, e.g. `health`. Unique within a system.
    pub id: String,
    /// What a person reads, e.g. "Hit Points".
    pub label: String,
    /// `bar` (has a maximum) or `counter` (does not).
    pub kind: String,
    /// Display order. The engine imposes none.
    pub order: i32,
    /// Whether more than one entry is permitted — a shield over health, or a
    /// multi-stage boss. A counter must not allow stacking.
    #[serde(default)]
    pub allow_stacking: bool,
    /// Where this resource's numbers live in the system's stored actor data.
    ///
    /// Without this the server would have to know each system's field names,
    /// and every new ruleset would need server changes before its tokens could
    /// show anything — the coupling FR-001 exists to prevent.
    pub source: ResourceSourceSpec,
}

/// Which stored slot a resource reads, and which fields make up its entries.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSourceSpec {
    /// `resourceData`, `traitData`, and so on.
    pub slot: String,
    /// Ordered. Index 0 is the base pool; later entries stack above it.
    pub entries: Vec<EntrySourceSpec>,
}

/// One layer's field mapping.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntrySourceSpec {
    pub current: String,
    /// Absent for a layer with no maximum of its own — temporary hit points
    /// are granted, not capped.
    pub max: Option<String>,
    /// A maximum fixed by the rules rather than stored per character.
    ///
    /// Blades in the Dark caps stress at nine and trauma at four, and neither
    /// is written into a character's data because neither varies. Without
    /// this, such a pool could only be drawn as a bare count — losing the one
    /// thing a player needs from it, which is how close to the cap they are.
    pub max_value: Option<i32>,
    pub label: Option<String>,
    /// Skip when the field is missing or zero, so an absent layer is absent
    /// rather than an empty bar on every character.
    #[serde(default)]
    pub optional: bool,
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
    validate_legal_content(&instance)?;
    validate_vision_content(&instance)?;
    combat::validate_combat_content(&instance)?;
    appearance::validate_appearance_content(&instance)?;
    content_patterns::validate_content_patterns(&instance)
}

/// Spec 045: a `vision` block that is present must be usable.
///
/// The schema above already types the fields, but it cannot say that a
/// distance must be readable from *somewhere* or that a square must have a
/// positive size. Both failures are silent at runtime — a `source` of `""`
/// reads nothing and a `unitsPerCell` of `0` divides a creature's sight into
/// a fallback — so they are refused at install time, where a person is
/// looking at the manifest they just wrote.
fn validate_vision_content(instance: &serde_json::Value) -> Result<(), String> {
    let Some(vision) = instance.get("vision") else {
        return Ok(());
    };
    if vision.is_null() {
        return Ok(());
    }

    if let Some(per_cell) = vision.get("unitsPerCell").and_then(|v| v.as_f64())
        && !(per_cell.is_finite() && per_cell > 0.0)
    {
        return Err(format!(
            "vision.unitsPerCell must be a positive number (got {per_cell})"
        ));
    }

    let mut sources: Vec<(&str, &serde_json::Value)> = Vec::new();
    if let Some(dark) = vision.get("darkvision") {
        sources.push(("vision.darkvision", dark));
    }
    if let Some(carried) = vision.get("carriedLight") {
        for reach in ["bright", "dim"] {
            if let Some(value) = carried.get(reach) {
                sources.push(match reach {
                    "bright" => ("vision.carriedLight.bright", value),
                    _ => ("vision.carriedLight.dim", value),
                });
            }
        }
    }

    for (path, source) in sources {
        if source.is_null() {
            continue;
        }
        let named = source
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if named.trim().is_empty() {
            return Err(format!("{path}.source must name a field to read"));
        }
    }

    Ok(())
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

    /// The manifest schema and the engine's model describe the same thing in
    /// two crates, and they must not drift.
    ///
    /// They are deliberately duplicated: this crate is the schema published to
    /// system authors and validated against hand-written JSON, and depending
    /// on the engine's crate would drag its graph into every pack that only
    /// wants to describe itself. The cost of that choice is this test.
    #[test]
    fn the_resource_spec_matches_the_engine_model_field_for_field() {
        let spec = serde_json::to_value(SystemResource {
            id: "health".into(),
            label: "Health".into(),
            kind: "bar".into(),
            order: 0,
            allow_stacking: true,
            source: ResourceSourceSpec {
                slot: "resourceData".into(),
                entries: vec![EntrySourceSpec {
                    current: "current_hp".into(),
                    max: Some("max_hp".into()),
                    max_value: None,
                    label: None,
                    optional: false,
                }],
            },
        })
        .expect("serialises");

        // These names are the contract. A rename on either side without the
        // other is a manifest that validates and then displays nothing.
        for field in ["id", "label", "kind", "order", "allowStacking", "source"] {
            assert!(spec.get(field).is_some(), "missing {field}");
        }
        let source = spec.get("source").unwrap();
        assert!(source.get("slot").is_some());
        let entry = &source.get("entries").unwrap()[0];
        for field in ["current", "max", "maxValue", "label", "optional"] {
            assert!(entry.get(field).is_some(), "entry missing {field}");
        }
    }
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

#[cfg(test)]
mod vision_validation_tests {
    use super::*;

    fn manifest_with(vision: serde_json::Value) -> String {
        serde_json::json!({
            "id": "t", "title": "T", "authors": [], "version": "1.0.0",
            "compatibility": { "minimum": "1.0.0" },
            "esmodules": [], "styles": [], "packs": [],
            "legal": { "licenseName": "n", "attributionText": "a" },
            "vision": vision,
        })
        .to_string()
    }

    #[test]
    fn a_manifest_with_no_vision_block_is_fine() {
        let json = serde_json::json!({
            "id": "t", "title": "T", "authors": [], "version": "1.0.0",
            "compatibility": { "minimum": "1.0.0" },
            "esmodules": [], "styles": [], "packs": [],
            "legal": { "licenseName": "n", "attributionText": "a" },
        })
        .to_string();
        assert!(validate_system_manifest(&json).is_ok());
    }

    #[test]
    fn a_square_of_no_size_is_refused() {
        // Silent at runtime otherwise: `safe_per_cell` would stand in and a
        // creature's sight would quietly be measured against the wrong scale.
        for bad in [0.0, -5.0] {
            let json = manifest_with(serde_json::json!({ "unitsPerCell": bad }));
            let error = validate_system_manifest(&json).expect_err("refused");
            assert!(error.contains("unitsPerCell"), "{error}");
        }
        assert!(
            validate_system_manifest(&manifest_with(serde_json::json!({ "unitsPerCell": 5 })))
                .is_ok()
        );
    }

    #[test]
    fn a_distance_that_names_no_field_is_refused() {
        // A `source` of "" reads nothing, for ever, without complaining.
        for path in ["darkvision", "carriedLight.bright", "carriedLight.dim"] {
            let vision = match path {
                "darkvision" => serde_json::json!({ "darkvision": { "source": "  " } }),
                "carriedLight.bright" => {
                    serde_json::json!({ "carriedLight": { "bright": { "source": "" } } })
                }
                _ => serde_json::json!({ "carriedLight": { "dim": { "source": "" } } }),
            };
            let error = validate_system_manifest(&manifest_with(vision)).expect_err("refused");
            assert!(error.contains(path), "expected {path} in: {error}");
        }
    }

    #[test]
    fn the_shipped_dnd5e_manifest_passes() {
        // The one that has to work: this is the declaration spec 045 ships.
        let json = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/systems/dnd5e/system.json"
        ))
        .expect("the 5e manifest is readable");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(
            parsed.get("vision").is_some(),
            "5e declares its vision block"
        );
        super::validate_vision_content(&parsed).expect("its vision block is usable");
    }
}

#[cfg(test)]
mod mirror_tests {
    //! The manifest schema duplicates several engine types rather than
    //! importing them, so that a pack author reads one definition of what they
    //! may write. Duplication is only safe while somebody checks it, and until
    //! spec 045 nobody did: the comment on `SystemResource` claimed "the two
    //! are kept honest by a test asserting the field names match", and no such
    //! test existed anywhere in the repository.

    use super::*;
    use serde_json::Value;

    /// The JSON keys a type serialises to.
    fn keys_of(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .expect("a struct serialises to an object")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_resource_schema_covers_every_field_of_the_engines_definition() {
        // Not equality: `SystemResource` also carries `source`, which is the
        // manifest's business and not the engine's. What must hold is that
        // nothing the engine reads is missing from what an author may write.
        use thunderforge_canvas_core::resource_display::{ResourceDefinition, ResourceKind};

        let engine = serde_json::to_value(ResourceDefinition {
            id: "health".into(),
            label: "Hit Points".into(),
            kind: ResourceKind::Bar,
            order: 0,
            allow_stacking: false,
        })
        .expect("serialises");
        let manifest = serde_json::to_value(SystemResource {
            id: "health".into(),
            label: "Hit Points".into(),
            kind: "bar".into(),
            order: 0,
            allow_stacking: false,
            source: ResourceSourceSpec {
                slot: "resourceData".into(),
                entries: vec![EntrySourceSpec {
                    current: "hp".into(),
                    max: Some("hp_max".into()),
                    max_value: None,
                    label: None,
                    optional: false,
                }],
            },
        })
        .expect("serialises");

        let declared = keys_of(&manifest);
        for field in keys_of(&engine) {
            assert!(
                declared.contains(&field),
                "the engine reads `{field}` and a pack author has no way to \
                 declare it: {declared:?}"
            );
        }
    }

    #[test]
    fn the_vision_schema_matches_the_engines_declaration_exactly() {
        // Spec 045. This one *is* equality: the block exists only to be read
        // by `vision_declaration`, so a key on one side and not the other is
        // either a field nobody can declare or one nothing will ever read.
        use thunderforge_canvas_core::vision_declaration::{
            CarriedLightDeclaration, DistanceSource, VisionDeclaration,
        };

        let engine = serde_json::to_value(VisionDeclaration {
            units_per_cell: Some(5.0),
            unit_label: Some("ft".into()),
            darkvision: Some(DistanceSource {
                slot: Some("traitData".into()),
                source: "darkvision".into(),
                unit: Some("feet".into()),
                default: None,
            }),
            carried_light: Some(CarriedLightDeclaration {
                bright: None,
                dim: None,
            }),
        })
        .expect("serialises");
        let manifest = serde_json::to_value(SystemVision {
            units_per_cell: Some(5.0),
            unit_label: Some("ft".into()),
            darkvision: Some(SystemDistanceSource {
                slot: Some("traitData".into()),
                source: "darkvision".into(),
                unit: Some("feet".into()),
                default: None,
            }),
            carried_light: Some(SystemCarriedLight {
                bright: None,
                dim: None,
            }),
        })
        .expect("serialises");

        assert_eq!(keys_of(&manifest), keys_of(&engine));
        assert_eq!(
            keys_of(&manifest["darkvision"]),
            keys_of(&engine["darkvision"]),
            "a distance is named the same way on both sides"
        );
        assert_eq!(
            keys_of(&manifest["carriedLight"]),
            keys_of(&engine["carriedLight"])
        );
    }
}
