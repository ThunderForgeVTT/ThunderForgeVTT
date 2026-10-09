//! The fixture generator (spec 048 T024, research R17).
//!
//! It lays out invented characters on D&D Beyond's character sheet: the
//! labels and field boxes in `ddb-layout.json` were measured from real exports
//! with `probe --layout`, and every value here is made up. Like a real export,
//! the values live in widget fields (`/V`) with no appearance stream and no
//! `/AcroForm` in the catalog, and the page content draws only the labels.
//!
//! Generation is deterministic: no timestamps, no `/Info`, a fixed `/ID`, and
//! objects numbered in the order they are written. `fixtures.rs` pins each
//! file's sha256, so a change here is a visible diff.

use std::collections::{BTreeMap, BTreeSet};

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use serde::Deserialize;

const LAYOUT: &str = include_str!("ddb-layout.json");

#[derive(Deserialize)]
struct Layout {
    pages: BTreeMap<String, LayoutPage>,
    spell_rows: SpellRows,
}

#[derive(Deserialize)]
struct LayoutPage {
    width: f64,
    height: f64,
    /// text, x, y, size, bold
    runs: Vec<(String, f64, f64, f64, bool)>,
    /// name, "Tx" or "Btn", box in PDF space
    fields: Vec<(String, String, [f64; 4])>,
}

#[derive(Deserialize)]
struct SpellRows {
    top: f64,
    pitch: f64,
    height: f64,
    count: usize,
    /// column, x, width
    columns: Vec<(String, f64, f64)>,
}

/// Ability order on the sheet: STR, DEX, CON, INT, WIS, CHA.
const STR: usize = 0;
const DEX: usize = 1;
const INT: usize = 3;
const WIS: usize = 4;
const CHA: usize = 5;

const ABILITIES: [(&str, &str, &str, &str, &str); 6] = [
    // score, modifier, save, save mark, abbreviation
    ("STR", "STRmod", "ST Strength", "StrProf", "STR"),
    ("DEX", "DEXmod ", "ST Dexterity", "DexProf", "DEX"),
    ("CON", "CONmod", "ST Constitution", "ConProf", "CON"),
    ("INT", "INTmod", "ST Intelligence", "IntProf", "INT"),
    ("WIS", "WISmod", "ST Wisdom", "WisProf", "WIS"),
    ("CHA", "CHamod", "ST Charisma", "ChaProf", "CHA"),
];

/// A skill's name as a fixture says it, its total field, its ability field,
/// its mark field, and its ability. The field names are D&D Beyond's, odd
/// spellings and trailing spaces included.
const SKILLS: [(&str, &str, &str, &str, usize); 18] = [
    (
        "acrobatics",
        "Acrobatics",
        "AcrobaticsMod",
        "AcrobaticsProf",
        DEX,
    ),
    (
        "animal_handling",
        "Animal",
        "AnimalMod",
        "AnimalHandlingProf",
        WIS,
    ),
    ("arcana", "Arcana", "ArcanaMod", "ArcanaProf", INT),
    (
        "athletics",
        "Athletics",
        "AthleticsMod",
        "AthleticsProf",
        STR,
    ),
    (
        "deception",
        "Deception",
        "DeceptionMod",
        "DeceptionProf",
        CHA,
    ),
    ("history", "History", "HistoryMod", "HistoryProf", INT),
    ("insight", "Insight", "InsightMod", "InsightProf", WIS),
    (
        "intimidation",
        "Intimidation",
        "IntimidationMod",
        "IntimidationProf",
        CHA,
    ),
    (
        "investigation",
        "Investigation",
        "InvestigationMod",
        "InvestigationProf",
        INT,
    ),
    ("medicine", "Medicine", "MedicineMod", "MedicineProf", WIS),
    ("nature", "Nature", "NatureMod", "NatureProf", INT),
    (
        "perception",
        "Perception",
        "PerceptionMod",
        "PerceptionProf",
        WIS,
    ),
    (
        "performance",
        "Performance",
        "PerformanceMod",
        "PerformanceProf",
        CHA,
    ),
    (
        "persuasion",
        "Persuasion",
        "PersuasionMod",
        "PersuasionProf",
        CHA,
    ),
    ("religion", "Religion", "ReligionMod", "ReligionProf", INT),
    (
        "sleight_of_hand",
        "SleightofHand",
        "SleightofHandMod",
        "SleightOfHandProf",
        DEX,
    ),
    ("stealth", "Stealth ", "StealthMod", "StealthProf", DEX),
    ("survival", "Survival", "SurvivalMod", "SurvivalProf", WIS),
];

/// Each weapon row's fields, spaces as D&D Beyond spells them.
const WEAPON_ROWS: [(&str, &str, &str, &str); 6] = [
    ("Wpn Name", "Wpn1 AtkBonus", "Wpn1 Damage", "Wpn Notes 1"),
    (
        "Wpn Name 2",
        "Wpn2 AtkBonus ",
        "Wpn2 Damage ",
        "Wpn Notes 2",
    ),
    (
        "Wpn Name 3",
        "Wpn3 AtkBonus  ",
        "Wpn3 Damage ",
        "Wpn Notes 3",
    ),
    ("Wpn Name 4", "Wpn4 AtkBonus", "Wpn4 Damage", "Wpn Notes 4"),
    ("Wpn Name 5", "Wpn5 AtkBonus", "Wpn5 Damage", "Wpn Notes 5"),
    ("Wpn Name 6", "Wpn6 AtkBonus", "Wpn6 Damage", "Wpn Notes 6"),
];

/// Equipment rows on the features page; the rest go on the overflow page.
const EQUIPMENT_ROWS_PER_PAGE: usize = 26;

/// One weapon or attack row: name, to-hit, damage, notes.
pub type Weapon = (&'static str, &'static str, &'static str, &'static str);
/// One equipment row: name, quantity, weight.
pub type Item = (&'static str, &'static str, &'static str);

#[derive(Clone, Default)]
pub struct Persona {
    pub gender: &'static str,
    pub age: &'static str,
    pub height: &'static str,
    pub weight: &'static str,
    pub faith: &'static str,
    pub skin: &'static str,
    pub eyes: &'static str,
    pub hair: &'static str,
    pub personality: &'static str,
    pub ideals: &'static str,
    pub bonds: &'static str,
    pub flaws: &'static str,
    pub appearance: &'static str,
    pub backstory: &'static str,
    pub allies: &'static str,
    pub notes: &'static str,
}

/// One row of the spell table.
#[derive(Clone, Copy)]
pub struct Spell {
    /// "P" prepared, "O" known but not prepared.
    pub prepared: &'static str,
    pub name: &'static str,
    /// The class or feature that grants it.
    pub source: &'static str,
    pub save_hit: &'static str,
    pub time: &'static str,
    pub range: &'static str,
    pub components: &'static str,
    pub duration: &'static str,
    pub page: &'static str,
    pub notes: &'static str,
}

#[derive(Clone)]
pub struct SpellLevel {
    /// "=== CANTRIPS ===", "=== 1st LEVEL ===".
    pub header: &'static str,
    /// "(At Will)", "4 Slots OOOO".
    pub slots: &'static str,
    pub spells: Vec<Spell>,
}

#[derive(Clone)]
pub struct Spellcasting {
    pub class: &'static str,
    pub ability: &'static str,
    pub save_dc: &'static str,
    pub attack: &'static str,
    pub levels: Vec<SpellLevel>,
}

/// An invented character, in the terms the sheet prints. Totals the sheet
/// derives (modifiers, saves, skills, passives, initiative, proficiency
/// bonus) are computed from these, so a fixture is consistent unless it
/// deliberately is not (`overrides`).
#[derive(Clone)]
pub struct Character {
    pub file: &'static str,
    pub name: &'static str,
    pub player: &'static str,
    pub class_level: &'static str,
    /// The total level, for the proficiency bonus.
    pub level: i32,
    pub species: &'static str,
    pub background: &'static str,
    pub alignment: &'static str,
    pub size: &'static str,
    pub xp: &'static str,
    pub scores: [i32; 6],
    pub save_proficient: [bool; 6],
    /// Skill to mark: 'P' proficient, 'E' expertise, 'H' half.
    pub skills: Vec<(&'static str, char)>,
    pub armour_class: i32,
    pub max_hp: i32,
    pub speed: &'static str,
    /// The hit dice line: "3d10 + 2d6".
    pub hit_dice: &'static str,
    pub senses: &'static str,
    pub defenses: &'static str,
    pub save_modifiers: &'static str,
    pub proficiencies: &'static str,
    pub weapons: Vec<Weapon>,
    /// The two ACTIONS columns.
    pub actions: [&'static str; 2],
    /// FEATURES & TRAITS columns; more than three adds the overflow page.
    pub features: Vec<&'static str>,
    /// CP, SP, EP, GP, PP.
    pub coins: [&'static str; 5],
    pub weight_carried: &'static str,
    pub equipment: Vec<Item>,
    pub attuned: Vec<&'static str>,
    pub persona: Persona,
    pub spells: Option<Spellcasting>,
    /// Raw field values set last, for a fixture that must be inconsistent.
    pub overrides: Vec<(&'static str, &'static str)>,
}

fn modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

fn signed(value: i32) -> String {
    if value < 0 {
        format!("{value}")
    } else {
        format!("+{value}")
    }
}

fn proficiency_bonus(level: i32) -> i32 {
    2 + (level - 1).max(0) / 4
}

/// Every field value the character fills, by exact field name.
pub fn field_values(c: &Character) -> BTreeMap<String, String> {
    let mut v: BTreeMap<String, String> = BTreeMap::new();
    let mut set = |name: &str, value: &str| {
        if !value.is_empty() {
            v.insert(name.to_string(), value.to_string());
        }
    };

    // The header every page repeats, each copy with its own suffix.
    for suffix in ["", "2", "3", "4"] {
        set(&format!("CharacterName{suffix}"), c.name);
    }
    for suffix in ["", "2", "3"] {
        set(&format!("CLASS  LEVEL{suffix}"), c.class_level);
        set(&format!("PLAYER NAME{suffix}"), c.player);
        set(&format!("RACE{suffix}"), c.species);
        set(&format!("BACKGROUND{suffix}"), c.background);
        set(&format!("EXPERIENCE POINTS{suffix}"), c.xp);
    }

    let pb = proficiency_bonus(c.level);
    let mods: Vec<i32> = c.scores.iter().map(|s| modifier(*s)).collect();
    for (index, (score, modf, save, mark, _)) in ABILITIES.iter().enumerate() {
        set(score, &c.scores[index].to_string());
        set(modf, &signed(mods[index]));
        let proficient = c.save_proficient[index];
        set(save, &signed(mods[index] + if proficient { pb } else { 0 }));
        set(mark, if proficient { "\u{2022}" } else { "" });
    }

    let mut totals = BTreeMap::new();
    for (key, total, ability_field, mark_field, ability) in SKILLS {
        let mark = c
            .skills
            .iter()
            .find(|(skill, _)| *skill == key)
            .map(|(_, mark)| *mark);
        let bonus = match mark {
            Some('P') => pb,
            Some('E') => 2 * pb,
            Some('H') => pb / 2,
            _ => 0,
        };
        let value = mods[ability] + bonus;
        totals.insert(key, value);
        set(total, &signed(value));
        set(ability_field, ABILITIES[ability].4);
        if let Some(mark) = mark {
            set(mark_field, &mark.to_string());
        }
    }
    set("Passive1", &(10 + totals["perception"]).to_string());
    set("Passive2", &(10 + totals["insight"]).to_string());
    set("Passive3", &(10 + totals["investigation"]).to_string());

    set("ProfBonus", &signed(pb));
    set("Init", &signed(mods[DEX]));
    set("AC", &c.armour_class.to_string());
    set("MaxHP", &c.max_hp.to_string());
    // D&D Beyond prints the current hit points blank and temporary ones as
    // a dash; the measured exports all do.
    set("TempHP", "--");
    set("Speed", c.speed);
    set("Total", c.hit_dice);
    set("AdditionalSenses", c.senses);
    set("Defenses", c.defenses);
    set("SaveModifiers", c.save_modifiers);
    set("ProficienciesLang", c.proficiencies);
    set("Actions1", c.actions[0]);
    set("Actions2", c.actions[1]);
    for (row, (name, hit, damage, notes)) in WEAPON_ROWS.iter().zip(&c.weapons) {
        set(row.0, name);
        set(row.1, hit);
        set(row.2, damage);
        set(row.3, notes);
    }

    for (index, column) in c.features.iter().enumerate() {
        set(&format!("FeaturesTraits{}", index + 1), column);
    }
    for (name, value) in ["CP", "SP", "EP", "GP", "PP"].iter().zip(c.coins) {
        set(name, value);
    }
    set("Weight Carried", c.weight_carried);
    let strength = c.scores[STR];
    set("Encumbered", &format!("{} lb.", strength * 15));
    set("PushDragLift", &format!("{} lb.", strength * 30));
    for (index, (name, qty, weight)) in c.equipment.iter().enumerate() {
        set(&format!("Eq Name{index}"), name);
        set(&format!("Eq Qty{index}"), qty);
        set(&format!("Eq Weight{index}"), weight);
    }
    for (index, name) in c.attuned.iter().enumerate() {
        set(&format!("Attuned Name{}", index + 1), name);
        set(&format!("Attuned Qty{}", index + 1), "1");
    }

    let p = &c.persona;
    for (name, value) in [
        ("GENDER", p.gender),
        ("AGE", p.age),
        ("SIZE", c.size),
        ("HEIGHT", p.height),
        ("WEIGHT", p.weight),
        ("ALIGNMENT", c.alignment),
        ("FAITH", p.faith),
        ("SKIN", p.skin),
        ("EYES", p.eyes),
        ("HAIR", p.hair),
        ("PersonalityTraits ", p.personality),
        ("Ideals", p.ideals),
        ("Bonds", p.bonds),
        ("Flaws", p.flaws),
        ("Appearance", p.appearance),
        ("Backstory", p.backstory),
        ("AlliesOrganizations", p.allies),
        ("AdditionalNotes1", p.notes),
    ] {
        set(name, value);
    }

    if let Some(spells) = &c.spells {
        set("spellCastingClass0", spells.class);
        set("spellCastingAbility0", spells.ability);
        set("spellSaveDC0", spells.save_dc);
        set("spellAtkBonus0", spells.attack);
        // The main page repeats the first casting class's save DC.
        set("AbilitySaveScore1", spells.ability);
        set("AbilitySaveDC", spells.save_dc);
    }
    for (name, value) in &c.overrides {
        v.insert((*name).to_string(), (*value).to_string());
    }
    v
}

/// A widget to write: name, button or not, box, value.
type Widget = (String, bool, [f64; 4], Option<String>);

/// The spell table's rows, D&D Beyond's way: a header row per level (after
/// a blank row, from the second level on), a row per spell numbered across
/// the whole table, and the form's own empty rows after them, named by slot.
fn spell_widgets(rows: &SpellRows, spells: &Spellcasting) -> Vec<Widget> {
    let mut out = Vec::new();
    let mut slot = 0usize;
    let mut number = 0usize;
    let row = |slot: usize, names: Vec<String>, values: Vec<&str>, out: &mut Vec<Widget>| {
        assert!(slot < rows.count, "the spell table has {} rows", rows.count);
        let top = rows.top - slot as f64 * rows.pitch;
        for (((_, x, w), name), value) in rows.columns.iter().zip(names).zip(values) {
            let value = (!value.is_empty()).then(|| value.to_string());
            out.push((name, false, [*x, top - rows.height, x + w, top], value));
        }
    };
    for (level, block) in spells.levels.iter().enumerate() {
        if level > 0 {
            let names = rows
                .columns
                .iter()
                .map(|(c, _, _)| format!("spell{c}BlankHeader{level}"))
                .collect();
            row(slot, names, vec![""; rows.columns.len()], &mut out);
            slot += 1;
        }
        let names = rows
            .columns
            .iter()
            .map(|(c, _, _)| match c.as_str() {
                "Name" => format!("spellHeader{level}"),
                "Source" => format!("spellSlotHeader{level}"),
                other => format!("spell{other}Header{level}"),
            })
            .collect();
        let mut values = vec![""; rows.columns.len()];
        values[1] = block.header;
        values[2] = block.slots;
        row(slot, names, values, &mut out);
        slot += 1;
        for spell in &block.spells {
            let names = rows
                .columns
                .iter()
                .map(|(c, _, _)| format!("spell{c}{number}"))
                .collect();
            let values = vec![
                spell.prepared,
                spell.name,
                spell.source,
                spell.save_hit,
                spell.time,
                spell.range,
                spell.components,
                spell.duration,
                spell.page,
                spell.notes,
            ];
            row(slot, names, values, &mut out);
            slot += 1;
            number += 1;
        }
    }
    // The form's own rows, empty, named by slot and spelled its way.
    while slot < rows.count {
        let names = rows
            .columns
            .iter()
            .map(|(c, _, _)| match c.as_str() {
                "Name" => format!("SpellName{slot}"),
                "Source" => format!("SpellSource{slot}"),
                "Page" => format!("Source{slot}"),
                other => format!("{other}{slot}"),
            })
            .collect();
        row(slot, names, vec![""; rows.columns.len()], &mut out);
        slot += 1;
    }
    out
}

/// Every value the sheet carries, spell rows included.
pub fn all_values(c: &Character) -> BTreeMap<String, String> {
    let mut values = field_values(c);
    if let Some(spells) = &c.spells {
        for (name, _, _, value) in spell_widgets(&layout().spell_rows, spells) {
            if let Some(value) = value {
                values.insert(name, value);
            }
        }
    }
    values
}

fn layout() -> Layout {
    serde_json::from_str(LAYOUT).expect("ddb-layout.json parses")
}

/// The sheet's pages for this character, in D&D Beyond's order.
fn roles(c: &Character) -> Vec<&'static str> {
    let mut roles = vec!["main", "features"];
    if c.features.len() > 3 || c.equipment.len() > EQUIPMENT_ROWS_PER_PAGE {
        roles.push("features_more");
    }
    roles.push("details");
    if c.spells.is_some() {
        roles.push("spells");
    }
    roles
}

/// The bytes of this character's sheet.
pub fn sheet(c: &Character) -> Vec<u8> {
    let layout = layout();
    let values = field_values(c);
    let mut pages: Vec<(&LayoutPage, Vec<Widget>)> = Vec::new();
    let mut known: BTreeSet<String> = BTreeSet::new();
    for role in roles(c) {
        let page = &layout.pages[role];
        let mut widgets: Vec<Widget> = page
            .fields
            .iter()
            .map(|(name, kind, rect)| {
                (
                    name.clone(),
                    kind == "Btn",
                    *rect,
                    values.get(name).cloned(),
                )
            })
            .collect();
        if role == "spells"
            && let Some(spells) = &c.spells
        {
            widgets.extend(spell_widgets(&layout.spell_rows, spells));
        }
        known.extend(widgets.iter().map(|w| w.0.clone()));
        pages.push((page, widgets));
    }
    // A value for a field no printed page has is a typo in a fixture, except
    // a header copy on a page this character does not print.
    const HEADERS: [&str; 6] = [
        "CharacterName",
        "CLASS  LEVEL",
        "PLAYER NAME",
        "RACE",
        "BACKGROUND",
        "EXPERIENCE POINTS",
    ];
    for name in values.keys() {
        let header_copy = HEADERS.iter().any(|h| name.starts_with(h));
        assert!(
            known.contains(name) || header_copy,
            "{}: no field named {name:?}",
            c.file
        );
    }
    write(&pages)
}

/// A one-page document with none of D&D Beyond's anchors: a stat block from
/// some other game. It must be refused as not a sheet.
pub fn not_a_sheet() -> Vec<u8> {
    let page = LayoutPage {
        width: 612.0,
        height: 792.0,
        runs: [
            ("Cave Goblin", 72.0, 700.0, 18.0, true),
            ("Small humanoid, any alignment", 72.0, 680.0, 10.0, false),
            ("Armour 2   Endurance 3   Wits 4", 72.0, 660.0, 10.0, false),
            (
                "Rusty Knife: roll 2 dice, keep the higher.",
                72.0,
                640.0,
                10.0,
                false,
            ),
            ("Flees when its leader falls.", 72.0, 620.0, 10.0, false),
        ]
        .into_iter()
        .map(|(t, x, y, s, b)| (t.to_string(), x, y, s, b))
        .collect(),
        fields: Vec::new(),
    };
    write(&[(&page, Vec::new())])
}

fn text_string(value: &str) -> Object {
    if value.is_ascii() {
        return Object::String(value.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut bytes = vec![0xFE, 0xFF];
    for unit in value.encode_utf16() {
        bytes.extend(unit.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

fn real(value: f64) -> Object {
    // Two decimals, as the layout is kept, so a float's last bit never
    // changes the bytes.
    Object::Real(((value * 100.0).round() / 100.0) as f32)
}

fn write(pages: &[(&LayoutPage, Vec<Widget>)]) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = |doc: &mut Document, base: &str| {
        doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => base,
            "Encoding" => "WinAnsiEncoding",
        })
    };
    let regular = font(&mut doc, "Helvetica");
    let bold = font(&mut doc, "Helvetica-Bold");
    let resources = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => regular, "F2" => bold },
    });

    let mut kids: Vec<Object> = Vec::new();
    for (page, widgets) in pages {
        let mut operations = Vec::new();
        for (text, x, y, size, is_bold) in &page.runs {
            let font = if *is_bold { "F2" } else { "F1" };
            operations.extend([
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec![font.into(), real(*size)]),
                Operation::new(
                    "Tm",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), real(*x), real(*y)],
                ),
                Operation::new(
                    "Tj",
                    vec![Object::String(
                        text.as_bytes().to_vec(),
                        StringFormat::Literal,
                    )],
                ),
                Operation::new("ET", vec![]),
            ]);
        }
        let content = Content { operations }.encode().expect("content encodes");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content));
        let page_id = doc.new_object_id();
        let mut annots: Vec<Object> = Vec::new();
        for (name, button, rect, value) in widgets {
            let mut widget = dictionary! {
                "Type" => "Annot",
                "Subtype" => "Widget",
                "FT" => if *button { "Btn" } else { "Tx" },
                "T" => text_string(name),
                "Rect" => rect.iter().map(|v| real(*v)).collect::<Vec<_>>(),
                "P" => page_id,
                "F" => 4,
            };
            if let Some(value) = value {
                widget.set("V", text_string(value));
            }
            annots.push(doc.add_object(widget).into());
        }
        let mut dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), real(page.width), real(page.height)],
            "Resources" => resources,
            "Contents" => content_id,
        };
        if !annots.is_empty() {
            dict.set("Annots", annots);
        }
        doc.objects.insert(page_id, Object::Dictionary(dict));
        kids.push(page_id.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let id = Object::String(b"thunderforge-048".to_vec(), StringFormat::Hexadecimal);
    doc.trailer.set("ID", vec![id.clone(), id]);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("the fixture writes");
    bytes
}
