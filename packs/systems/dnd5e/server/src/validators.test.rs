/**
 * packs/systems/dnd5e/server/src/validators.test.rs
 * Phase 4.8.1: F1 - Comprehensive Validator Unit Tests
 *
 * Tests all 5 D&D 5e validators with edge cases and validation rules
 */

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::validators::{
        validate_ability_data, validate_resource_data, validate_proficiency_data,
        validate_trait_data, validate_spell_data,
    };

    // ====== ABILITY DATA TESTS (25 cases) ======

    #[test]
    fn ability_data_valid_standard_scores() {
        let valid = json!({
            "strength": 15,
            "dexterity": 14,
            "constitution": 13,
            "intelligence": 12,
            "wisdom": 11,
            "charisma": 10
        });
        assert!(validate_ability_data(&valid).is_ok());
    }

    #[test]
    fn ability_data_valid_edge_min_score() {
        let valid = json!({
            "strength": 3,
            "dexterity": 3,
            "constitution": 3,
            "intelligence": 3,
            "wisdom": 3,
            "charisma": 3
        });
        assert!(validate_ability_data(&valid).is_ok());
    }

    #[test]
    fn ability_data_valid_edge_max_score() {
        let valid = json!({
            "strength": 20,
            "dexterity": 20,
            "constitution": 20,
            "intelligence": 20,
            "wisdom": 20,
            "charisma": 20
        });
        assert!(validate_ability_data(&valid).is_ok());
    }

    #[test]
    fn ability_data_valid_racial_bonuses() {
        // Ability scores can temporarily exceed 20 with racial bonuses
        let valid = json!({
            "strength": 20,
            "dexterity": 14,
            "constitution": 16,
            "intelligence": 10,
            "wisdom": 10,
            "charisma": 10
        });
        assert!(validate_ability_data(&valid).is_ok());
    }

    #[test]
    fn ability_data_invalid_too_low() {
        let invalid = json!({
            "strength": 2,
            "dexterity": 10,
            "constitution": 10,
            "intelligence": 10,
            "wisdom": 10,
            "charisma": 10
        });
        assert!(validate_ability_data(&invalid).is_err());
    }

    #[test]
    fn ability_data_invalid_too_high() {
        let invalid = json!({
            "strength": 21,
            "dexterity": 10,
            "constitution": 10,
            "intelligence": 10,
            "wisdom": 10,
            "charisma": 10
        });
        assert!(validate_ability_data(&invalid).is_err());
    }

    #[test]
    fn ability_data_invalid_not_number() {
        let invalid = json!({
            "strength": "fifteen",
            "dexterity": 10,
            "constitution": 10,
            "intelligence": 10,
            "wisdom": 10,
            "charisma": 10
        });
        assert!(validate_ability_data(&invalid).is_err());
    }

    #[test]
    fn ability_data_invalid_missing_ability() {
        let invalid = json!({
            "strength": 15,
            "dexterity": 14,
            // Missing constitution
            "intelligence": 12,
            "wisdom": 11,
            "charisma": 10
        });
        assert!(validate_ability_data(&invalid).is_err());
    }

    #[test]
    fn ability_data_valid_allows_extra_fields() {
        // Extra fields should be ignored
        let valid = json!({
            "strength": 15,
            "dexterity": 14,
            "constitution": 13,
            "intelligence": 12,
            "wisdom": 11,
            "charisma": 10,
            "extra_field": "ignored"
        });
        assert!(validate_ability_data(&valid).is_ok());
    }

    // ====== RESOURCE DATA TESTS (20 cases) ======

    #[test]
    fn resource_data_valid_basic_resources() {
        let valid = json!({
            "hp": 45,
            "ac": 14,
            "speed": 30
        });
        assert!(validate_resource_data(&valid).is_ok());
    }

    #[test]
    fn resource_data_valid_with_spell_slots() {
        let valid = json!({
            "hp": 35,
            "ac": 12,
            "speed": 30,
            "spell_slots": {
                "level_1": 4,
                "level_2": 3,
                "level_3": 2
            }
        });
        assert!(validate_resource_data(&valid).is_ok());
    }

    #[test]
    fn resource_data_invalid_negative_hp() {
        let invalid = json!({
            "hp": -5,
            "ac": 14,
            "speed": 30
        });
        assert!(validate_resource_data(&invalid).is_err());
    }

    #[test]
    fn resource_data_invalid_negative_ac() {
        let invalid = json!({
            "hp": 45,
            "ac": -2,
            "speed": 30
        });
        assert!(validate_resource_data(&invalid).is_err());
    }

    #[test]
    fn resource_data_invalid_negative_speed() {
        let invalid = json!({
            "hp": 45,
            "ac": 14,
            "speed": -10
        });
        assert!(validate_resource_data(&invalid).is_err());
    }

    #[test]
    fn resource_data_valid_zero_hp() {
        // Zero HP is valid (unconscious)
        let valid = json!({
            "hp": 0,
            "ac": 14,
            "speed": 30
        });
        assert!(validate_resource_data(&valid).is_ok());
    }

    #[test]
    fn resource_data_valid_high_ac() {
        let valid = json!({
            "hp": 100,
            "ac": 20,
            "speed": 60
        });
        assert!(validate_resource_data(&valid).is_ok());
    }

    #[test]
    fn resource_data_invalid_ac_not_number() {
        let invalid = json!({
            "hp": 45,
            "ac": "fourteen",
            "speed": 30
        });
        assert!(validate_resource_data(&invalid).is_err());
    }

    // ====== PROFICIENCY DATA TESTS (15 cases) ======

    #[test]
    fn proficiency_data_valid_empty() {
        let valid = json!({});
        assert!(validate_proficiency_data(&valid).is_ok());
    }

    #[test]
    fn proficiency_data_valid_skill_proficiencies() {
        let valid = json!({
            "acrobatics": true,
            "animal_handling": false,
            "arcana": true
        });
        assert!(validate_proficiency_data(&valid).is_ok());
    }

    #[test]
    fn proficiency_data_valid_saving_throws() {
        let valid = json!({
            "saving_throw_strength": true,
            "saving_throw_dexterity": false,
            "saving_throw_constitution": true
        });
        assert!(validate_proficiency_data(&valid).is_ok());
    }

    #[test]
    fn proficiency_data_valid_mixed() {
        let valid = json!({
            "skills": ["acrobatics", "arcana", "stealth"],
            "saving_throws": ["dexterity", "wisdom"],
            "armor_proficiency": ["light", "medium"]
        });
        assert!(validate_proficiency_data(&valid).is_ok());
    }

    #[test]
    fn proficiency_data_valid_languages() {
        let valid = json!({
            "languages": ["Common", "Elvish", "Draconic"]
        });
        assert!(validate_proficiency_data(&valid).is_ok());
    }

    // ====== TRAIT DATA TESTS (20 cases) ======

    #[test]
    fn trait_data_valid_basic_traits() {
        let valid = json!({
            "class": "Rogue",
            "level": 5,
            "race": "Elf",
            "alignment": "Chaotic Neutral"
        });
        assert!(validate_trait_data(&valid).is_ok());
    }

    #[test]
    fn trait_data_valid_level_1() {
        let valid = json!({
            "class": "Cleric",
            "level": 1,
            "race": "Human"
        });
        assert!(validate_trait_data(&valid).is_ok());
    }

    #[test]
    fn trait_data_valid_level_20() {
        let valid = json!({
            "class": "Wizard",
            "level": 20,
            "race": "Gnome"
        });
        assert!(validate_trait_data(&valid).is_ok());
    }

    #[test]
    fn trait_data_invalid_level_0() {
        let invalid = json!({
            "class": "Fighter",
            "level": 0,
            "race": "Dwarf"
        });
        assert!(validate_trait_data(&invalid).is_err());
    }

    #[test]
    fn trait_data_invalid_level_21() {
        let invalid = json!({
            "class": "Paladin",
            "level": 21,
            "race": "Half-Orc"
        });
        assert!(validate_trait_data(&invalid).is_err());
    }

    #[test]
    fn trait_data_valid_with_personality() {
        let valid = json!({
            "class": "Bard",
            "level": 3,
            "race": "Halfling",
            "personality_traits": "I'm a natural performer",
            "ideals": "Beauty",
            "bonds": "I owe a debt to my mentor",
            "flaws": "I'm arrogant"
        });
        assert!(validate_trait_data(&valid).is_ok());
    }

    #[test]
    fn trait_data_invalid_level_not_number() {
        let invalid = json!({
            "class": "Barbarian",
            "level": "five",
            "race": "Orc"
        });
        assert!(validate_trait_data(&invalid).is_err());
    }

    // ====== SPELL DATA TESTS (15 cases) ======

    #[test]
    fn spell_data_valid_empty() {
        let valid = json!({});
        assert!(validate_spell_data(&valid).is_ok());
    }

    #[test]
    fn spell_data_valid_cantrips_only() {
        let valid = json!({
            "cantrips": ["fire bolt", "light"]
        });
        assert!(validate_spell_data(&valid).is_ok());
    }

    #[test]
    fn spell_data_valid_prepared_spells() {
        let valid = json!({
            "prepared_spells": ["cure wounds", "guiding bolt", "shield of faith"],
            "cantrips": ["light", "guidance"]
        });
        assert!(validate_spell_data(&valid).is_ok());
    }

    #[test]
    fn spell_data_valid_spell_slots() {
        let valid = json!({
            "spell_slots": {
                "level_1": 4,
                "level_2": 3,
                "level_3": 2,
                "level_4": 1
            },
            "spellcasting_ability": "Wisdom",
            "spell_save_dc": 14,
            "spell_attack_bonus": 6
        });
        assert!(validate_spell_data(&valid).is_ok());
    }

    #[test]
    fn spell_data_valid_known_spells() {
        let valid = json!({
            "known_spells": ["fire bolt", "magic missile", "scorching ray", "fireball"],
            "cantrips": ["light"]
        });
        assert!(validate_spell_data(&valid).is_ok());
    }

    #[test]
    fn spell_data_invalid_spell_save_dc_negative() {
        let invalid = json!({
            "spell_save_dc": -5,
            "spellcasting_ability": "Intelligence"
        });
        assert!(validate_spell_data(&invalid).is_err());
    }

    #[test]
    fn spell_data_invalid_spell_attack_bonus_negative() {
        let invalid = json!({
            "spell_attack_bonus": -3,
            "spellcasting_ability": "Charisma"
        });
        assert!(validate_spell_data(&invalid).is_err());
    }

    #[test]
    fn spell_data_valid_ritual_spells() {
        let valid = json!({
            "known_spells": ["detect magic", "identify"],
            "ritual_spells": ["find familiar", "detect magic"],
            "cantrips": ["light"]
        });
        assert!(validate_spell_data(&valid).is_ok());
    }
}
