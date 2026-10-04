//! The creatures this pack can put on an actor: SRD stat blocks.
//!
//! # How a monster has to be written down to fight
//!
//! Nothing in the combat flow knows what a monster is. It reads the same
//! places it reads for a character, and a stat block is only useful once it
//! has been spread across them. This is where each line of a printed block
//! goes, and it is the whole contract: a creature with these filled in can be
//! attacked, can attack, moves and sees, with nobody typing anything.
//!
//! | Printed | Stored | Read by |
//! |---|---|---|
//! | Hit Points 10 (3d6) | `resource_data.max_hp` and `current_hp` = 10, `hit_dice` = `"3d6"` | `combat.hitPoints`: damage and healing |
//! | AC 15 | `ability_data.armor_class` | `combat.defence`: what an attack roll must meet |
//! | STR 8 … CHA 8 | `ability_data.strength` … `charisma` | the rules: modifiers, saves, skills |
//! | CR 1/4 | `trait_data.challenge` = `"1/4"` | the rules: proficiency bonus, where a character has `level` |
//! | Small | `trait_data.size` = `"small"` | `combat.sizes`: squares filled, and so reach |
//! | Speed 30 ft., Fly 40 ft. | `trait_data.speed_walk`, `speed_fly`, … (feet) | `movement`: how far a turn goes |
//! | Darkvision 60 ft. | `trait_data.darkvision` (feet) | `vision.darkvision`: what the token sees unlit |
//! | Blindsight, Tremorsense, Truesight | `trait_data.blindsight`, … (feet) | the sheet only; the board has no rule for them |
//! | Stealth +6 | `proficiency_data.skill_proficiencies`, and `skill_expertise` when the bonus is doubled | the rules: the +6 is derived, never stored |
//! | Saving throws | `proficiency_data.saving_throw_proficiencies` | the rules, likewise |
//! | Languages | `proficiency_data.languages` | the sheet |
//! | Type, alignment | `trait_data.creature_type`, `alignment` | the sheet |
//! | Traits, bonus actions, saves-or-suffer | `trait_data.traits`, one line each | the sheet: a Game Master reads them |
//!
//! **An attack is not a field.** It is a world ability attached to the actor,
//! which is what the attack flow rolls (`src/server/src/combat/weapon.rs`):
//!
//! * one `ATTACK_ROLL` effect whose formula is `1d20+4`: the to-hit, as the
//!   block prints it, already holding the proficiency bonus;
//! * one `DAMAGE` effect whose formula is the dice (`1d6+2`), with a second
//!   kind of damage added as further terms (`1d6+2+1d6`), because the flow
//!   rolls the first damage effect only;
//! * attack fields: `reach` in feet for a melee attack (5 is adjacent),
//!   `rangeNormal` / `rangeLong` for a ranged or thrown one;
//! * Multiattack is a third ability whose attack fields name its parts.
//!
//! What a block says that is not arithmetic (a rider on a hit, a recharge, a
//! save) stays prose: in the ability's description or in `traits`. That is a
//! person's to adjudicate, and this product does not adjudicate for them.
//!
//! **Hit points are the printed average**, not a roll. `hit_dice` is kept so a
//! Game Master who wants each goblin different can roll it.
//!
//! # Where the data lives
//!
//! `packs/systems/dnd5e/stat-blocks.json`, beside the manifest, so the web
//! pack (`web/src/StatBlocks.ts`, which does the spreading in the browser) and
//! this crate read one file. The tests here hold that file to the book's own
//! arithmetic and run every block through this pack's validators and rules;
//! a number typed wrong fails one of them.
//!
//! Every block is from the System Reference Document 5.2.1 and is covered by
//! the attribution the manifest carries. A creature that is not in that
//! document is not in this file.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The file itself, compiled in: the blocks are part of the pack, not
/// something to find on a disk at run time.
const STAT_BLOCKS: &str = include_str!("../../stat-blocks.json");

#[derive(Debug, Deserialize)]
struct StatBlockFile {
    blocks: Vec<StatBlock>,
}

/// One creature, as the file has it. Field for field what a printed block
/// says; where each goes is the table at the top of this module.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatBlock {
    pub id: String,
    pub name: String,
    /// The bestiary creature this block answers to, by the bestiary's slug.
    pub bestiary: Option<String>,
    pub size: String,
    pub creature_type: String,
    pub alignment: String,
    pub armor_class: i64,
    /// The average the book prints.
    pub hit_points: i64,
    pub hit_dice: String,
    /// Feet, by mode: `walk`, `fly`, `swim`, `climb`, `burrow`.
    pub speed: BTreeMap<String, i64>,
    pub abilities: BTreeMap<String, i64>,
    #[serde(default)]
    pub saving_throws: Vec<String>,
    /// Skill id to `"proficient"` or `"expertise"`.
    #[serde(default)]
    pub skills: BTreeMap<String, String>,
    /// Feet, by sense: `darkvision`, `blindsight`, `tremorsense`, `truesight`.
    #[serde(default)]
    pub senses: BTreeMap<String, i64>,
    #[serde(default)]
    pub languages: Vec<String>,
    pub challenge: String,
    #[serde(default)]
    pub traits: Vec<StatBlockTrait>,
    pub attacks: Vec<StatBlockAttack>,
    /// The attacks one Multiattack makes, by name, repeated as often as made.
    #[serde(default)]
    pub multiattack: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatBlockTrait {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatBlockAttack {
    pub name: String,
    /// The bonus as printed: ability modifier plus proficiency bonus.
    pub to_hit: i64,
    /// Dice the damage flow can roll: `1d6+2`, or `1d6+2+1d6` for two kinds.
    pub damage: String,
    /// Feet. None for an attack that is only ranged.
    pub reach: Option<i64>,
    /// Normal and long range in feet. None for an attack that is only melee.
    pub range: Option<(i64, i64)>,
    pub text: String,
}

/// Every bundled stat block.
pub fn stat_blocks() -> Vec<StatBlock> {
    serde_json::from_str::<StatBlockFile>(STAT_BLOCKS)
        .expect("stat-blocks.json is compiled in and its tests parse it")
        .blocks
}

/// The four slots a stat block writes, as the validators will be handed
/// them. `spell_data` is left alone: no bundled creature casts from slots.
pub struct StatBlockSlots {
    pub ability_data: Value,
    pub resource_data: Value,
    pub proficiency_data: Value,
    pub trait_data: Value,
}

impl StatBlock {
    /// Spread the block across the slots, by the table at the top of this
    /// module. `web/src/StatBlocks.ts` does the same in the browser, where
    /// the write is made; this is the one the validators are tested against.
    pub fn slots(&self) -> StatBlockSlots {
        let mut ability_data = serde_json::Map::new();
        for (ability, score) in &self.abilities {
            ability_data.insert(ability.clone(), json!(score));
        }
        ability_data.insert("armor_class".to_string(), json!(self.armor_class));

        let skills: Vec<&String> = self.skills.keys().collect();
        let expertise: Vec<&String> = self
            .skills
            .iter()
            .filter(|(_, grade)| grade.as_str() == "expertise")
            .map(|(skill, _)| skill)
            .collect();

        let mut trait_data = serde_json::Map::new();
        trait_data.insert("challenge".to_string(), json!(self.challenge));
        trait_data.insert("creature_type".to_string(), json!(self.creature_type));
        trait_data.insert("alignment".to_string(), json!(self.alignment));
        trait_data.insert("size".to_string(), json!(self.size));
        for (mode, feet) in &self.speed {
            trait_data.insert(format!("speed_{mode}"), json!(feet));
        }
        for (sense, feet) in &self.senses {
            trait_data.insert(sense.clone(), json!(feet));
        }
        trait_data.insert(
            "traits".to_string(),
            json!(self
                .traits
                .iter()
                .map(|t| format!("{}. {}", t.name, t.text))
                .collect::<Vec<_>>()),
        );

        StatBlockSlots {
            ability_data: Value::Object(ability_data),
            resource_data: json!({
                "max_hp": self.hit_points,
                "current_hp": self.hit_points,
                "temporary_hp": 0,
                "hit_dice": self.hit_dice,
            }),
            proficiency_data: json!({
                "skill_proficiencies": skills,
                "skill_expertise": expertise,
                "saving_throw_proficiencies": self.saving_throws,
                "languages": self.languages,
            }),
            trait_data: Value::Object(trait_data),
        }
    }
}

#[cfg(test)]
#[path = "stat_blocks_tests.rs"]
mod tests;
