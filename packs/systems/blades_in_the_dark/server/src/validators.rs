// Validators — mirrors packs/systems/dnd5e/server/src/validators.rs's pattern.

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

/// Validates Blades in the Dark attribute scores (0-4 range, per the
/// digest's `core_stats[].range` — aggregated from each attribute's linked
/// action ratings, not a d20-style ability score).
pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    for attribute in ["insight", "prowess", "resolve"] {
        let value = obj
            .get(attribute)
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: format!("ability_data.{}", attribute),
                message: "must be an integer".to_string(),
            })?;

        if !(0..=4).contains(&value) {
            return Err(ValidationError {
                field: format!("ability_data.{}", attribute),
                message: "must be between 0 and 4".to_string(),
            });
        }
    }

    Ok(())
}

pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(v) = obj.get("stress") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.stress".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("trauma") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.trauma".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("coin") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.coin".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }

    Ok(())
}

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

pub fn validate_trait_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    data.as_object().ok_or(ValidationError {
        field: "trait_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;
    Ok(())
}

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
    fn ability_data_accepts_values_within_0_to_4_range() {
        assert!(validate_ability_data(&json!({"insight": 0, "prowess": 2, "resolve": 4})).is_ok());
    }

    #[test]
    fn ability_data_rejects_value_above_4() {
        let err = validate_ability_data(&json!({"insight": 5, "prowess": 2, "resolve": 2}))
            .expect_err("insight of 5 exceeds the digest's 0-4 attribute range");
        assert_eq!(err.field, "ability_data.insight");
    }

    #[test]
    fn ability_data_rejects_negative_value() {
        let err = validate_ability_data(&json!({"insight": 2, "prowess": -1, "resolve": 2}))
            .expect_err("prowess of -1 is below the digest's 0-4 attribute range");
        assert_eq!(err.field, "ability_data.prowess");
    }

    #[test]
    fn resource_data_accepts_empty_object() {
        assert!(validate_resource_data(&json!({})).is_ok());
    }

    #[test]
    fn proficiency_data_rejects_non_array_trained_skills() {
        assert!(validate_proficiency_data(&json!({"trained_skills": "not-an-array"})).is_err());
    }
}
