//! D&D 5e Server Models
//!
//! Base data models for D&D 5e actors and items.
//! Per ADR-000 (Base vs. Derived Data):
//! - Store only BASE stats in database
//! - Calculate DERIVED stats (modifiers, skill bonuses, spell slots) on client/engine

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// D&D 5e Actor Data - Base stats only (stored in database)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnD5eActorData {
    /// Actor ID (token_id in world_tokens)
    pub id: String,

    /// Class name (e.g., "Rogue", "Cleric", "Wizard")
    /// Phase 4.8.1 will add multiclassing support
    pub class: String,

    /// Character level (1-20)
    pub level: u32,

    /// BASE Ability Scores (before bonuses)
    pub abilities: AbilityScores,

    /// Proficiencies (skill, saving throw, weapon, armor, tool)
    pub proficiencies: Proficiencies,

    /// Hit Points (base, not including modifiers)
    pub hit_points: i32,

    /// Armor Class (base, not including DEX modifier)
    pub armor_class: i32,

    /// Experience points (for leveling)
    pub experience: u32,

    /// Money (copper pieces)
    pub currency: CurrencyPurse,

    /// Features and traits (text descriptions, not calculated)
    pub features: Vec<String>,

    /// Spell known/prepared list (just names, not availability/slots)
    pub known_spells: Vec<String>,
}

impl Default for DnD5eActorData {
    fn default() -> Self {
        Self {
            id: String::new(),
            class: "Rogue".to_string(),
            level: 1,
            abilities: AbilityScores::default(),
            proficiencies: Proficiencies::default(),
            hit_points: 8,
            armor_class: 12,
            experience: 0,
            currency: CurrencyPurse::default(),
            features: vec![],
            known_spells: vec![],
        }
    }
}

/// Six Ability Scores (Strength, Dexterity, Constitution, Intelligence, Wisdom, Charisma)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AbilityScores {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

impl Default for AbilityScores {
    fn default() -> Self {
        Self {
            strength: 10,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        }
    }
}

impl AbilityScores {
    /// Get ability score by name
    pub fn get(&self, ability: &str) -> Option<i32> {
        match ability.to_lowercase().as_str() {
            "strength" | "str" => Some(self.strength),
            "dexterity" | "dex" => Some(self.dexterity),
            "constitution" | "con" => Some(self.constitution),
            "intelligence" | "int" => Some(self.intelligence),
            "wisdom" | "wis" => Some(self.wisdom),
            "charisma" | "cha" => Some(self.charisma),
            _ => None,
        }
    }

    /// Get all abilities as map
    pub fn as_map(&self) -> HashMap<String, i32> {
        let mut map = HashMap::new();
        map.insert("strength".to_string(), self.strength);
        map.insert("dexterity".to_string(), self.dexterity);
        map.insert("constitution".to_string(), self.constitution);
        map.insert("intelligence".to_string(), self.intelligence);
        map.insert("wisdom".to_string(), self.wisdom);
        map.insert("charisma".to_string(), self.charisma);
        map
    }
}

/// Proficiency tracking (boolean flags)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proficiencies {
    /// Skill proficiencies (18 skills)
    pub skills: SkillProficiencies,

    /// Saving throw proficiencies
    pub saving_throws: SavingThrowProficiencies,

    /// Weapon proficiencies
    pub weapons: Vec<String>,

    /// Armor proficiencies
    pub armor: Vec<String>,

    /// Tool proficiencies
    pub tools: Vec<String>,

    /// Languages known
    pub languages: Vec<String>,
}

impl Default for Proficiencies {
    fn default() -> Self {
        Self {
            skills: SkillProficiencies::default(),
            saving_throws: SavingThrowProficiencies::default(),
            weapons: vec![],
            armor: vec![],
            tools: vec![],
            languages: vec!["Common".to_string()],
        }
    }
}

/// 18 D&D 5e Skills
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillProficiencies {
    pub acrobatics: bool,
    pub animal_handling: bool,
    pub arcana: bool,
    pub athletics: bool,
    pub deception: bool,
    pub history: bool,
    pub insight: bool,
    pub intimidation: bool,
    pub investigation: bool,
    pub medicine: bool,
    pub nature: bool,
    pub perception: bool,
    pub performance: bool,
    pub persuasion: bool,
    pub religion: bool,
    pub sleight_of_hand: bool,
    pub stealth: bool,
    pub survival: bool,
}

/// Saving throw proficiencies (ability-based)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavingThrowProficiencies {
    pub strength: bool,
    pub dexterity: bool,
    pub constitution: bool,
    pub intelligence: bool,
    pub wisdom: bool,
    pub charisma: bool,
}

/// Currency purse (copper pieces are base unit)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct CurrencyPurse {
    pub platinum: i32, // 100 pp = 1 gp
    pub gold: i32,     // 10 gp = 1 pp
    pub electrum: i32, // 5 ep = 1 gp
    pub silver: i32,   // 10 sp = 1 gp
    pub copper: i32,   // 100 cp = 1 gp
}

/// D&D 5e Item Data - Base item information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnD5eItemData {
    /// Item ID
    pub id: String,

    /// Item name
    pub name: String,

    /// Item type (weapon, armor, spell, consumable, etc.)
    pub item_type: String,

    /// Item rarity (common, uncommon, rare, very rare, legendary, artifact)
    pub rarity: String,

    /// Whether item requires attunement
    pub requires_attunement: bool,

    /// Whether item is magical
    pub is_magical: bool,

    /// Item description
    pub description: String,

    /// Quantity
    pub quantity: i32,
}

impl Default for DnD5eItemData {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            item_type: "equipment".to_string(),
            rarity: "common".to_string(),
            requires_attunement: false,
            is_magical: false,
            description: String::new(),
            quantity: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ability_scores_default() {
        let scores = AbilityScores::default();
        assert_eq!(scores.strength, 10);
        assert_eq!(scores.dexterity, 10);
    }

    #[test]
    fn test_ability_scores_get() {
        let scores = AbilityScores {
            strength: 15,
            ..Default::default()
        };
        assert_eq!(scores.get("strength"), Some(15));
        assert_eq!(scores.get("STR"), Some(15));
        assert_eq!(scores.get("invalid"), None);
    }

    #[test]
    fn test_actor_data_default() {
        let actor = DnD5eActorData::default();
        assert_eq!(actor.class, "Rogue");
        assert_eq!(actor.level, 1);
        assert_eq!(actor.hit_points, 8);
    }
}
