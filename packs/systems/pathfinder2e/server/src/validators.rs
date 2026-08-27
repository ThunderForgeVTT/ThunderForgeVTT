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

/// research/system_pathfinder2e.json `core_stats[].scale`: PF2e ability
/// values are modifiers (not raw 1-20 scores like dnd5e) — "modifier-based,
/// typically -5 to +10ish".
const ABILITY_MODIFIER_MIN: i64 = -5;
const ABILITY_MODIFIER_MAX: i64 = 10;

fn require_ability_modifier(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<(), ValidationError> {
    let value = obj
        .get(field)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ValidationError {
            field: format!("ability_data.{field}"),
            message: "must be an integer".to_string(),
        })?;
    if !(ABILITY_MODIFIER_MIN..=ABILITY_MODIFIER_MAX).contains(&value) {
        return Err(ValidationError {
            field: format!("ability_data.{field}"),
            message: format!(
                "must be within the modifier range {ABILITY_MODIFIER_MIN}..={ABILITY_MODIFIER_MAX} (got {value})"
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

    require_ability_modifier(obj, "strength")?;
    require_ability_modifier(obj, "dexterity")?;
    require_ability_modifier(obj, "constitution")?;
    require_ability_modifier(obj, "intelligence")?;
    require_ability_modifier(obj, "wisdom")?;
    require_ability_modifier(obj, "charisma")?;

    Ok(())
}

pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(v) = obj.get("current_hp") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.current_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("max_hp") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.max_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("focus_points") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.focus_points".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("hero_points") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.hero_points".to_string(),
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

    /// research/system_pathfinder2e.json `core_stats[].scale`: PF2e
    /// ability values are modifiers, not raw 1-20 scores — "modifier-based,
    /// typically -5 to +10ish" (min -5, max 10). An out-of-range value
    /// must be rejected, not silently accepted as any integer.
    #[test]
    fn ability_data_rejects_modifier_above_max() {
        let data = json!({
            "strength": 11,
            "dexterity": 0,
            "constitution": 0,
            "intelligence": 0,
            "wisdom": 0,
            "charisma": 0
        });
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn ability_data_rejects_modifier_below_min() {
        let data = json!({
            "strength": -6,
            "dexterity": 0,
            "constitution": 0,
            "intelligence": 0,
            "wisdom": 0,
            "charisma": 0
        });
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn ability_data_accepts_modifier_within_range() {
        let data = json!({
            "strength": -5,
            "dexterity": 10,
            "constitution": 0,
            "intelligence": 4,
            "wisdom": -2,
            "charisma": 7
        });
        assert!(validate_ability_data(&data).is_ok());
    }
}
