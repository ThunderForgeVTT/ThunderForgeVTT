//! What 5e computes, as opposed to what it stores.
//!
//! The published character sheet stores an ability score *and* its modifier,
//! because paper cannot compute. Six of its 336 fields are scores and six more
//! are the modifiers derived from them, and the same is true of every saving
//! throw, every skill and the passive score. This is where those stop being
//! things a player copies by hand.
//!
//! # Everything that can come from the manifest, does
//!
//! `system.json` already declares the six abilities and all eighteen skills,
//! and each skill names the ability it keys off. Restating any of that here
//! would create a second list to keep in step — the failure
//! `thunderforge_canvas_core::attributes` exists to end. So the rules are
//! constructed from the manifest and hold only what a manifest cannot express:
//! the arithmetic, and the by-level proficiency table.

use std::collections::BTreeMap;

use thunderforge_canvas_core::attributes::AttributeDeclaration;
use thunderforge_canvas_core::system_rules::{
    DeclaredValue, DeclaredValueKind, DeclaredValues, Origin, SystemRules,
};

/// Where a character's level is stored, inside `trait_data`.
const LEVEL: &str = "level";
/// Which skills the character is proficient in, inside `proficiency_data`.
const SKILL_PROFICIENCIES: &str = "skill_proficiencies";
/// Which saving throws, likewise.
const SAVE_PROFICIENCIES: &str = "saving_throw_proficiencies";
/// The skill a passive score is read from.
const PERCEPTION: &str = "perception";

/// An ability modifier, the way the book has it.
///
/// **Floor, not truncation.** Rust's `/` rounds toward zero, so `(7 - 10) / 2`
/// is `-1` where 5e wants `-2`. The staged version of this rule — preserved
/// verbatim from a trait deleted in T010 so its numbers would survive — had
/// exactly that bug, and it was wrong for every odd score below ten: 7, 5, 3
/// and 1 are the common ones on a real sheet. The dead TypeScript beside it
/// used `Math.floor` and was right.
///
/// The lesson is not about division. It is that a rule with no test is a rule
/// nobody has checked, however long it has sat in the repository looking
/// settled.
pub fn ability_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Proficiency bonus by character level.
///
/// The one table here a manifest does not carry. Out of range yields none
/// rather than a default: a level the book does not cover is a sheet nobody
/// can read, and guessing two would put a number on it that no rule supports.
pub fn proficiency_bonus(level: i32) -> Option<i32> {
    match level {
        1..=4 => Some(2),
        5..=8 => Some(3),
        9..=12 => Some(4),
        13..=16 => Some(5),
        17..=20 => Some(6),
        _ => None,
    }
}

/// One skill, as the manifest declares it.
struct Skill {
    id: String,
    label: String,
    ability: String,
}

pub struct DnD5eRules {
    /// Ability id to label, from the manifest.
    abilities: BTreeMap<String, String>,
    skills: Vec<Skill>,
}

impl DnD5eRules {
    pub fn from_manifest(manifest: &serde_json::Value) -> Self {
        let abilities = manifest
            .get("abilities")
            .and_then(|a| a.as_object())
            .map(|block| {
                block
                    .iter()
                    .map(|(id, spec)| {
                        let label = spec
                            .get("label")
                            .and_then(|l| l.as_str())
                            .unwrap_or(id)
                            .to_string();
                        (id.clone(), label)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let skills = manifest
            .get("skills")
            .and_then(|s| s.as_object())
            .map(|block| {
                block
                    .iter()
                    .filter_map(|(id, spec)| {
                        Some(Skill {
                            id: id.clone(),
                            label: spec.get("label").and_then(|l| l.as_str())?.to_string(),
                            ability: spec.get("ability").and_then(|a| a.as_str())?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self { abilities, skills }
    }

    fn is_proficient(stored: &DeclaredValues, field: &str, id: &str) -> bool {
        matches!(
            stored.get(field).map(|value| &value.value),
            Some(DeclaredValueKind::List(items)) if items.iter().any(|item| item == id)
        )
    }
}

/// `strength` becomes `strengthMod`, `saveStrength`, and so on — the shape the
/// rest of the product already uses for a declared identifier.
fn modifier_id(ability: &str) -> String {
    format!("{ability}Mod")
}

fn save_id(ability: &str) -> String {
    format!("save{}{}", ability[..1].to_uppercase(), &ability[1..])
}

fn skill_id(skill: &str) -> String {
    let mut out = String::from("skill");
    let mut upper = true;
    for c in skill.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn declaration(id: String, label: String, order: usize) -> AttributeDeclaration {
    AttributeDeclaration {
        source: id.clone(),
        id,
        label,
        abbreviation: None,
        order,
    }
}

fn derived(id: String, label: String, value: i32) -> DeclaredValue {
    DeclaredValue {
        id,
        label,
        abbreviation: None,
        value: DeclaredValueKind::Integer(value),
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Derived,
    }
}

impl SystemRules for DnD5eRules {
    fn id(&self) -> &str {
        "dnd5e"
    }

    fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
        let mut out = Vec::new();
        let mut order = 0;

        for (ability, label) in &self.abilities {
            out.push(declaration(
                modifier_id(ability),
                format!("{label} Modifier"),
                order,
            ));
            order += 1;
            out.push(declaration(
                save_id(ability),
                format!("{label} Save"),
                order,
            ));
            order += 1;
        }

        out.push(declaration(
            "proficiencyBonus".to_string(),
            "Proficiency Bonus".to_string(),
            order,
        ));
        order += 1;

        for skill in &self.skills {
            out.push(declaration(skill_id(&skill.id), skill.label.clone(), order));
            order += 1;
        }

        out.push(declaration(
            "passivePerception".to_string(),
            "Passive Perception".to_string(),
            order,
        ));

        out
    }

    fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue> {
        let mut out = Vec::new();

        // Modifiers first, and kept, because everything below is built on
        // them.
        let mut modifiers: BTreeMap<&str, i32> = BTreeMap::new();
        for (ability, label) in &self.abilities {
            let Some(score) = stored.integer(ability) else {
                // A score nobody entered is not a score of nothing.
                continue;
            };
            let modifier = ability_modifier(score);
            modifiers.insert(ability, modifier);
            out.push(derived(
                modifier_id(ability),
                format!("{label} Modifier"),
                modifier,
            ));
        }

        // Level drives the proficiency bonus, and without it nothing that
        // depends on proficiency can be computed at all.
        let bonus = stored.integer(LEVEL).and_then(proficiency_bonus);
        if let Some(bonus) = bonus {
            out.push(derived(
                "proficiencyBonus".to_string(),
                "Proficiency Bonus".to_string(),
                bonus,
            ));
        }

        for (ability, label) in &self.abilities {
            let (Some(modifier), Some(bonus)) = (modifiers.get(ability.as_str()), bonus) else {
                continue;
            };
            let proficient = Self::is_proficient(stored, SAVE_PROFICIENCIES, ability);
            out.push(derived(
                save_id(ability),
                format!("{label} Save"),
                modifier + if proficient { bonus } else { 0 },
            ));
        }

        for skill in &self.skills {
            let (Some(modifier), Some(bonus)) = (modifiers.get(skill.ability.as_str()), bonus)
            else {
                continue;
            };
            let proficient = Self::is_proficient(stored, SKILL_PROFICIENCIES, &skill.id);
            let total = modifier + if proficient { bonus } else { 0 };
            out.push(derived(skill_id(&skill.id), skill.label.clone(), total));

            if skill.id == PERCEPTION {
                // Ten plus the skill: the number a Game Master reads without
                // asking anyone to roll.
                out.push(derived(
                    "passivePerception".to_string(),
                    "Passive Perception".to_string(),
                    10 + total,
                ));
            }
        }

        out
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
