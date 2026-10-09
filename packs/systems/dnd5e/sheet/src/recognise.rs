//! Whether a document is a D&D Beyond export (contracts/sheet-mapping-5e.md,
//! "Recognition and refusals").
//!
//! Page 1 carries "CHARACTER NAME" and "CLASS & LEVEL", and some page carries
//! "FEATURES & TRAITS". The spell page is optional: a non-caster's export has
//! none.

use thunderforge_pdf::{Document, PageText};
use thunderforge_sheet_import::Recognition;

pub const NOT_A_SHEET: &str = "This does not look like a D&D Beyond character sheet.";

const FIRST_PAGE: [&str; 2] = ["CHARACTER NAME", "CLASS & LEVEL"];
const SOME_PAGE: &str = "FEATURES & TRAITS";

pub fn recognise(doc: &Document) -> Recognition {
    let no = || Recognition::No {
        reason: NOT_A_SHEET.to_string(),
    };
    let Ok(first) = PageText::read(doc, 1) else {
        return no();
    };
    if FIRST_PAGE.iter().any(|label| first.find(label).is_empty()) {
        return no();
    }
    let features = (1..=doc.page_count() as u32)
        .any(|page| PageText::read(doc, page).is_ok_and(|text| !text.find(SOME_PAGE).is_empty()));
    if features { Recognition::Yes } else { no() }
}
