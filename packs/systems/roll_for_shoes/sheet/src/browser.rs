//! The answer the browser's import page reads (spec 048 T032).
//!
//! Not behind the `wasm` feature, so host tests pin exactly what `readSheet`
//! returns. The page shows a plan built from `reading` and sends the same
//! bytes up; the server reads them again with this crate and must agree.

use serde_json::{Value, json};
use thunderforge_pdf::{Document, Limits};
use thunderforge_sheet_import::{ReadError, SheetReader};

use crate::RfsPdf;

/// `{ "recognised": true, "reading": … }`, or
/// `{ "recognised": bool, "code": "SHEET_…", "error": "…" }` with the
/// refusal code and the sentence a person sees, the server's own.
///
/// A file that is not a PDF, or is a PDF but not this kind of sheet, is "not
/// recognised". A recognised sheet whose required block cannot be read keeps
/// `recognised: true`, so the page can say "this is a Roll for Shoes sheet, but".
pub fn read_sheet_json(bytes: &[u8]) -> String {
    answer(bytes).to_string()
}

fn answer(bytes: &[u8]) -> Value {
    // The server's bounds, so a file it would refuse is refused here first,
    // with the same code and sentence, before anything is uploaded.
    let doc = match Document::from_bytes_bounded(bytes, Limits::default()) {
        Ok(doc) => doc,
        Err(error) => return refused(false, &ReadError::Pdf(error)),
    };
    match RfsPdf.read(&doc) {
        Ok(reading) => match serde_json::to_value(&reading) {
            Ok(reading) => json!({ "recognised": true, "reading": reading }),
            Err(error) => json!({
                "recognised": true,
                "code": "SHEET_UNREADABLE",
                "error": format!("The reading could not be written: {error}."),
            }),
        },
        Err(error @ ReadError::Page { .. }) => refused(true, &error),
        Err(error) => refused(false, &error),
    }
}

fn refused(recognised: bool, error: &ReadError) -> Value {
    json!({ "recognised": recognised, "code": error.code(), "error": error.sentence() })
}
