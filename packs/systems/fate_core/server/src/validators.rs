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

pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // This system has no fixed ability scores; any object (even empty) is valid.

    Ok(())
}

pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    if let Some(v) = obj.get("fate_points") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.fate_points".to_string(),
            message: "must be an integer".to_string(),
        })?;
    }
    if let Some(v) = obj.get("refresh") {
        v.as_i64().ok_or(ValidationError {
            field: "resource_data.refresh".to_string(),
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
}
