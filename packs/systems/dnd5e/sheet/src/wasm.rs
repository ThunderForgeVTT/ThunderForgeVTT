//! The reader in the browser (spec 048 T032): the same crate the server
//! re-reads an upload with, compiled for a different target.

use wasm_bindgen::prelude::*;

/// Read a character sheet. Returns JSON: see [`crate::read_sheet_json`].
/// Never throws: a sheet it cannot read is an answer, not an exception.
#[wasm_bindgen(js_name = readSheet)]
pub fn read_sheet(bytes: &[u8]) -> String {
    crate::read_sheet_json(bytes)
}
