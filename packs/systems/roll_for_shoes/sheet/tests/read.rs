//! The Roll for Shoes sheet reader (spec 048 T067, US4).

#[path = "fixtures/gen.rs"]
mod generator;

use serde_json::{Value, json};
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{
    Certainty, ImportedCharacter, ReadError, Recognition, SheetReader,
};
use thunderforge_system_roll_for_shoes_sheet::{NOT_A_SHEET, RfsPdf, read_sheet_json};

fn fixture(file: &str) -> Vec<u8> {
    if file == "not-a-rfs-sheet.pdf" {
        return generator::not_a_sheet();
    }
    let character = generator::all()
        .into_iter()
        .find(|c| c.file == file)
        .expect("a fixture");
    generator::sheet(&character)
}

fn read(file: &str) -> Result<ImportedCharacter, ReadError> {
    let bytes = fixture(file);
    let doc = Document::from_bytes(&bytes).expect("parses");
    RfsPdf.read(&doc)
}

fn skills(reading: &ImportedCharacter) -> Vec<(String, Value)> {
    reading
        .content
        .iter()
        .map(|c| {
            assert_eq!(c.kind, "skill");
            (c.name.value.clone().unwrap_or_default(), json!(c.fields))
        })
        .collect()
}

#[test]
fn the_reader_names_itself() {
    assert_eq!(RfsPdf.id(), "tf-rfs-pdf");
    assert_eq!(RfsPdf.version(), "1");
}

#[test]
fn a_clean_sheet_reads_its_name_xp_and_every_skill_in_order() {
    let reading = read("wren-4.pdf").expect("reads");
    assert_eq!(reading.reader.id, "tf-rfs-pdf");
    assert_eq!(reading.identity.name.value.as_deref(), Some("Wren Ashdown"));
    assert_eq!(reading.identity.name.certainty, Certainty::Read);
    let source = reading.identity.name.source.as_ref().expect("a source");
    assert_eq!(source.page, 1);
    assert_eq!(source.text, "Wren Ashdown");
    assert_eq!(reading.identity.xp.value, Some(2));
    assert_eq!(
        skills(&reading),
        vec![
            (
                "Do Anything".into(),
                json!({"row": 1, "level": 1, "grew_from": null})
            ),
            (
                "Sneak".into(),
                json!({"row": 2, "level": 2, "grew_from": 1})
            ),
            (
                "Hide in Shadows".into(),
                json!({"row": 3, "level": 3, "grew_from": 2})
            ),
            (
                "Climb".into(),
                json!({"row": 4, "level": 2, "grew_from": 1})
            ),
            (
                "Pick Locks".into(),
                json!({"row": 5, "level": 3, "grew_from": 2})
            ),
        ]
    );
    assert!(
        reading
            .content
            .iter()
            .all(|c| c.name.certainty == Certainty::Read)
    );
    // Nothing a D&D sheet has is read from this one.
    assert!(reading.classes.is_empty());
    assert!(reading.abilities.is_empty());
}

#[test]
fn a_level_that_is_not_a_number_is_uncertain_and_says_what_was_printed() {
    let reading = read("misprinted-lineage.pdf").expect("reads");
    let bake = &reading.content[1];
    assert_eq!(bake.name.value.as_deref(), Some("Bake"));
    assert_eq!(bake.fields["level"], Value::Null);
    match &bake.name.certainty {
        Certainty::Uncertain { reason } => assert!(reason.contains("two"), "{reason}"),
        other => panic!("expected uncertain, got {other:?}"),
    }
    // The reader reports what the sheet says; the lineage is the pack's to
    // judge when it plans the skills.
    assert_eq!(reading.content[2].fields["grew_from"], json!(2));
    assert_eq!(reading.content[2].name.certainty, Certainty::Read);
}

#[test]
fn another_document_is_not_recognised() {
    let bytes = fixture("not-a-rfs-sheet.pdf");
    let doc = Document::from_bytes(&bytes).expect("parses");
    assert_eq!(
        RfsPdf.recognise(&doc),
        Recognition::No {
            reason: NOT_A_SHEET.to_string()
        }
    );
    match RfsPdf.read(&doc) {
        Err(error @ ReadError::NotRecognised(_)) => {
            assert_eq!(error.code(), "SHEET_NOT_RECOGNISED");
            assert_eq!(error.sentence(), NOT_A_SHEET);
        }
        other => panic!("expected not recognised, got {other:?}"),
    }
}

#[test]
fn the_same_bytes_read_the_same() {
    assert_eq!(read("wren-5.pdf").unwrap(), read("wren-5.pdf").unwrap());
}

#[test]
fn the_browser_answer_is_the_reading_or_the_refusal() {
    let answer: Value = serde_json::from_str(&read_sheet_json(&fixture("wren-4.pdf"))).unwrap();
    assert_eq!(answer["recognised"], json!(true));
    let reading: ImportedCharacter = serde_json::from_value(answer["reading"].clone()).unwrap();
    assert_eq!(reading, read("wren-4.pdf").unwrap());

    let refused: Value =
        serde_json::from_str(&read_sheet_json(&fixture("not-a-rfs-sheet.pdf"))).unwrap();
    assert_eq!(
        refused,
        json!({"recognised": false, "code": "SHEET_NOT_RECOGNISED", "error": NOT_A_SHEET})
    );
    let garbage: Value = serde_json::from_str(&read_sheet_json(b"not a pdf")).unwrap();
    assert_eq!(garbage["recognised"], json!(false));
    assert_eq!(garbage["code"], json!("SHEET_UNREADABLE"));
}
