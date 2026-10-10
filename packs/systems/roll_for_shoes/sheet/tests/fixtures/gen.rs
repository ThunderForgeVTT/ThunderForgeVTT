//! The fixture generator (spec 048 T067).
//!
//! It lays out invented characters on the one-page ThunderForge Roll for
//! Shoes sheet. Every value is made up. The values live in widget fields
//! (`/V`) and the page content draws only the labels, the way the D&D Beyond
//! fixtures do, so one PDF layer reads both.
//!
//! Generation is deterministic: no timestamps, no `/Info`, a fixed `/ID`, and
//! objects numbered in the order they are written. `fixtures.rs` pins each
//! file's sha256, so a change here is a visible diff.

#![allow(dead_code)]

use std::collections::BTreeMap;

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, StringFormat, dictionary};

/// Skill rows the sheet prints.
pub const ROWS: usize = 12;

/// One skill row as written: the name, the level and the row it grew from,
/// each as the text in the box, so a fixture can misprint one.
pub struct Row {
    pub name: &'static str,
    pub level: &'static str,
    pub from: &'static str,
}

pub struct Character {
    pub file: &'static str,
    pub name: &'static str,
    pub xp: &'static str,
    pub skills: Vec<Row>,
}

const fn row(name: &'static str, level: &'static str, from: &'static str) -> Row {
    Row { name, level, from }
}

/// The invented characters, one fixture each.
pub fn all() -> Vec<Character> {
    vec![
        // A clean sheet: a lineage two deep on two branches.
        Character {
            file: "wren-4.pdf",
            name: "Wren Ashdown",
            xp: "2",
            skills: vec![
                row("Do Anything", "1", ""),
                row("Sneak", "2", "1"),
                row("Hide in Shadows", "3", "2"),
                row("Climb", "2", "1"),
                row("Pick Locks", "3", "2"),
            ],
        },
        // The same character, a session on: a skill more, and XP spent.
        Character {
            file: "wren-5.pdf",
            name: "Wren Ashdown",
            xp: "0",
            skills: vec![
                row("Do Anything", "1", ""),
                row("Sneak", "2", "1"),
                row("Hide in Shadows", "3", "2"),
                row("Climb", "2", "1"),
                row("Pick Locks", "3", "2"),
                row("Vanish in a Crowd", "4", "3"),
            ],
        },
        // What a hand-filled sheet gets wrong: a level that is a word, and a
        // skill two levels above the one it says it grew from.
        Character {
            file: "misprinted-lineage.pdf",
            name: "Odo Fenwick",
            xp: "1",
            skills: vec![
                row("Do Anything", "1", ""),
                row("Bake", "two", "1"),
                row("Bake Under Pressure", "4", "2"),
            ],
        },
    ]
}

/// The field names on the sheet.
pub fn skill_field(n: usize) -> String {
    format!("Skill{n}")
}

pub fn level_field(n: usize) -> String {
    format!("Level{n}")
}

pub fn from_field(n: usize) -> String {
    format!("From{n}")
}

/// Every value the sheet holds for this character, by field name.
pub fn field_values(c: &Character) -> BTreeMap<String, String> {
    assert!(c.skills.len() <= ROWS, "{}: too many skills", c.file);
    let mut values = BTreeMap::new();
    values.insert("Name".to_string(), c.name.to_string());
    values.insert("XP".to_string(), c.xp.to_string());
    for (index, skill) in c.skills.iter().enumerate() {
        let n = index + 1;
        values.insert(skill_field(n), skill.name.to_string());
        values.insert(level_field(n), skill.level.to_string());
        if !skill.from.is_empty() {
            values.insert(from_field(n), skill.from.to_string());
        }
    }
    values
}

type Run = (String, f64, f64, f64, bool);
type Widget = (String, [f64; 4], Option<String>);

/// The labels the page draws: text, x, y, size, bold.
fn labels() -> Vec<Run> {
    let mut runs: Vec<Run> = [
        ("ROLL FOR SHOES", 72.0, 730.0, 20.0, true),
        ("ThunderForge character sheet", 72.0, 712.0, 10.0, false),
        ("NAME", 72.0, 682.0, 9.0, true),
        ("XP", 400.0, 682.0, 9.0, true),
        ("SKILLS", 72.0, 632.0, 12.0, true),
        ("SKILL", 72.0, 612.0, 9.0, true),
        ("LEVEL", 380.0, 612.0, 9.0, true),
        ("GREW FROM", 460.0, 612.0, 9.0, true),
    ]
    .into_iter()
    .map(|(t, x, y, s, b)| (t.to_string(), x, y, s, b))
    .collect();
    for n in 1..=ROWS {
        runs.push((n.to_string(), 56.0, row_top(n) - 13.0, 9.0, false));
    }
    runs
}

fn row_top(n: usize) -> f64 {
    604.0 - 24.0 * (n - 1) as f64
}

fn widgets(values: &BTreeMap<String, String>) -> Vec<Widget> {
    let mut boxes: Vec<(String, [f64; 4])> = vec![
        ("Name".to_string(), [72.0, 660.0, 380.0, 678.0]),
        ("XP".to_string(), [400.0, 660.0, 540.0, 678.0]),
    ];
    for n in 1..=ROWS {
        let top = row_top(n);
        boxes.push((skill_field(n), [72.0, top - 18.0, 370.0, top]));
        boxes.push((level_field(n), [380.0, top - 18.0, 450.0, top]));
        boxes.push((from_field(n), [460.0, top - 18.0, 540.0, top]));
    }
    boxes
        .into_iter()
        .map(|(name, rect)| {
            let value = values.get(&name).cloned();
            (name, rect, value)
        })
        .collect()
}

/// The bytes of this character's sheet.
pub fn sheet(c: &Character) -> Vec<u8> {
    let values = field_values(c);
    let widgets = widgets(&values);
    for name in values.keys() {
        assert!(
            widgets.iter().any(|w| &w.0 == name),
            "{}: no field named {name:?}",
            c.file
        );
    }
    write(&[(labels(), widgets)])
}

/// A one-page document with none of the sheet's labels: a D&D-like stat
/// block. It must be refused as not this sheet.
pub fn not_a_sheet() -> Vec<u8> {
    let runs: Vec<Run> = [
        ("Marsh Troll", 72.0, 700.0, 18.0, true),
        ("Large giant, chaotic evil", 72.0, 680.0, 10.0, false),
        ("Armour Class 15   Hit Points 84", 72.0, 660.0, 10.0, false),
    ]
    .into_iter()
    .map(|(t, x, y, s, b)| (t.to_string(), x, y, s, b))
    .collect();
    write(&[(runs, Vec::new())])
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
    Object::Real(((value * 100.0).round() / 100.0) as f32)
}

fn write(pages: &[(Vec<Run>, Vec<Widget>)]) -> Vec<u8> {
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
    for (runs, widgets) in pages {
        let mut operations = Vec::new();
        for (text, x, y, size, is_bold) in runs {
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
        for (name, rect, value) in widgets {
            let mut widget = dictionary! {
                "Type" => "Annot",
                "Subtype" => "Widget",
                "FT" => "Tx",
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
            "MediaBox" => vec![0.into(), 0.into(), real(612.0), real(792.0)],
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
    let id = Object::String(b"thunderforge-048-rfs".to_vec(), StringFormat::Hexadecimal);
    doc.trailer.set("ID", vec![id.clone(), id]);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("the fixture writes");
    bytes
}
