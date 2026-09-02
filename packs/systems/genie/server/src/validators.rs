// Genie System Validators
// Validates system-specific data stored in world_actor_system_data JSONB columns.
// Mirrors packs/systems/dnd5e/server/src/validators.rs's pattern.

/// Validation error for system data
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

// ============================================================================
// ability_data Validators
// ============================================================================

/// Validates Genie ability scores (might, cunning, spirit — all required integers)
pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    let required_abilities = ["might", "cunning", "spirit"];

    for ability in &required_abilities {
        obj.get(*ability)
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: format!("ability_data.{ability}"),
                message: "must be an integer".to_string(),
            })?;
    }

    Ok(())
}

// ============================================================================
// resource_data Validators
// ============================================================================

/// Validates Genie Wish Points and Health resource pools
pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    let max_wish_points =
        obj.get("max_wish_points")
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: "resource_data.max_wish_points".to_string(),
                message: "must be an integer".to_string(),
            })?;

    let max_health = obj
        .get("max_health")
        .and_then(|v| v.as_i64())
        .ok_or(ValidationError {
            field: "resource_data.max_health".to_string(),
            message: "must be a positive integer".to_string(),
        })?;

    if max_health < 1 {
        return Err(ValidationError {
            field: "resource_data.max_health".to_string(),
            message: "must be at least 1".to_string(),
        });
    }

    if max_wish_points < 0 {
        return Err(ValidationError {
            field: "resource_data.max_wish_points".to_string(),
            message: "must not be negative".to_string(),
        });
    }

    Ok(())
}

// ============================================================================
// proficiency_data Validators
// ============================================================================

/// Validates Genie skill training flags
pub fn validate_proficiency_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "proficiency_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(trained) = obj.get("trained_skills") {
        trained.as_array().ok_or(ValidationError {
            field: "proficiency_data.trained_skills".to_string(),
            message: "must be an array".to_string(),
        })?;
    }

    Ok(())
}

// ============================================================================
// trait_data Validators (reused slot: conditions, Patron link, NPC size_category)
// ============================================================================

/// Validates Genie conditions/Patron/size_category, reusing the registry's
/// `trait_data` slot since there is no dedicated condition-data slot
/// (data-model.md's `condition_data`/`patron_lore_entry_id`/`size_category`).
pub fn validate_trait_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "trait_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(conditions) = obj.get("active_conditions") {
        let conditions = conditions.as_array().ok_or(ValidationError {
            field: "trait_data.active_conditions".to_string(),
            message: "must be an array".to_string(),
        })?;

        // Known condition keys, mirroring `conditions` in
        // packs/systems/genie/system.json (spec 018 User Story 4).
        let valid_conditions = ["bound", "exposed", "favored"];

        for condition in conditions {
            let value = condition.as_str().ok_or(ValidationError {
                field: "trait_data.active_conditions".to_string(),
                message: "each condition must be a string".to_string(),
            })?;
            if !valid_conditions.contains(&value) {
                return Err(ValidationError {
                    field: "trait_data.active_conditions".to_string(),
                    message: format!(
                        "unknown condition key {value:?}; must be one of {valid_conditions:?}"
                    ),
                });
            }
        }
    }

    if let Some(patron) = obj.get("patron_lore_entry_id") {
        if !patron.is_null() {
            patron.as_str().ok_or(ValidationError {
                field: "trait_data.patron_lore_entry_id".to_string(),
                message: "must be a string (lore entry UUID) or null".to_string(),
            })?;
        }
    }

    if let Some(size) = obj.get("size_category") {
        if !size.is_null() {
            let value = size.as_str().ok_or(ValidationError {
                field: "trait_data.size_category".to_string(),
                message: "must be a string or null".to_string(),
            })?;
            let valid = ["diminutive", "small", "medium", "large", "huge", "colossal"];
            if !valid.contains(&value) {
                return Err(ValidationError {
                    field: "trait_data.size_category".to_string(),
                    message: format!("must be one of {valid:?}"),
                });
            }
        }
    }

    if let Some(level) = obj.get("level") {
        if !level.is_null() {
            let value = level.as_i64().ok_or(ValidationError {
                field: "trait_data.level".to_string(),
                message: "must be an integer or null".to_string(),
            })?;
            // Matches `wishPoints`'s table range in system.json (levels
            // 1-10) — the only levels calculateMaxWishPoints knows how to
            // score.
            if !(1..=10).contains(&value) {
                return Err(ValidationError {
                    field: "trait_data.level".to_string(),
                    message: "must be between 1 and 10".to_string(),
                });
            }
        }
    }

    Ok(())
}

// ============================================================================
// Registry adapters (Result<(), String> instead of Result<(), ValidationError>)
// ============================================================================

pub fn validate_ability_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_ability_data(data).map_err(|e| e.to_string())
}

pub fn validate_resource_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_resource_data(data).map_err(|e| e.to_string())
}

pub fn validate_proficiency_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_proficiency_data(data).map_err(|e| e.to_string())
}

pub fn validate_trait_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_trait_data(data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ability_data_requires_all_three_abilities() {
        let data = json!({ "might": 3, "cunning": 4 });
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn ability_data_accepts_complete_set() {
        let data = json!({ "might": 3, "cunning": 4, "spirit": 2 });
        assert!(validate_ability_data(&data).is_ok());
    }

    #[test]
    fn resource_data_rejects_zero_max_health() {
        let data = json!({ "max_wish_points": 2, "max_health": 0 });
        assert!(validate_resource_data(&data).is_err());
    }

    #[test]
    fn trait_data_rejects_unknown_size_category() {
        let data = json!({ "size_category": "gigantic" });
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn trait_data_accepts_known_size_category() {
        let data = json!({ "size_category": "colossal" });
        assert!(validate_trait_data(&data).is_ok());
    }

    #[test]
    fn trait_data_rejects_unknown_condition_key() {
        let data = json!({ "active_conditions": ["not_a_real_condition"] });
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn trait_data_accepts_known_condition_keys() {
        let data = json!({ "active_conditions": ["bound", "exposed", "favored"] });
        assert!(validate_trait_data(&data).is_ok());
    }

    #[test]
    fn trait_data_accepts_empty_conditions_list() {
        let data = json!({ "active_conditions": [] });
        assert!(validate_trait_data(&data).is_ok());
    }

    #[test]
    fn trait_data_rejects_level_zero() {
        let data = json!({ "level": 0 });
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn trait_data_rejects_level_above_ten() {
        let data = json!({ "level": 11 });
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn trait_data_accepts_level_in_range() {
        let data = json!({ "level": 3 });
        assert!(validate_trait_data(&data).is_ok());
    }

    /// **The boundaries themselves**, written as literals.
    ///
    /// A mutation audit on 2026-09-02 narrowed this rule from 1-10 to 2-9 and
    /// all twenty tests in this pack still passed: the accept case uses 3 and
    /// the reject cases sit outside both ends, so the two levels the rule
    /// actually names were never supplied. Levels 1 and 10 are also the ends
    /// of the Wish Points table in `system.json`, so getting this wrong would
    /// make a first- or tenth-level character unsaveable.
    ///
    /// Literals rather than the range's own bounds: a test written against the
    /// constant asserts that the rule accepts whatever the rule is written
    /// against, which is true for every range and catches nothing.
    #[test]
    fn trait_data_accepts_the_first_and_last_levels_the_table_covers() {
        assert!(
            validate_trait_data(&json!({ "level": 1 })).is_ok(),
            "a first-level character must be saveable"
        );
        assert!(
            validate_trait_data(&json!({ "level": 10 })).is_ok(),
            "and so must a tenth-level one — the last rung of the Wish Points table"
        );
    }

    #[test]
    fn trait_data_accepts_no_level_at_all() {
        let data = json!({ "active_conditions": [] });
        assert!(validate_trait_data(&data).is_ok());
    }
}
