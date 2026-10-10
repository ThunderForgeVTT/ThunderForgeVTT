//! The ThunderForge Roll for Shoes sheet reader (spec 048 US4).
//!
//! The one-page sheet keeps every value in a named widget field (`Name`,
//! `XP`, and `Skill`/`Level`/`From` for each numbered row) and draws only the
//! labels, so the reader finds the sheet by its labels and reads each value
//! by its field's name.
//!
//! Skills are read as content of the kind `skill`, one per row, with the row
//! number, the level and the row it grew from. Where they land is the pack's
//! refine hook's to decide: a skill is part of the character, not a piece of
//! the world's compendium.
//!
//! The reader is pure: the same bytes give the same reading in the browser
//! (as wasm) and on the server, which is what lets the two agree on a plan.

mod browser;
#[cfg(feature = "wasm")]
mod wasm;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use thunderforge_pdf::{Document, PageText, Rect};
use thunderforge_sheet_import::{
    Field, ImportedCharacter, NamedContent, ReadError, ReaderStamp, Recognition, SheetReader,
    Source,
};

pub use browser::read_sheet_json;

pub const NOT_A_SHEET: &str =
    "This does not look like a ThunderForge Roll for Shoes character sheet.";

/// The labels page 1 carries.
const LABELS: [&str; 2] = ["ROLL FOR SHOES", "GREW FROM"];

/// Skill rows the sheet prints.
pub const ROWS: usize = 12;

/// The `tf-rfs-pdf` reader.
#[derive(Clone, Copy, Debug, Default)]
pub struct RfsPdf;

/// Bumped whenever the same bytes could read differently.
pub const VERSION: &str = "1";

impl SheetReader for RfsPdf {
    fn id(&self) -> &'static str {
        "tf-rfs-pdf"
    }

    fn version(&self) -> &'static str {
        VERSION
    }

    fn recognise(&self, doc: &Document) -> Recognition {
        let no = || Recognition::No {
            reason: NOT_A_SHEET.to_string(),
        };
        if doc.page_count() != 1 {
            return no();
        }
        match PageText::read(doc, 1) {
            Ok(page) if LABELS.iter().all(|label| !page.find(label).is_empty()) => Recognition::Yes,
            _ => no(),
        }
    }

    fn read(&self, doc: &Document) -> Result<ImportedCharacter, ReadError> {
        if let Recognition::No { reason } = self.recognise(doc) {
            return Err(ReadError::NotRecognised(reason));
        }
        let sheet = Sheet::read(doc);
        let mut character = ImportedCharacter::default();
        character.identity.name = sheet.string("Name");
        character.identity.xp = sheet.number("XP");
        for row in 1..=ROWS {
            if let Some(skill) = skill(&sheet, row) {
                character.content.push(skill);
            }
        }
        character.reader = ReaderStamp {
            id: self.id().to_string(),
            version: self.version().to_string(),
        };
        Ok(character)
    }
}

/// One row, if it names a skill. A level or a "grew from" the reader cannot
/// read makes the skill uncertain, with what was printed.
fn skill(sheet: &Sheet, row: usize) -> Option<NamedContent> {
    let name = sheet.filled(&format!("Skill{row}"))?;
    let mut doubts = Vec::new();
    let mut number = |field: String, what: &str| -> Value {
        match sheet.filled(&field) {
            None => Value::Null,
            Some(entry) => match entry.text().parse::<u32>() {
                Ok(n) => json!(n),
                Err(_) => {
                    doubts.push(format!(
                        "the {what} reads \u{201C}{}\u{201D}, which is not a number",
                        entry.text()
                    ));
                    Value::Null
                }
            },
        }
    };
    let level = number(format!("Level{row}"), "level");
    let grew_from = number(format!("From{row}"), "row it grew from");
    if level.is_null() && doubts.is_empty() {
        doubts.push("the level is blank".to_string());
    }
    let text = name.text().to_string();
    let source = Some(name.source());
    let name = if doubts.is_empty() {
        Field::read(text, source)
    } else {
        Field::uncertain(Some(text), format!("{}.", doubts.join("; ")), source)
    };
    Some(NamedContent {
        kind: "skill".to_string(),
        name,
        fields: BTreeMap::from([
            ("row".to_string(), json!(row)),
            ("level".to_string(), level),
            ("grew_from".to_string(), grew_from),
        ]),
        link: Default::default(),
    })
}

/// One field's value and where it sits.
struct Entry {
    value: String,
    page: u32,
    rect: Rect,
}

impl Entry {
    fn source(&self) -> Source {
        Source {
            page: self.page,
            rect: self.rect,
            text: self.value.clone(),
        }
    }

    fn text(&self) -> &str {
        self.value.trim()
    }
}

/// Every named field on the sheet.
struct Sheet {
    fields: BTreeMap<String, Entry>,
}

impl Sheet {
    fn read(doc: &Document) -> Sheet {
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

    fn filled(&self, name: &str) -> Option<&Entry> {
        self.fields
            .get(name)
            .filter(|entry| !entry.text().is_empty())
    }

    fn string(&self, name: &str) -> Field<String> {
        match self.filled(name) {
            Some(entry) => Field::read(entry.text().to_string(), Some(entry.source())),
            None => Field::unread(),
        }
    }

    fn number(&self, name: &str) -> Field<i64> {
        match self.filled(name) {
            Some(entry) => match entry.text().parse::<i64>() {
                Ok(n) => Field::read(n, Some(entry.source())),
                Err(_) => Field::uncertain(
                    None,
                    format!(
                        "\u{201C}{}\u{201D} is not a number this reader can read.",
                        entry.text()
                    ),
                    Some(entry.source()),
                ),
            },
            None => Field::unread(),
        }
    }
}
