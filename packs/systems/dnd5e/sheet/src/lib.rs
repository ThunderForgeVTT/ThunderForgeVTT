//! The D&D Beyond character sheet reader (spec 048).
//!
//! D&D Beyond exports keep every value in a widget form field (`/V`) and draw
//! only the labels, so the reader finds the sheet by its labels and reads
//! each value by its field's name. The field names were measured from real
//! exports (`tests/fixtures/ddb-layout.json`).
//!
//! The reader is pure: the same bytes give the same reading in the browser
//! (as wasm) and on the server, which is what lets the two agree on a plan.

mod browser;
mod content;
mod fields;
mod glyphs;
mod recognise;
#[cfg(feature = "wasm")]
mod wasm;

use thunderforge_pdf::Document;
use thunderforge_sheet_import::{
    ImportedCharacter, ReadError, ReaderStamp, Recognition, SheetReader,
};

pub use browser::read_sheet_json;
pub use recognise::NOT_A_SHEET;

/// The `ddb-pdf` reader.
#[derive(Clone, Copy, Debug, Default)]
pub struct DdbPdf;

/// Bumped whenever the same bytes could read differently.
pub const VERSION: &str = "1";

impl SheetReader for DdbPdf {
    fn id(&self) -> &'static str {
        "ddb-pdf"
    }

    fn version(&self) -> &'static str {
        VERSION
    }

    fn recognise(&self, doc: &Document) -> Recognition {
        recognise::recognise(doc)
    }

    fn read(&self, doc: &Document) -> Result<ImportedCharacter, ReadError> {
        if let Recognition::No { reason } = recognise::recognise(doc) {
            return Err(ReadError::NotRecognised(reason));
        }
        let sheet = fields::Sheet::read(doc);
        let mut character = fields::read_character(&sheet)?;
        character.content = content::read_content(&sheet, &character);
        character.reader = ReaderStamp {
            id: self.id().to_string(),
            version: self.version().to_string(),
        };
        Ok(character)
    }
}
