//! What the browser receives from `readSheet` (spec 048 T032).
//!
//! The wasm export is a thin wrapper over `read_sheet_json`, so these pin the
//! answer the import page reads: `{ recognised, reading }` for a sheet, and
//! `{ recognised, error }` with the sentence a person sees otherwise.

use std::path::PathBuf;

use serde_json::Value;
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{ImportedCharacter, SheetReader};
use thunderforge_system_dnd5e_sheet::{DdbPdf, NOT_A_SHEET, read_sheet_json};

fn bytes(file: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn answer(file: &str) -> Value {
    serde_json::from_str(&read_sheet_json(&bytes(file))).expect("readSheet answers JSON")
}

#[test]
fn a_sheet_answers_recognised_with_the_same_reading_the_server_reads() {
    let answer = answer("fighter-5.pdf");
    assert_eq!(answer["recognised"], true);
    assert!(answer.get("error").is_none(), "{answer}");
    let reading: ImportedCharacter =
        serde_json::from_value(answer["reading"].clone()).expect("a reading");
    let native = DdbPdf
        .read(&Document::from_bytes(&bytes("fighter-5.pdf")).unwrap())
        .unwrap();
    assert_eq!(reading, native);
}

#[test]
fn another_pdf_answers_not_recognised_with_the_reason() {
    let answer = answer("not-a-ddb-sheet.pdf");
    assert_eq!(answer["recognised"], false);
    assert_eq!(answer["error"], NOT_A_SHEET);
    assert!(answer.get("reading").is_none());
}

#[test]
fn bytes_that_are_not_a_pdf_answer_not_recognised_with_an_error() {
    let answer: Value = serde_json::from_str(&read_sheet_json(b"not a pdf at all")).unwrap();
    assert_eq!(answer["recognised"], false);
    assert!(
        answer["error"].as_str().is_some_and(|e| !e.is_empty()),
        "{answer}"
    );
}

/// The native answer for every fixture, written where the browser harness
/// (`pnpm -F @thunderforge/dnd5e test:sheet-reader`, T043) compares the wasm
/// answer against it: `target/sheet-fixtures/<fixture>.json`.
#[test]
fn every_fixture_answers_and_the_answer_is_kept_for_the_browser() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../target"))
        .join("sheet-fixtures");
    std::fs::create_dir_all(&out).expect("the fixtures directory");
    let mut written = 0;
    for entry in std::fs::read_dir(&fixtures).expect("the fixtures") {
        let path = entry.expect("a fixture").path();
        if path.extension().is_none_or(|ext| ext != "pdf") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        // Written as `readSheet` returns it: parsing it into a `Value` and
        // printing it again would round its f32 rectangles.
        let raw = read_sheet_json(&bytes(&format!("{stem}.pdf")));
        let answer: Value = serde_json::from_str(&raw).expect("readSheet answers JSON");
        assert!(answer["recognised"].is_boolean(), "{stem}: {answer}");
        std::fs::write(out.join(format!("{stem}.json")), raw).expect("the native answer");
        written += 1;
    }
    assert!(written >= 8, "only {written} fixtures");
}
