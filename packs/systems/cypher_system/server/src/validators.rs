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

/// Validates Cypher System stat Pools (Might/Speed/Intellect).
///
/// Per research/system_cypher_system.json's `core_stats[].scale`, the
/// 7-17 / 9-13 / 8-14 ranges are only *typical* values at 1st-tier
/// character creation — the `progression` section describes "Increasing
/// Capabilities" as an uncapped XP purchase that adds points to a stat
/// Pool with no stated hard ceiling. So a Pool is validated only as a
/// non-negative integer here, not clamped to the typical creation range;
/// a wildly high value (from advancement) must still be accepted.
pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    for stat in ["might", "speed", "intellect"] {
        let value = obj
            .get(stat)
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: format!("ability_data.{stat}"),
                message: "must be an integer".to_string(),
            })?;
        if value < 0 {
            return Err(ValidationError {
                field: format!("ability_data.{stat}"),
                message: "must be a non-negative integer (stat Pools cannot go negative)"
                    .to_string(),
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

    if let Some(v) = obj.get("might_pool") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.might_pool".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("speed_pool") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.speed_pool".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("intellect_pool") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.intellect_pool".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("effort") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.effort".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("xp") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.xp".to_string(),
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
    fn resource_data_accepts_empty_object() {
        assert!(validate_resource_data(&json!({})).is_ok());
    }

    #[test]
    fn ability_data_accepts_typical_creation_range() {
        let data = json!({"might": 12, "speed": 11, "intellect": 10});
        assert!(validate_ability_data(&data).is_ok());
    }

    #[test]
    fn ability_data_accepts_wildly_out_of_typical_range_value() {
        // Per the digest's `progression` section, "Increasing Capabilities"
        // is an uncapped XP-purchased Pool increase with no stated hard
        // ceiling, so a Pool far above the typical 7-17/9-13/8-14
        // creation ranges (e.g. from many tiers of advancement) must
        // still be accepted, not rejected as "out of range".
        let data = json!({"might": 500, "speed": 999, "intellect": 250});
        assert!(validate_ability_data(&data).is_ok());
    }

    #[test]
    fn ability_data_rejects_negative_pool() {
        let data = json!({"might": -1, "speed": 10, "intellect": 10});
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn ability_data_rejects_non_integer_pool() {
        let data = json!({"might": "twelve", "speed": 10, "intellect": 10});
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn proficiency_data_rejects_non_array_trained_skills() {
        assert!(validate_proficiency_data(&json!({"trained_skills": "not-an-array"})).is_err());
    }
}
