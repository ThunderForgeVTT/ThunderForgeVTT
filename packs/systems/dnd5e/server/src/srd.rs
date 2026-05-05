//! D&D 5e System Reference Document (SRD) Data
//!
//! Authoritative data structures for D&D 5e classes, spells, items, etc.
//! Used as reference for derived data calculations.

/// D&D 5e Skills and their associated abilities
pub struct SkillDefinition {
    pub name: &'static str,
    pub ability: &'static str,
}

pub const SKILLS: &[SkillDefinition] = &[
    SkillDefinition { name: "Acrobatics", ability: "dexterity" },
    SkillDefinition { name: "Animal Handling", ability: "wisdom" },
    SkillDefinition { name: "Arcana", ability: "intelligence" },
    SkillDefinition { name: "Athletics", ability: "strength" },
    SkillDefinition { name: "Deception", ability: "charisma" },
    SkillDefinition { name: "History", ability: "intelligence" },
    SkillDefinition { name: "Insight", ability: "wisdom" },
    SkillDefinition { name: "Intimidation", ability: "charisma" },
    SkillDefinition { name: "Investigation", ability: "intelligence" },
    SkillDefinition { name: "Medicine", ability: "wisdom" },
    SkillDefinition { name: "Nature", ability: "intelligence" },
    SkillDefinition { name: "Perception", ability: "wisdom" },
    SkillDefinition { name: "Performance", ability: "charisma" },
    SkillDefinition { name: "Persuasion", ability: "charisma" },
    SkillDefinition { name: "Religion", ability: "intelligence" },
    SkillDefinition { name: "Sleight of Hand", ability: "dexterity" },
    SkillDefinition { name: "Stealth", ability: "dexterity" },
    SkillDefinition { name: "Survival", ability: "wisdom" },
];

/// Get skill by name
pub fn get_skill(name: &str) -> Option<&'static SkillDefinition> {
    SKILLS.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// Class definitions (HP at 1st level, hit die)
pub struct ClassDefinition {
    pub name: &'static str,
    pub hit_points_1st: i32,
    pub hit_die: i32,
}

pub const CLASSES: &[ClassDefinition] = &[
    ClassDefinition { name: "Barbarian", hit_points_1st: 12, hit_die: 12 },
    ClassDefinition { name: "Bard", hit_points_1st: 8, hit_die: 8 },
    ClassDefinition { name: "Cleric", hit_points_1st: 8, hit_die: 8 },
    ClassDefinition { name: "Druid", hit_points_1st: 8, hit_die: 8 },
    ClassDefinition { name: "Fighter", hit_points_1st: 10, hit_die: 10 },
    ClassDefinition { name: "Monk", hit_points_1st: 8, hit_die: 8 },
    ClassDefinition { name: "Paladin", hit_points_1st: 10, hit_die: 10 },
    ClassDefinition { name: "Ranger", hit_points_1st: 10, hit_die: 10 },
    ClassDefinition { name: "Rogue", hit_points_1st: 8, hit_die: 8 },
    ClassDefinition { name: "Sorcerer", hit_points_1st: 6, hit_die: 6 },
    ClassDefinition { name: "Warlock", hit_points_1st: 6, hit_die: 6 },
    ClassDefinition { name: "Wizard", hit_points_1st: 6, hit_die: 6 },
];

/// Get class by name
pub fn get_class(name: &str) -> Option<&'static ClassDefinition> {
    CLASSES.iter().find(|c| c.name.eq_ignore_ascii_case(name))
}

/// Spell levels by character level (index 0 = cantrips, 1-9 = spell levels)
/// For full casters at each character level (Wizard/Cleric/Druid/Sorcerer)
pub const SPELL_SLOTS_BY_LEVEL: &[&[i32]] = &[
    &[2, 0, 0, 0, 0, 0, 0, 0, 0],    // Level 1
    &[3, 2, 0, 0, 0, 0, 0, 0, 0],    // Level 2
    &[4, 3, 2, 0, 0, 0, 0, 0, 0],    // Level 3
    &[4, 3, 3, 2, 0, 0, 0, 0, 0],    // Level 4
    &[4, 4, 3, 3, 2, 0, 0, 0, 0],    // Level 5
    &[4, 4, 3, 3, 3, 2, 0, 0, 0],    // Level 6
    &[4, 4, 4, 3, 3, 3, 2, 0, 0],    // Level 7
    &[4, 4, 4, 3, 3, 3, 3, 2, 0],    // Level 8
    &[4, 4, 4, 4, 3, 3, 3, 3, 3],    // Level 9
    &[5, 4, 4, 4, 3, 3, 3, 3, 3],    // Level 10
    &[5, 4, 4, 4, 4, 3, 3, 3, 3],    // Level 11
    &[5, 4, 4, 4, 4, 3, 3, 3, 3],    // Level 12
    &[5, 4, 4, 4, 4, 4, 3, 3, 3],    // Level 13
    &[5, 4, 4, 4, 4, 4, 3, 3, 3],    // Level 14
    &[5, 4, 4, 4, 4, 4, 4, 3, 3],    // Level 15
    &[5, 4, 4, 4, 4, 4, 4, 3, 3],    // Level 16
    &[5, 5, 4, 4, 4, 4, 4, 4, 3],    // Level 17
    &[5, 5, 4, 4, 4, 4, 4, 4, 3],    // Level 18
    &[5, 5, 4, 4, 4, 4, 4, 4, 4],    // Level 19
    &[5, 5, 4, 4, 4, 4, 4, 4, 4],    // Level 20
];

/// Get spell slots for character level (1-20)
pub fn get_spell_slots(level: u32) -> Option<&'static [i32]> {
    if level > 0 && level <= 20 {
        Some(SPELL_SLOTS_BY_LEVEL[(level - 1) as usize])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_skill() {
        let skill = get_skill("Perception");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().ability, "wisdom");
    }

    #[test]
    fn test_get_skill_case_insensitive() {
        assert!(get_skill("acrobatics").is_some());
        assert!(get_skill("ACROBATICS").is_some());
    }

    #[test]
    fn test_get_class() {
        let class = get_class("Wizard");
        assert!(class.is_some());
        assert_eq!(class.unwrap().hit_points_1st, 6);
    }

    #[test]
    fn test_spell_slots() {
        let slots_l1 = get_spell_slots(1).unwrap();
        assert_eq!(slots_l1[0], 2);  // 2 cantrips

        let slots_l5 = get_spell_slots(5).unwrap();
        assert_eq!(slots_l5[2], 3);  // 3 second-level slots at level 5
        assert_eq!(slots_l5[3], 3);  // 3 third-level slots at level 5
    }

    #[test]
    fn test_spell_slots_bounds() {
        assert!(get_spell_slots(0).is_none());
        assert!(get_spell_slots(21).is_none());
        assert!(get_spell_slots(10).is_some());
    }
}
