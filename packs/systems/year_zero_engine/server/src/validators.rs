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

/// Standard d6-pool variant's attribute range (research/system_year_zero_engine.json
/// core_stats[].scale: `{"type": "pool", "min": 1, "max": 5}`). The step-dice variant
/// (letter grades D-A, used by games like Twilight 2000) is out of scope for this pass.
const ABILITY_MIN: i64 = 1;
const ABILITY_MAX: i64 = 5;

fn validate_ability_score(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<(), ValidationError> {
    let value = obj.get(field).and_then(|v| v.as_i64()).ok_or(ValidationError {
        field: format!("ability_data.{field}"),
        message: "must be an integer".to_string(),
    })?;
    if !(ABILITY_MIN..=ABILITY_MAX).contains(&value) {
        return Err(ValidationError {
            field: format!("ability_data.{field}"),
            message: format!(
                "must be between {ABILITY_MIN} and {ABILITY_MAX} (standard d6-pool variant range)"
            ),
        });
    }
    Ok(())
}

pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    validate_ability_score(obj, "strength")?;
    validate_ability_score(obj, "agility")?;
    validate_ability_score(obj, "wits")?;
    validate_ability_score(obj, "empathy")?;

    Ok(())
}

pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(v) = obj.get("health") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.health".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("resolve") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.resolve".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("stress") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.stress".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("experience_points") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.experience_points".to_string(),
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
    fn proficiency_data_rejects_non_array_trained_skills() {
        assert!(validate_proficiency_data(&json!({"trained_skills": "not-an-array"})).is_err());
    }

    fn valid_abilities() -> serde_json::Value {
        json!({"strength": 3, "agility": 3, "wits": 3, "empathy": 3})
    }

    #[test]
    fn ability_data_accepts_in_range_scores() {
        assert!(validate_ability_data(&valid_abilities()).is_ok());
    }

    #[test]
    fn ability_data_rejects_score_below_standard_dice_pool_range() {
        // research/system_year_zero_engine.json core_stats[].scale: standard
        // d6-pool variant range is 1-5; 0 is below the minimum.
        let mut abilities = valid_abilities();
        abilities["strength"] = json!(0);
        let err = validate_ability_data(&abilities).unwrap_err();
        assert_eq!(err.field, "ability_data.strength");
    }

    #[test]
    fn ability_data_rejects_score_above_standard_dice_pool_range() {
        let mut abilities = valid_abilities();
        abilities["empathy"] = json!(6);
        let err = validate_ability_data(&abilities).unwrap_err();
        assert_eq!(err.field, "ability_data.empathy");
    }
}
