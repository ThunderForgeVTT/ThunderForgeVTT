//! The character a reader produces (spec 048 FR-005).
//!
//! Every leaf is a [`Field`]: the value, how sure the reader is of it, and
//! where on the page it came from. The shape is system-neutral. A 5e sheet
//! and a Roll for Shoes sheet both fit it, and what has no neutral home goes
//! into [`ImportedCharacter::notes`] rather than being dropped (FR-013).
//!
//! The server deserialises a reading the browser sent, so every struct here
//! refuses unknown keys. A missing key is its default: unread.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thunderforge_pdf::Rect;

/// How sure a reader is of one value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Certainty {
    /// Read plainly from the sheet.
    Read,
    /// Read, but something about it is doubtful. The reason is shown to the
    /// person, who confirms or corrects it.
    Uncertain { reason: String },
    /// Not on the sheet, or not readable. Never written.
    #[default]
    Unread,
}

/// Where on the sheet a value came from, so the review can point at it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// One-based.
    pub page: u32,
    pub rect: Rect,
    /// The text as the sheet holds it, before any parsing.
    pub text: String,
}

/// One value read from a sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field<T> {
    pub value: Option<T>,
    #[serde(default)]
    pub certainty: Certainty,
    #[serde(default)]
    pub source: Option<Source>,
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Field::unread()
    }
}

impl<T> Field<T> {
    pub fn read(value: T, source: Option<Source>) -> Self {
        Field {
            value: Some(value),
            certainty: Certainty::Read,
            source,
        }
    }

    pub fn uncertain(value: Option<T>, reason: impl Into<String>, source: Option<Source>) -> Self {
        Field {
            value,
            certainty: Certainty::Uncertain {
                reason: reason.into(),
            },
            source,
        }
    }

    pub fn unread() -> Self {
        Field {
            value: None,
            certainty: Certainty::Unread,
            source: None,
        }
    }

    /// Read, but the sheet printed nothing: "Milestone" experience, say.
    pub fn read_empty(source: Option<Source>) -> Self {
        Field {
            value: None,
            certainty: Certainty::Read,
            source,
        }
    }
}

/// Which reader produced a reading. The plan carries it, so a reviewed plan
/// names the reader version the server must match.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderStamp {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Identity {
    pub name: Field<String>,
    /// Shown as read and ignored: it never identifies or authorises anyone.
    pub player_name: Field<String>,
    pub species: Field<String>,
    pub background: Field<String>,
    pub alignment: Field<String>,
    pub size: Field<String>,
    pub xp: Field<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassLevel {
    pub name: Field<String>,
    pub subclass: Field<String>,
    pub level: Field<i32>,
    /// Sides of the class's hit die: 10 for a d10.
    pub hit_die: Field<i32>,
}

/// How well a character knows a skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMark {
    None,
    Half,
    Proficient,
    Expertise,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Proficiencies {
    /// Neutral skill key, such as "perception", to the mark beside it.
    pub skills: BTreeMap<String, Field<SkillMark>>,
    /// Neutral ability key, such as "str", to whether its save is proficient.
    pub saves: BTreeMap<String, Field<bool>>,
    pub armour: Field<Vec<String>>,
    pub weapons: Field<Vec<String>>,
    pub tools: Field<Vec<String>>,
    pub languages: Field<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Defences {
    pub armour_class: Field<i32>,
    pub resistances: Field<Vec<String>>,
    pub immunities: Field<Vec<String>>,
    pub vulnerabilities: Field<Vec<String>>,
    pub condition_immunities: Field<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HitDice {
    pub die: Field<i32>,
    pub total: Field<i32>,
    pub used: Field<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeathSaves {
    pub successes: Field<i32>,
    pub failures: Field<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Resources {
    pub hp_max: Field<i32>,
    pub hp_current: Field<i32>,
    pub hp_temp: Field<i32>,
    pub hit_dice: Vec<HitDice>,
    pub death_saves: DeathSaves,
    /// Coin key, such as "gp", to the count.
    pub coins: BTreeMap<String, Field<i64>>,
    pub inspiration: Field<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Movement {
    /// "walk", "fly", "swim", "climb", "burrow": feet.
    pub speeds: BTreeMap<String, Field<i32>>,
    /// "darkvision", "blindsight", ...: range in feet.
    pub senses: BTreeMap<String, Field<i32>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PactSlots {
    pub level: Field<i32>,
    pub total: Field<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Spellcasting {
    pub class: Field<String>,
    pub ability: Field<String>,
    pub save_dc: Field<i32>,
    pub attack_bonus: Field<i32>,
    /// Slot level ("1" to "9") to the number of slots.
    pub slots: BTreeMap<String, Field<i32>>,
    pub pact_slots: Option<PactSlots>,
}

/// How a piece of content sits on this character.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContentLink {
    pub prepared: Option<bool>,
    /// What grants it: a class, a species, a feat. Sorted, no repeats.
    pub granted_by: Vec<String>,
    pub uses: Option<i32>,
    pub recharge: Option<String>,
    pub equipped: Option<bool>,
    pub attuned: Option<bool>,
    pub quantity: Option<i32>,
}

/// A spell, feature, item or other named thing the sheet lists.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NamedContent {
    /// A neutral kind: "spell", "feat", "feature", "species_trait", "item",
    /// "attack". The system's declaration says where each kind lands.
    pub kind: String,
    pub name: Field<String>,
    /// What the sheet says about the thing itself, not about this character:
    /// a spell's range, an item's weight. This is what content is compared by.
    pub fields: BTreeMap<String, Value>,
    pub link: ContentLink,
}

/// Something read that has no neutral home. It is kept, labelled.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Note {
    /// What the sheet calls it: "Immune to disease", "Encumbrance".
    pub label: String,
    pub text: Field<String>,
}

/// A system-neutral character, read from a sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportedCharacter {
    pub reader: ReaderStamp,
    pub identity: Identity,
    pub classes: Vec<ClassLevel>,
    /// Neutral ability key ("str", "dex", ...) to the score.
    pub abilities: BTreeMap<String, Field<i32>>,
    pub proficiencies: Proficiencies,
    pub defences: Defences,
    pub resources: Resources,
    pub movement: Movement,
    pub spellcasting: Vec<Spellcasting>,
    pub content: Vec<NamedContent>,
    /// What the sheet printed that the rules derive: "skill.perception",
    /// "passive.perception", "initiative". Never written; checked.
    pub derived: BTreeMap<String, Field<i32>>,
    /// Appearance, personality and story, by neutral key: "age",
    /// "personality_traits", "backstory".
    pub persona: BTreeMap<String, Field<String>>,
    pub notes: Vec<Note>,
}

/// One value-bearing place in a reading, by neutral path.
#[derive(Clone, Debug, PartialEq)]
pub struct Leaf {
    /// "abilities.str", "classes.0.level", "content.3", "notes.1".
    pub path: String,
    pub value: Option<Value>,
    pub certainty: Certainty,
    pub source: Option<Source>,
}

impl ImportedCharacter {
    /// Every leaf, in path order within each part.
    ///
    /// A piece of content and a note are one leaf each: they are planned
    /// whole. A derived value's path is `derived.` and its key.
    pub fn leaves(&self) -> Vec<Leaf> {
        let mut out = Vec::new();
        let tree = serde_json::to_value(self).expect("a reading always serialises");
        let Value::Object(parts) = tree else {
            return out;
        };
        for (part, value) in &parts {
            match part.as_str() {
                "reader" => {}
                "content" => {
                    for (index, item) in self.content.iter().enumerate() {
                        out.push(Leaf {
                            path: format!("content.{index}"),
                            value: Some(serde_json::to_value(item).expect("content serialises")),
                            certainty: item.name.certainty.clone(),
                            source: item.name.source.clone(),
                        });
                    }
                }
                "notes" => {
                    for (index, note) in self.notes.iter().enumerate() {
                        out.push(Leaf {
                            path: format!("notes.{index}"),
                            value: note.text.value.clone().map(Value::String),
                            certainty: note.text.certainty.clone(),
                            source: note.text.source.clone(),
                        });
                    }
                }
                _ => walk(part, value, &mut out),
            }
        }
        out
    }
}

/// Whether `node` is a serialised [`Field`]. Only a Field has a
/// `certainty` key; no other struct here uses that name.
pub(crate) fn is_field(node: &Value) -> bool {
    node.as_object()
        .is_some_and(|map| map.contains_key("certainty") && map.contains_key("value"))
}

fn walk(path: &str, node: &Value, out: &mut Vec<Leaf>) {
    if is_field(node) {
        out.push(Leaf {
            path: path.to_string(),
            value: node.get("value").filter(|v| !v.is_null()).cloned(),
            certainty: serde_json::from_value(node["certainty"].clone()).unwrap_or_default(),
            source: node
                .get("source")
                .and_then(|s| serde_json::from_value(s.clone()).ok()),
        });
        return;
    }
    match node {
        Value::Object(map) => {
            for (key, child) in map {
                walk(&format!("{path}.{key}"), child, out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                walk(&format!("{path}.{index}"), child, out);
            }
        }
        // A bare value outside a Field (a pact slot's absence is null).
        _ => {}
    }
}
