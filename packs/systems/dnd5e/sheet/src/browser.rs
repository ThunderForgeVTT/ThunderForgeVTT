//! The answer the browser's import page reads (spec 048 T032).
//!
//! Not behind the `wasm` feature, so host tests pin exactly what `readSheet`
//! returns. The page shows a plan built from `reading` and sends the same
//! bytes up; the server reads them again with this crate and must agree.

use serde_json::{Value, json};
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{ReadError, SheetReader};

use crate::DdbPdf;

/// `{ "recognised": true, "reading": … }`, or
/// `{ "recognised": bool, "error": "…" }` with the sentence a person sees.
///
/// A file that is not a PDF, or is a PDF but not this kind of sheet, is "not
/// recognised". A recognised sheet whose required block cannot be read keeps
/// `recognised: true`, so the page can say "this is a D&D Beyond sheet, but".
pub fn read_sheet_json(bytes: &[u8]) -> String {
    answer(bytes).to_string()
}

fn answer(bytes: &[u8]) -> Value {
    let doc = match Document::from_bytes(bytes) {
        Ok(doc) => doc,
        Err(error) => return json!({ "recognised": false, "error": error.to_string() }),
    };
    match DdbPdf.read(&doc) {
        Ok(reading) => match serde_json::to_value(&reading) {
            Ok(reading) => json!({ "recognised": true, "reading": reading }),
            Err(error) => json!({ "recognised": true, "error": error.to_string() }),
        },
        Err(error @ (ReadError::NotRecognised(_) | ReadError::Pdf(_))) => {
            json!({ "recognised": false, "error": error.to_string() })
        }
        Err(error @ ReadError::Page { .. }) => {
            json!({ "recognised": true, "error": error.to_string() })
        }
    }
}
