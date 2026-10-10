//! The sheet's values, read by field name (spec 048 T030).
//!
//! T030 planned label-relative regions, but every value in a D&D Beyond
//! export sits in a named widget field (T011, closed by measurement), so the
//! reader reads each field by its name and keeps the field's box as the
//! source the review points at.

use std::collections::BTreeMap;

use thunderforge_pdf::{Document, Rect};
use thunderforge_sheet_import::{
    ClassLevel, Field, ImportedCharacter, Note, ReadError, Source,
    character::{DeathSaves, HitDice, Spellcasting},
};

use crate::content::spell_level;
use crate::glyphs;

/// One field's value and where it sits.
#[derive(Clone, Debug)]
pub struct Entry {
    pub value: String,
    pub page: u32,
    pub rect: Rect,
}

impl Entry {
    pub fn source(&self) -> Source {
        Source {
            page: self.page,
            rect: self.rect,
            text: self.value.clone(),
        }
    }

    pub fn text(&self) -> &str {
        self.value.trim()
    }
}

/// Every named field on the sheet, by its name with the ends trimmed: the
/// export spells some with a trailing space ("DEXmod ", "Stealth ").
pub struct Sheet {
    fields: BTreeMap<String, Entry>,
}

impl Sheet {
    pub fn read(doc: &Document) -> Sheet {
        let mut fields = BTreeMap::new();
        for page in doc.pages() {
            for field in doc.form_fields(&page) {
                fields
                    .entry(field.name.trim().to_string())
                    .or_insert(Entry {
                        value: field.value,
                        page: field.page,
                        rect: field.rect,
                    });
            }
        }
        Sheet { fields }
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.fields.get(name.trim())
    }

    /// The field, when it holds something.
    pub fn filled(&self, name: &str) -> Option<&Entry> {
        self.get(name).filter(|entry| !entry.text().is_empty())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Entry)> {
        self.fields.iter()
    }

    pub fn string(&self, name: &str) -> Field<String> {
        match self.filled(name) {
            Some(entry) => Field::read(entry.text().to_string(), Some(entry.source())),
            None => Field::unread(),
        }
    }

    pub fn number(&self, name: &str) -> Field<i32> {
        match self.filled(name) {
            Some(entry) => match parse_int(entry.text()) {
                Some(n) => Field::read(n as i32, Some(entry.source())),
                None => not_a_number(entry),
            },
            None => Field::unread(),
        }
    }
}

fn not_a_number<T>(entry: &Entry) -> Field<T> {
    Field::uncertain(
        None,
        format!(
            "\u{201C}{}\u{201D} is not a number this reader can read.",
            entry.text()
        ),
        Some(entry.source()),
    )
}

/// "+3", "-1", "1,215", "6500".
pub fn parse_int(text: &str) -> Option<i64> {
    let cleaned: String = text
        .trim()
        .trim_start_matches('+')
        .chars()
        .filter(|c| *c != ',')
        .collect();
    cleaned.parse().ok()
}

/// The abilities, by neutral key, and their fields on the sheet.
pub const ABILITIES: [(&str, &str, &str, &str, &str); 6] = [
    ("str", "STR", "STRmod", "ST Strength", "StrProf"),
    ("dex", "DEX", "DEXmod", "ST Dexterity", "DexProf"),
    ("con", "CON", "CONmod", "ST Constitution", "ConProf"),
    ("int", "INT", "INTmod", "ST Intelligence", "IntProf"),
    ("wis", "WIS", "WISmod", "ST Wisdom", "WisProf"),
    ("cha", "CHA", "CHamod", "ST Charisma", "ChaProf"),
];

/// Skill key, total field, mark field, and what the sheet calls it.
const SKILLS: [(&str, &str, &str, &str); 18] = [
    ("acrobatics", "Acrobatics", "AcrobaticsProf", "Acrobatics"),
    (
        "animal_handling",
        "Animal",
        "AnimalHandlingProf",
        "Animal Handling",
    ),
    ("arcana", "Arcana", "ArcanaProf", "Arcana"),
    ("athletics", "Athletics", "AthleticsProf", "Athletics"),
    ("deception", "Deception", "DeceptionProf", "Deception"),
    ("history", "History", "HistoryProf", "History"),
    ("insight", "Insight", "InsightProf", "Insight"),
    (
        "intimidation",
        "Intimidation",
        "IntimidationProf",
        "Intimidation",
    ),
    (
        "investigation",
        "Investigation",
        "InvestigationProf",
        "Investigation",
    ),
    ("medicine", "Medicine", "MedicineProf", "Medicine"),
    ("nature", "Nature", "NatureProf", "Nature"),
    ("perception", "Perception", "PerceptionProf", "Perception"),
    (
        "performance",
        "Performance",
        "PerformanceProf",
        "Performance",
    ),
    ("persuasion", "Persuasion", "PersuasionProf", "Persuasion"),
    ("religion", "Religion", "ReligionProf", "Religion"),
    (
        "sleight_of_hand",
        "SleightofHand",
        "SleightOfHandProf",
        "Sleight of Hand",
    ),
    ("stealth", "Stealth", "StealthProf", "Stealth"),
    ("survival", "Survival", "SurvivalProf", "Survival"),
];

/// Persona key and the details page field that holds it.
const PERSONA: [(&str, &str); 15] = [
    ("gender", "GENDER"),
    ("age", "AGE"),
    ("height", "HEIGHT"),
    ("weight", "WEIGHT"),
    ("faith", "FAITH"),
    ("skin", "SKIN"),
    ("eyes", "EYES"),
    ("hair", "HAIR"),
    ("personality_traits", "PersonalityTraits"),
    ("ideals", "Ideals"),
    ("bonds", "Bonds"),
    ("flaws", "Flaws"),
    ("appearance", "Appearance"),
    ("backstory", "Backstory"),
    ("allies_and_organizations", "AlliesOrganizations"),
];

pub const DAMAGE_TYPES: [&str; 13] = [
    "acid",
    "bludgeoning",
    "cold",
    "fire",
    "force",
    "lightning",
    "necrotic",
    "piercing",
    "poison",
    "psychic",
    "radiant",
    "slashing",
    "thunder",
];

const CONDITIONS: [&str; 15] = [
    "blinded",
    "charmed",
    "deafened",
    "exhaustion",
    "frightened",
    "grappled",
    "incapacitated",
    "invisible",
    "paralyzed",
    "petrified",
    "poisoned",
    "prone",
    "restrained",
    "stunned",
    "unconscious",
];

/// Sides of each class's hit die, by the class's name in lower case.
fn class_hit_die(name: &str) -> Option<i32> {
    Some(match name.to_lowercase().as_str() {
        "barbarian" => 12,
        "fighter" | "paladin" | "ranger" => 10,
        "artificer" | "bard" | "cleric" | "druid" | "monk" | "rogue" | "warlock" => 8,
        "sorcerer" | "wizard" => 6,
        _ => return None,
    })
}

/// Read everything but the content rows.
pub fn read_character(sheet: &Sheet) -> Result<ImportedCharacter, ReadError> {
    let mut c = ImportedCharacter::default();
    if ABILITIES.iter().all(|a| sheet.filled(a.1).is_none()) {
        return Err(ReadError::Page {
            page: 1,
            reason: "The ability scores are missing from the sheet, so nothing was read.".into(),
        });
    }

    let id = &mut c.identity;
    id.name = sheet.string("CharacterName");
    id.player_name = sheet.string("PLAYER NAME");
    id.species = sheet.string("RACE");
    id.background = sheet.string("BACKGROUND");
    id.alignment = sheet.string("ALIGNMENT");
    id.size = sheet.string("SIZE");
    id.xp = match sheet.filled("EXPERIENCE POINTS") {
        Some(entry) => match parse_int(entry.text()) {
            Some(xp) => Field::read(xp, Some(entry.source())),
            // "(Milestone)": read, and no number to hold.
            None if entry.text().to_lowercase().contains("milestone") => {
                Field::read_empty(Some(entry.source()))
            }
            None => not_a_number(entry),
        },
        None => Field::unread(),
    };
    c.classes = sheet
        .filled("CLASS  LEVEL")
        .map(classes)
        .unwrap_or_default();

    for (key, score, modifier, save, mark) in ABILITIES {
        c.abilities.insert(key.into(), sheet.number(score));
        c.derived
            .insert(format!("modifier.{key}"), sheet.number(modifier));
        c.derived.insert(format!("save.{key}"), sheet.number(save));
        if let Some(entry) = sheet.get(mark) {
            let field = match glyphs::save_mark(&entry.value) {
                Ok(proficient) => Field::read(proficient, Some(entry.source())),
                Err(glyph) => Field::uncertain(
                    None,
                    glyphs::unknown_mark(&format!("the {} save", key.to_uppercase()), &glyph),
                    Some(entry.source()),
                ),
            };
            c.proficiencies.saves.insert(key.into(), field);
        }
    }
    for (key, total, mark, label) in SKILLS {
        c.derived
            .insert(format!("skill.{key}"), sheet.number(total));
        if let Some(entry) = sheet.get(mark) {
            let field = match glyphs::skill_mark(&entry.value) {
                Ok(mark) => Field::read(mark, Some(entry.source())),
                Err(glyph) => Field::uncertain(
                    None,
                    glyphs::unknown_mark(label, &glyph),
                    Some(entry.source()),
                ),
            };
            c.proficiencies.skills.insert(key.into(), field);
        }
    }
    for (key, field) in [
        ("passive.perception", "Passive1"),
        ("passive.insight", "Passive2"),
        ("passive.investigation", "Passive3"),
        ("initiative", "Init"),
        ("proficiency_bonus", "ProfBonus"),
    ] {
        c.derived.insert(key.into(), sheet.number(field));
    }

    proficiency_lists(sheet, &mut c);
    defences(sheet, &mut c);
    resources(sheet, &mut c);
    movement(sheet, &mut c);
    c.spellcasting = spellcasting(sheet);

    for (key, field) in PERSONA {
        if sheet.filled(field).is_some() {
            c.persona.insert(key.into(), sheet.string(field));
        }
    }
    notes(sheet, &mut c);
    Ok(c)
}

/// "Fighter 3 / Wizard 2": one entry per class.
fn classes(entry: &Entry) -> Vec<ClassLevel> {
    let source = Some(entry.source());
    entry
        .text()
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, level) = match part.rsplit_once(' ') {
                Some((name, n)) if n.parse::<i32>().is_ok() => (
                    name.trim(),
                    Field::read(n.parse().unwrap_or(0), source.clone()),
                ),
                _ => (
                    part,
                    Field::uncertain(
                        None,
                        format!("The class line names {part} with no level. Enter its level."),
                        source.clone(),
                    ),
                ),
            };
            let hit_die = match class_hit_die(name) {
                Some(die) => Field::read(die, source.clone()),
                None => Field::uncertain(
                    None,
                    format!(
                        "{name} is not a class this reader knows, so its hit die is not known."
                    ),
                    source.clone(),
                ),
            };
            ClassLevel {
                name: Field::read(name.to_string(), source.clone()),
                subclass: Field::unread(),
                level,
                hit_die,
            }
        })
        .collect()
}

/// The `=== HEADING ===` blocks of a text field, in order.
fn sections(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(heading) = line
            .strip_prefix("===")
            .and_then(|rest| rest.strip_suffix("==="))
        {
            out.push((heading.trim().to_string(), Vec::new()));
        } else if !line.is_empty() {
            if out.is_empty() {
                out.push((String::new(), Vec::new()));
            }
            out.last_mut().expect("pushed").1.push(line.to_string());
        }
    }
    out
}

fn list(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

fn proficiency_lists(sheet: &Sheet, c: &mut ImportedCharacter) {
    let Some(entry) = sheet.filled("ProficienciesLang") else {
        return;
    };
    let p = &mut c.proficiencies;
    for list_field in [
        &mut p.armour,
        &mut p.weapons,
        &mut p.tools,
        &mut p.languages,
    ] {
        *list_field = Field::read(Vec::new(), Some(entry.source()));
    }
    for (heading, lines) in sections(&entry.value) {
        let target = match heading.to_uppercase().as_str() {
            "ARMOR" | "ARMOUR" => &mut p.armour,
            "WEAPONS" => &mut p.weapons,
            "TOOLS" => &mut p.tools,
            "LANGUAGES" => &mut p.languages,
            _ => {
                c.notes.push(note(
                    if heading.is_empty() {
                        "Proficiencies".into()
                    } else {
                        heading.clone()
                    },
                    lines.join("\n"),
                    entry,
                ));
                continue;
            }
        };
        *target = Field::read(list(&lines), Some(entry.source()));
    }
}

fn note(label: String, text: String, entry: &Entry) -> Note {
    Note {
        label,
        text: Field::read(text, Some(entry.source())),
    }
}

/// "Poison - Resistance", one per line or comma.
fn defences(sheet: &Sheet, c: &mut ImportedCharacter) {
    c.defences.armour_class = sheet.number("AC");
    let Some(entry) = sheet.get("Defenses") else {
        return;
    };
    let mut lists: [Vec<String>; 4] = Default::default();
    let mut unknown = Vec::new();
    for part in entry.value.split(['\n', ',']).map(str::trim) {
        if part.is_empty() {
            continue;
        }
        let parsed = part.rsplit_once(" - ").and_then(|(name, kind)| {
            let name = name.trim().to_lowercase();
            let kind = kind.trim().to_lowercase();
            let damage = DAMAGE_TYPES.contains(&name.as_str());
            let condition = CONDITIONS.contains(&name.as_str());
            let slot = match kind.as_str() {
                "resistance" | "resistances" if damage => 0,
                "immunity" | "immunities" if damage => 1,
                "vulnerability" | "vulnerabilities" if damage => 2,
                "immunity" | "immunities" if condition => 3,
                _ => return None,
            };
            Some((slot, name))
        });
        match parsed {
            Some((slot, name)) if !lists[slot].contains(&name) => lists[slot].push(name),
            Some(_) => {}
            None => unknown.push(part.to_string()),
        }
    }
    let [resistances, immunities, vulnerabilities, conditions] = lists;
    let d = &mut c.defences;
    d.resistances = Field::read(resistances, Some(entry.source()));
    d.immunities = Field::read(immunities, Some(entry.source()));
    d.vulnerabilities = Field::read(vulnerabilities, Some(entry.source()));
    d.condition_immunities = Field::read(conditions, Some(entry.source()));
    if !unknown.is_empty() {
        c.notes
            .push(note("Defenses".into(), unknown.join("\n"), entry));
    }
}

fn checked(entry: &Entry) -> bool {
    !matches!(entry.text(), "" | "Off")
}

fn resources(sheet: &Sheet, c: &mut ImportedCharacter) {
    let r = &mut c.resources;
    r.hp_max = sheet.number("MaxHP");
    // D&D Beyond prints the current hit points blank; blank stays unread.
    r.hp_current = sheet.number("CurrentHP");
    r.hp_temp = match sheet.filled("TempHP") {
        Some(entry) if entry.text() == "--" => Field::read(0, Some(entry.source())),
        _ => sheet.number("TempHP"),
    };
    if let Some(entry) = sheet.filled("Total") {
        for part in entry.text().split('+').map(str::trim) {
            let parsed = part.split_once('d').and_then(|(n, die)| {
                Some((
                    n.trim().parse::<i32>().ok()?,
                    die.trim().parse::<i32>().ok()?,
                ))
            });
            r.hit_dice.push(match parsed {
                Some((total, die)) => HitDice {
                    die: Field::read(die, Some(entry.source())),
                    total: Field::read(total, Some(entry.source())),
                    used: Field::unread(),
                },
                None => HitDice {
                    die: Field::uncertain(
                        None,
                        format!("\u{201C}{part}\u{201D} is not a hit dice count like 3d10."),
                        Some(entry.source()),
                    ),
                    total: Field::unread(),
                    used: Field::unread(),
                },
            });
        }
    }
    // Measured: the top row of boxes is SUCCESSES, the bottom FAILURES.
    let count = |names: [&str; 3]| -> Field<i32> {
        let boxes: Vec<&Entry> = names.iter().filter_map(|n| sheet.get(n)).collect();
        match boxes.first() {
            Some(first) => Field::read(
                boxes.iter().filter(|b| checked(b)).count() as i32,
                Some(first.source()),
            ),
            None => Field::unread(),
        }
    };
    r.death_saves = DeathSaves {
        successes: count(["Check Box 12", "Check Box 13", "Check Box 14"]),
        failures: count(["Check Box 15", "Check Box 16", "Check Box 17"]),
    };
    if let Some(entry) = sheet.get("Inspiration") {
        r.inspiration = Field::read(checked(entry), Some(entry.source()));
    }
    for coin in ["cp", "sp", "ep", "gp", "pp"] {
        if let Some(entry) = sheet.filled(&coin.to_uppercase()) {
            let field = match parse_int(entry.text()) {
                Some(n) if n >= 0 => Field::read(n, Some(entry.source())),
                _ => not_a_number(entry),
            };
            r.coins.insert(coin.into(), field);
        }
    }
}

/// "30 ft. (Walking)", "Darkvision 60 ft.".
fn movement(sheet: &Sheet, c: &mut ImportedCharacter) {
    if let Some(entry) = sheet.filled("Speed") {
        let mut unread = Vec::new();
        for part in entry.text().split(',').map(str::trim) {
            let feet = part
                .split_whitespace()
                .next()
                .and_then(|n| n.parse::<i32>().ok());
            let mode = part
                .split_once('(')
                .map(|(_, rest)| rest.trim_end_matches(')').trim().to_lowercase());
            let key = match mode.as_deref() {
                None | Some("walking") => Some("walk"),
                Some("flying") => Some("fly"),
                Some("swimming") => Some("swim"),
                Some("climbing") => Some("climb"),
                Some("burrowing") => Some("burrow"),
                Some(_) => None,
            };
            match (key, feet) {
                (Some(key), Some(feet)) => {
                    c.movement
                        .speeds
                        .insert(key.into(), Field::read(feet, Some(entry.source())));
                }
                _ => unread.push(part.to_string()),
            }
        }
        if !unread.is_empty() {
            c.notes.push(note("Speed".into(), unread.join(", "), entry));
        }
    }
    if let Some(entry) = sheet.filled("AdditionalSenses") {
        let mut unread = Vec::new();
        for part in entry.text().split(',').map(str::trim) {
            let words: Vec<&str> = part.split_whitespace().collect();
            let at = words.iter().position(|w| w.parse::<i32>().is_ok());
            let sense = at.map(|at| words[..at].join("_").to_lowercase());
            match (sense.as_deref(), at) {
                (
                    Some(sense @ ("darkvision" | "blindsight" | "tremorsense" | "truesight")),
                    Some(at),
                ) => {
                    let feet = words[at].parse().unwrap_or(0);
                    c.movement
                        .senses
                        .insert(sense.into(), Field::read(feet, Some(entry.source())));
                }
                _ => unread.push(part.to_string()),
            }
        }
        if !unread.is_empty() {
            c.notes
                .push(note("Senses".into(), unread.join(", "), entry));
        }
    }
}

/// A character casting from two or more classes: the export prints them in
/// the one column, "Cleric / Wizard" over "WIS / INT", "15 / 13" and
/// "+7 / +5". Each part is a caster of its own, read from the same box.
/// `None` when the column holds one class, or its parts do not line up.
fn casting_columns(sheet: &Sheet, index: usize) -> Option<Vec<Spellcasting>> {
    let parts = |name: &str| -> Option<(Vec<String>, &Entry)> {
        let entry = sheet.filled(&format!("{name}{index}"))?;
        let parts = entry.text().split('/').map(|p| p.trim().to_string());
        Some((parts.collect(), entry))
    };
    let (classes, class_entry) = parts("spellCastingClass")?;
    if classes.len() < 2 || classes.iter().any(String::is_empty) {
        return None;
    }
    let lined_up = |name: &str| parts(name).filter(|(p, _)| p.len() == classes.len());
    let (abilities, ability_entry) = lined_up("spellCastingAbility")?;
    let (dcs, dc_entry) = lined_up("spellSaveDC")?;
    let (attacks, attack_entry) = lined_up("spellAtkBonus")?;
    let number = |text: &str, entry: &Entry| match parse_int(text) {
        Some(n) => Field::read(n as i32, Some(entry.source())),
        None => not_a_number(entry),
    };
    Some(
        (0..classes.len())
            .map(|i| Spellcasting {
                class: Field::read(classes[i].clone(), Some(class_entry.source())),
                ability: Field::read(abilities[i].to_lowercase(), Some(ability_entry.source())),
                save_dc: number(&dcs[i], dc_entry),
                attack_bonus: number(&attacks[i], attack_entry),
                slots: BTreeMap::new(),
                pact_slots: None,
            })
            .collect(),
    )
}

fn spellcasting(sheet: &Sheet) -> Vec<Spellcasting> {
    let mut out = Vec::new();
    for index in 0.. {
        if sheet.filled(&format!("spellCastingClass{index}")).is_none() {
            break;
        }
        if let Some(split) = casting_columns(sheet, index) {
            out.extend(split);
            continue;
        }
        let class = sheet.string(&format!("spellCastingClass{index}"));
        let ability = sheet.string(&format!("spellCastingAbility{index}"));
        let save_dc = sheet.number(&format!("spellSaveDC{index}"));
        let attack_bonus = sheet.number(&format!("spellAtkBonus{index}"));
        out.push(Spellcasting {
            class,
            ability: Field {
                value: ability.value.map(|a| a.to_lowercase()),
                ..ability
            },
            save_dc,
            attack_bonus,
            slots: BTreeMap::new(),
            pact_slots: None,
        });
    }
    // The slot counts head each level of the one spell table, so they belong
    // to the character, and are kept on the first casting class.
    if let Some(first) = out.first_mut() {
        for level in 0.. {
            let Some(header) = sheet.get(&format!("spellHeader{level}")) else {
                break;
            };
            let Some(spell_level) = spell_level(header.text()).filter(|l| *l > 0) else {
                continue;
            };
            if let Some(entry) = sheet.filled(&format!("spellSlotHeader{level}")) {
                let count = entry
                    .text()
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<i32>().ok());
                first.slots.insert(
                    spell_level.to_string(),
                    match count {
                        Some(n) => Field::read(n, Some(entry.source())),
                        None => Field::uncertain(
                            None,
                            format!(
                                "\u{201C}{}\u{201D} does not say how many slots there are.",
                                entry.text()
                            ),
                            Some(entry.source()),
                        ),
                    },
                );
            }
        }
    }
    out
}

fn notes(sheet: &Sheet, c: &mut ImportedCharacter) {
    let joined = |names: &[&str]| -> Option<(String, &Entry)> {
        let entries: Vec<&Entry> = names.iter().filter_map(|n| sheet.filled(n)).collect();
        let first = *entries.first()?;
        let text = entries
            .iter()
            .map(|e| e.text())
            .collect::<Vec<_>>()
            .join("\n\n");
        Some((text, first))
    };
    let mut front = Vec::new();
    for (label, names) in [
        ("Actions", &["Actions1", "Actions2"][..]),
        ("Saving throw modifiers", &["SaveModifiers"][..]),
        (
            "Additional notes",
            &["AdditionalNotes1", "AdditionalNotes2"][..],
        ),
    ] {
        if let Some((text, entry)) = joined(names) {
            front.push(note(label.into(), text, entry));
        }
    }
    front.append(&mut c.notes);
    c.notes = front;
}
