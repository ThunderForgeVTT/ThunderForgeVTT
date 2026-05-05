// D&D 5e System Validators
// Validates system-specific data stored in world_actor_system_data JSONB columns
// Ensures data integrity without storing constraints in database

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

/// Validates D&D 5e ability scores (1-20 range)
pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data
        .as_object()
        .ok_or(ValidationError {
            field: "ability_data".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

    // Required abilities
    let required_abilities = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"];

    for ability in &required_abilities {
        let value = obj
            .get(*ability)
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: format!("ability_data.{}", ability),
                message: "must be an integer".to_string(),
            })?;

        if value < 1 || value > 20 {
            return Err(ValidationError {
                field: format!("ability_data.{}", ability),
                message: "must be between 1 and 20".to_string(),
            });
        }
    }

    Ok(())
}

// ============================================================================
// resource_data Validators
// ============================================================================

/// Validates D&D 5e HP and resources
pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data
        .as_object()
        .ok_or(ValidationError {
            field: "resource_data".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

    // max_hp is required
    let max_hp = obj
        .get("max_hp")
        .and_then(|v| v.as_i64())
        .ok_or(ValidationError {
            field: "resource_data.max_hp".to_string(),
            message: "must be a positive integer".to_string(),
        })?;

    if max_hp < 1 {
        return Err(ValidationError {
            field: "resource_data.max_hp".to_string(),
            message: "must be at least 1".to_string(),
        });
    }

    // current_hp validation (if present)
    if let Some(current_hp_val) = obj.get("current_hp") {
        let current_hp = current_hp_val.as_i64().ok_or(ValidationError {
            field: "resource_data.current_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if current_hp < 0 {
            return Err(ValidationError {
                field: "resource_data.current_hp".to_string(),
                message: "cannot be negative".to_string(),
            });
        }

        // Optional: warn if current_hp > max_hp (but allow it for temp HP)
        if current_hp > max_hp {
            // This is allowed (temporary HP), but we could log a warning
        }
    }

    // temporary_hp validation (if present)
    if let Some(temp_hp_val) = obj.get("temporary_hp") {
        let temp_hp = temp_hp_val.as_i64().ok_or(ValidationError {
            field: "resource_data.temporary_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if temp_hp < 0 {
            return Err(ValidationError {
                field: "resource_data.temporary_hp".to_string(),
                message: "cannot be negative".to_string(),
            });
        }
    }

    Ok(())
}

// ============================================================================
// proficiency_data Validators
// ============================================================================

/// Validates D&D 5e skill and saving throw proficiencies
pub fn validate_proficiency_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data
        .as_object()
        .ok_or(ValidationError {
            field: "proficiency_data".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

    // Valid skill names (matching system.json)
    let valid_skills = [
        "acrobatics",
        "animal_handling",
        "arcana",
        "athletics",
        "deception",
        "history",
        "insight",
        "intimidation",
        "investigation",
        "medicine",
        "nature",
        "perception",
        "performance",
        "persuasion",
        "religion",
        "sleight_of_hand",
        "stealth",
        "survival",
    ];

    // Validate skill_proficiencies (if present)
    if let Some(skills_val) = obj.get("skill_proficiencies") {
        let skills = skills_val.as_object().ok_or(ValidationError {
            field: "proficiency_data.skill_proficiencies".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

        for (skill_name, proficient) in skills {
            // Check if skill name is valid
            if !valid_skills.contains(&skill_name.as_str()) {
                return Err(ValidationError {
                    field: format!("proficiency_data.skill_proficiencies.{}", skill_name),
                    message: "unknown skill name".to_string(),
                });
            }

            // Check if value is boolean
            if !proficient.is_boolean() {
                return Err(ValidationError {
                    field: format!("proficiency_data.skill_proficiencies.{}", skill_name),
                    message: "must be a boolean".to_string(),
                });
            }
        }
    }

    // Validate saving_throw_proficiencies (if present)
    if let Some(saves_val) = obj.get("saving_throw_proficiencies") {
        let saves = saves_val.as_object().ok_or(ValidationError {
            field: "proficiency_data.saving_throw_proficiencies".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

        let valid_abilities = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"];

        for (ability, proficient) in saves {
            if !valid_abilities.contains(&ability.as_str()) {
                return Err(ValidationError {
                    field: format!("proficiency_data.saving_throw_proficiencies.{}", ability),
                    message: "invalid ability name".to_string(),
                });
            }

            if !proficient.is_boolean() {
                return Err(ValidationError {
                    field: format!("proficiency_data.saving_throw_proficiencies.{}", ability),
                    message: "must be a boolean".to_string(),
                });
            }
        }
    }

    // Validate proficiency_bonus (if present)
    if let Some(bonus_val) = obj.get("proficiency_bonus") {
        let bonus = bonus_val.as_i64().ok_or(ValidationError {
            field: "proficiency_data.proficiency_bonus".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if bonus < 2 || bonus > 6 {
            return Err(ValidationError {
                field: "proficiency_data.proficiency_bonus".to_string(),
                message: "must be between 2 and 6 (character levels 1-20)".to_string(),
            });
        }
    }

    // Validate languages (if present)
    if let Some(langs_val) = obj.get("languages") {
        let _langs = langs_val.as_array().ok_or(ValidationError {
            field: "proficiency_data.languages".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        // All items should be strings
        for (i, lang) in _langs.iter().enumerate() {
            if !lang.is_string() {
                return Err(ValidationError {
                    field: format!("proficiency_data.languages[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    Ok(())
}

// ============================================================================
// trait_data Validators
// ============================================================================

/// Validates D&D 5e character class, level, race, and feats
pub fn validate_trait_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data
        .as_object()
        .ok_or(ValidationError {
            field: "trait_data".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

    // class is required
    let _class = obj
        .get("class")
        .and_then(|v| v.as_str())
        .ok_or(ValidationError {
            field: "trait_data.class".to_string(),
            message: "must be a string".to_string(),
        })?;

    // level is required
    let level = obj
        .get("level")
        .and_then(|v| v.as_i64())
        .ok_or(ValidationError {
            field: "trait_data.level".to_string(),
            message: "must be an integer".to_string(),
        })?;

    if level < 1 || level > 20 {
        return Err(ValidationError {
            field: "trait_data.level".to_string(),
            message: "must be between 1 and 20".to_string(),
        });
    }

    // Validate optional fields
    if let Some(subclass_val) = obj.get("subclass") {
        if !subclass_val.is_string() && !subclass_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.subclass".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    if let Some(race_val) = obj.get("race") {
        if !race_val.is_string() && !race_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.race".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    if let Some(bg_val) = obj.get("background") {
        if !bg_val.is_string() && !bg_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.background".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    // Validate feats array (if present)
    if let Some(feats_val) = obj.get("feats") {
        let _feats = feats_val.as_array().ok_or(ValidationError {
            field: "trait_data.feats".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, feat) in _feats.iter().enumerate() {
            if !feat.is_string() {
                return Err(ValidationError {
                    field: format!("trait_data.feats[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Validate traits array (if present)
    if let Some(traits_val) = obj.get("traits") {
        let _traits = traits_val.as_array().ok_or(ValidationError {
            field: "trait_data.traits".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, trait_) in _traits.iter().enumerate() {
            if !trait_.is_string() {
                return Err(ValidationError {
                    field: format!("trait_data.traits[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    Ok(())
}

// ============================================================================
// spell_data Validators
// ============================================================================

/// Validates D&D 5e spellcasting data
pub fn validate_spell_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data
        .as_object()
        .ok_or(ValidationError {
            field: "spell_data".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

    // If any spell data is present, spellcasting_ability should be valid
    if let Some(ability_val) = obj.get("spellcasting_ability") {
        let ability = ability_val.as_str().ok_or(ValidationError {
            field: "spell_data.spellcasting_ability".to_string(),
            message: "must be a string".to_string(),
        })?;

        let valid_abilities = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"];
        if !valid_abilities.contains(&ability) {
            return Err(ValidationError {
                field: "spell_data.spellcasting_ability".to_string(),
                message: "must be one of the six ability scores".to_string(),
            });
        }
    }

    // Validate spell_save_dc (if present)
    if let Some(dc_val) = obj.get("spell_save_dc") {
        let dc = dc_val.as_i64().ok_or(ValidationError {
            field: "spell_data.spell_save_dc".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if dc < 8 || dc > 20 {
            return Err(ValidationError {
                field: "spell_data.spell_save_dc".to_string(),
                message: "must be between 8 and 20".to_string(),
            });
        }
    }

    // Validate spell_attack_bonus (if present)
    if let Some(bonus_val) = obj.get("spell_attack_bonus") {
        if !bonus_val.is_i64() {
            return Err(ValidationError {
                field: "spell_data.spell_attack_bonus".to_string(),
                message: "must be an integer".to_string(),
            });
        }
    }

    // Validate cantrips_known array (if present)
    if let Some(cantrips_val) = obj.get("cantrips_known") {
        let _cantrips = cantrips_val.as_array().ok_or(ValidationError {
            field: "spell_data.cantrips_known".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, cantrip) in _cantrips.iter().enumerate() {
            if !cantrip.is_string() {
                return Err(ValidationError {
                    field: format!("spell_data.cantrips_known[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Validate spells_known array (if present)
    if let Some(spells_val) = obj.get("spells_known") {
        let _spells = spells_val.as_array().ok_or(ValidationError {
            field: "spell_data.spells_known".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, spell) in _spells.iter().enumerate() {
            if !spell.is_string() {
                return Err(ValidationError {
                    field: format!("spell_data.spells_known[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Validate spell_slots (if present)
    if let Some(slots_val) = obj.get("spell_slots") {
        let slots = slots_val.as_object().ok_or(ValidationError {
            field: "spell_data.spell_slots".to_string(),
            message: "must be a JSON object".to_string(),
        })?;

        let valid_levels = ["level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8", "level_9"];

        for (level_key, slot_count_val) in slots {
            if !valid_levels.contains(&level_key.as_str()) {
                return Err(ValidationError {
                    field: format!("spell_data.spell_slots.{}", level_key),
                    message: "invalid spell level (must be level_1 through level_9)".to_string(),
                });
            }

            let slot_count = slot_count_val.as_i64().ok_or(ValidationError {
                field: format!("spell_data.spell_slots.{}", level_key),
                message: "must be an integer".to_string(),
            })?;

            if slot_count < 0 {
                return Err(ValidationError {
                    field: format!("spell_data.spell_slots.{}", level_key),
                    message: "cannot be negative".to_string(),
                });
            }
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_ability_data_valid() {
        let data = json!({
            "strength": 10,
            "dexterity": 12,
            "constitution": 14,
            "intelligence": 9,
            "wisdom": 16,
            "charisma": 11
        });
        assert!(validate_ability_data(&data).is_ok());
    }

    #[test]
    fn test_validate_ability_data_out_of_range() {
        let data = json!({
            "strength": 21,
            "dexterity": 12,
            "constitution": 14,
            "intelligence": 9,
            "wisdom": 16,
            "charisma": 11
        });
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn test_validate_ability_data_zero() {
        let data = json!({
            "strength": 0,
            "dexterity": 12,
            "constitution": 14,
            "intelligence": 9,
            "wisdom": 16,
            "charisma": 11
        });
        assert!(validate_ability_data(&data).is_err());
    }

    #[test]
    fn test_validate_resource_data_valid() {
        let data = json!({
            "max_hp": 32,
            "current_hp": 28,
            "temporary_hp": 5
        });
        assert!(validate_resource_data(&data).is_ok());
    }

    #[test]
    fn test_validate_resource_data_missing_max_hp() {
        let data = json!({"current_hp": 28});
        assert!(validate_resource_data(&data).is_err());
    }

    #[test]
    fn test_validate_resource_data_negative_current_hp() {
        let data = json!({
            "max_hp": 32,
            "current_hp": -5
        });
        assert!(validate_resource_data(&data).is_err());
    }

    #[test]
    fn test_validate_proficiency_data_valid_skills() {
        let data = json!({
            "skill_proficiencies": {
                "acrobatics": true,
                "arcana": false
            }
        });
        assert!(validate_proficiency_data(&data).is_ok());
    }

    #[test]
    fn test_validate_proficiency_data_invalid_skill() {
        let data = json!({
            "skill_proficiencies": {
                "invalid_skill": true
            }
        });
        assert!(validate_proficiency_data(&data).is_err());
    }

    #[test]
    fn test_validate_trait_data_valid() {
        let data = json!({
            "class": "Wizard",
            "level": 5,
            "race": "Elf",
            "feats": ["War Caster"]
        });
        assert!(validate_trait_data(&data).is_ok());
    }

    #[test]
    fn test_validate_trait_data_missing_class() {
        let data = json!({"level": 5});
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn test_validate_trait_data_invalid_level() {
        let data = json!({
            "class": "Wizard",
            "level": 21
        });
        assert!(validate_trait_data(&data).is_err());
    }

    #[test]
    fn test_validate_spell_data_valid() {
        let data = json!({
            "spellcasting_ability": "intelligence",
            "spell_save_dc": 14,
            "cantrips_known": ["Fire Bolt"],
            "spells_known": ["Magic Missile"],
            "spell_slots": {
                "level_1": 4,
                "level_2": 2
            }
        });
        assert!(validate_spell_data(&data).is_ok());
    }

    #[test]
    fn test_validate_spell_data_invalid_ability() {
        let data = json!({
            "spellcasting_ability": "invalid_ability"
        });
        assert!(validate_spell_data(&data).is_err());
    }

    #[test]
    fn test_validate_spell_data_invalid_dc() {
        let data = json!({"spell_save_dc": 25});
        assert!(validate_spell_data(&data).is_err());
    }
}

// ============================================================================
// Registry Adapters: Convert ValidationError -> String
// ============================================================================
// These functions wrap the validators to return Result<(), String> for use
// in the generic system registry (src/server/src/systems/mod.rs)

/// Adapter: validate_ability_data for registry
pub fn validate_ability_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_ability_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_resource_data for registry
pub fn validate_resource_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_resource_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_proficiency_data for registry
pub fn validate_proficiency_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_proficiency_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_trait_data for registry
pub fn validate_trait_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_trait_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_spell_data for registry
pub fn validate_spell_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_spell_data(data).map_err(|e| e.to_string())
}
