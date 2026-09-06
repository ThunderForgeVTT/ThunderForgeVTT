use super::*;

#[test]
fn test_registry_new() {
    let registry = GameSystemRegistry::new();
    assert_eq!(registry.systems.len(), 0);
}

#[test]
fn test_registry_register_and_validate() {
    let mut registry = GameSystemRegistry::new();

    // Simple test validator that rejects values > 100
    fn test_validator(data: &Value) -> Result<(), String> {
        if let Some(val) = data.get("test_field").and_then(|v| v.as_i64())
            && val > 100
        {
            return Err("Value too large".to_string());
        }
        Ok(())
    }

    registry.register(
        "test_system",
        SystemValidators {
            ability_data: Some(test_validator),
            resource_data: None,
            proficiency_data: None,
            trait_data: None,
            spell_data: None,
        },
    );

    // Should pass
    let valid_data = serde_json::json!({ "test_field": 50 });
    assert!(
        registry
            .validate("test_system", "ability_data", &valid_data)
            .is_ok()
    );

    // Should fail
    let invalid_data = serde_json::json!({ "test_field": 150 });
    assert!(
        registry
            .validate("test_system", "ability_data", &invalid_data)
            .is_err()
    );

    // Unknown system
    assert!(
        registry
            .validate("unknown_system", "ability_data", &valid_data)
            .is_err()
    );

    // Unknown data type
    assert!(
        registry
            .validate("test_system", "unknown_type", &valid_data)
            .is_err()
    );
}

#[test]
fn test_global_registry_dnd5e_registered() {
    let registry = GAME_SYSTEMS.lock().unwrap();
    // D&D 5e should be registered on first access
    assert!(registry.systems.contains_key("dnd5e"));
}
