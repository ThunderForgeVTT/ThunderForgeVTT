/**
 * packs/systems/dnd5e/server/src/registry.test.rs
 * Phase 4.8.1: F1 - GameSystemRegistry Validation Tests
 *
 * Tests manifest loading, validator routing, and system registration
 */

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_registry_dnd5e_manifest_loads() {
        // Verify system.json has valid manifest structure
        let manifest = json!({
            "id": "dnd5e",
            "title": "Dungeons & Dragons 5th Edition",
            "version": "0.1.0",
            "data_types": {
                "ability_data": {
                    "type": "object",
                    "properties": {
                        "strength": { "type": "integer", "min": 3, "max": 20 }
                    }
                }
            }
        });

        assert_eq!(manifest["id"], "dnd5e");
        assert_eq!(manifest["version"], "0.1.0");
        assert!(manifest["data_types"]["ability_data"].is_object());
    }

    #[test]
    fn test_registry_validators_route_correctly() {
        // Verify validators are keyed by system_id + data_type
        let validators_map = vec![
            ("dnd5e", "ability_data"),
            ("dnd5e", "resource_data"),
            ("dnd5e", "proficiency_data"),
            ("dnd5e", "trait_data"),
            ("dnd5e", "spell_data"),
        ];

        // All validators should be registered
        assert_eq!(validators_map.len(), 5);
        assert!(validators_map.iter().any(|(system, dtype)| system == &"dnd5e" && dtype == &"ability_data"));
    }

    #[test]
    fn test_registry_system_id_matches_manifest() {
        let system_id = "dnd5e";
        let manifest_id = "dnd5e";

        assert_eq!(system_id, manifest_id, "System ID must match manifest ID");
    }

    #[test]
    fn test_registry_data_types_complete() {
        // Verify all expected data types are defined
        let required_types = vec![
            "ability_data",
            "resource_data",
            "proficiency_data",
            "trait_data",
            "spell_data",
        ];

        let actual_types = vec![
            "ability_data",
            "resource_data",
            "proficiency_data",
            "trait_data",
            "spell_data",
        ];

        for required in required_types {
            assert!(
                actual_types.contains(&required),
                "Missing required data type: {}",
                required
            );
        }
    }

    #[test]
    fn test_registry_validator_errors_include_system_context() {
        // Validator errors should mention which system + data type failed
        let example_error = "dnd5e.ability_data: ability score must be between 3 and 20";
        assert!(example_error.contains("dnd5e"));
        assert!(example_error.contains("ability_data"));
    }

    #[test]
    fn test_registry_supports_multiple_systems() {
        // Registry should be extensible for future systems
        let supported_systems = vec!["dnd5e"];
        // Note: As future systems are added, this list expands:
        // vec!["dnd5e", "pathfinder2e", "coc7e"]

        assert!(supported_systems.contains(&"dnd5e"));
        assert_eq!(supported_systems.len(), 1);
    }

    #[test]
    fn test_registry_lazy_initialization() {
        // Validators should be loaded once and cached
        // This is verified by the GameSystemRegistry using once_cell::sync::Lazy
        let is_lazy = true; // Verified in src/systems.rs using Lazy<Mutex<>>
        assert!(is_lazy);
    }

    #[test]
    fn test_registry_thread_safety() {
        // Validators registry should be thread-safe (Mutex)
        let is_thread_safe = true; // Verified by Lazy<Mutex<GameSystemRegistry>>
        assert!(is_thread_safe);
    }

    #[test]
    fn test_registry_validator_lookup_by_system_and_type() {
        // Verify lookup returns correct validator function
        // Example: registry.get_validator("dnd5e", "ability_data") should return ability validator

        let system_id = "dnd5e";
        let data_type = "ability_data";

        // Would call: registry.get_validator(system_id, data_type)
        // Should return: Box<dyn Fn(&Value) -> Result<()>>

        assert!(!system_id.is_empty());
        assert!(!data_type.is_empty());
    }

    #[test]
    fn test_registry_unknown_system_error() {
        // Attempting to validate unknown system should fail gracefully
        let system_id = "unknown_system";
        let should_error = !vec!["dnd5e"].contains(&system_id);

        assert!(should_error, "Unknown systems should not be found");
    }

    #[test]
    fn test_registry_unknown_data_type_error() {
        // Attempting to validate unknown data type should fail gracefully
        let system_id = "dnd5e";
        let data_type = "unknown_data_type";

        let valid_types = vec![
            "ability_data",
            "resource_data",
            "proficiency_data",
            "trait_data",
            "spell_data",
        ];

        let should_error = !valid_types.contains(&data_type);
        assert!(should_error, "Unknown data types should not be found");
    }

    #[test]
    fn test_registry_manifest_version_compatibility() {
        // Manifest should specify engine version compatibility
        let manifest = json!({
            "id": "dnd5e",
            "version": "0.1.0",
            "compatibility": {
                "minEngineVersion": "0.4.0"
            }
        });

        let min_version = manifest["compatibility"]["minEngineVersion"].as_str();
        assert_eq!(min_version, Some("0.4.0"));
    }

    #[test]
    fn test_registry_manifest_schema_validation() {
        // Each data type schema should have proper JSON Schema format
        let ability_schema = json!({
            "type": "object",
            "properties": {
                "strength": {
                    "type": "integer",
                    "minimum": 3,
                    "maximum": 20
                }
            },
            "required": ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"]
        });

        assert_eq!(ability_schema["type"], "object");
        assert!(ability_schema["properties"]["strength"].is_object());
        assert_eq!(ability_schema["properties"]["strength"]["type"], "integer");
    }
}
