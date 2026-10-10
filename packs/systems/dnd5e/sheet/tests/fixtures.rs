//! The generated fixtures (spec 048 T024).
//!
//! Each fixture is generated from `fixtures/characters.rs` on the measured
//! layout, and its sha256 is pinned here, so a change to the generator or a
//! character is a visible diff. Regenerate the committed files with
//! `THUNDERFORGE_WRITE_FIXTURES=1 cargo test -p thunderforge-system-dnd5e-sheet --test fixtures`,
//! then update the pins.

#[path = "fixtures/characters.rs"]
mod characters;
#[path = "fixtures/gen.rs"]
mod generator;

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use thunderforge_pdf::Document;

const PINNED: &[(&str, &str)] = &[
    (
        "fighter-5.pdf",
        "296cb40d35a6ca4da110a1d56a04d399a773fdc4d367b19f5ed4fb6c60280715",
    ),
    (
        "fighter3-wizard2.pdf",
        "48b8290f7d50d6053a98694ebdced1755f144eb33c86571bdc7c04cc5e5267ff",
    ),
    (
        "fighter3-wizard2-l6.pdf",
        "83a7290ca2a878c9e5fb093be20e3437cb1541e935708095bb232160e0edc360",
    ),
    (
        "cleric-7.pdf",
        "9bfea6384c45eccb4f48027cb5a26e43c7b9cf12b49a6ab68e5e42b5649665c3",
    ),
    (
        "rogue-4.pdf",
        "be567128c084f32ef2527022f8b7bc6c6578237bd874a6d1a49cba971094fa39",
    ),
    (
        "warforged-defences.pdf",
        "2008f1613ae89c324c18b0ee6c50bb449700829a6f7d6cfb2ba56d5b391b5d23",
    ),
    (
        "uncertain-mark.pdf",
        "f8d7aca062afdc4699d1a414c6fffeae202ebbd3108413fed3d9b3ddd7ff5d51",
    ),
    (
        "not-a-ddb-sheet.pdf",
        "5b79f2ae7c900abcad6a61c16074315f2a57c19a5925f66b1477589c93997f8f",
    ),
];

fn generated() -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = characters::all()
        .iter()
        .map(|c| (c.file, generator::sheet(c)))
        .collect();
    out.push(("not-a-ddb-sheet.pdf", generator::not_a_sheet()));
    out
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn generation_is_deterministic() {
    assert!(generated() == generated(), "two runs wrote different bytes");
}

#[test]
fn the_committed_fixtures_are_the_generated_ones() {
    let fixtures = generated();
    if std::env::var_os("THUNDERFORGE_WRITE_FIXTURES").is_some() {
        for (file, bytes) in &fixtures {
            std::fs::write(dir().join(file), bytes).expect("write fixture");
        }
    }
    let table: Vec<String> = fixtures
        .iter()
        .map(|(file, bytes)| format!("    (\"{file}\", \"{}\"),", sha256(bytes)))
        .collect();
    for (file, bytes) in &fixtures {
        let pinned = PINNED.iter().find(|(name, _)| name == file);
        assert_eq!(
            pinned.map(|(_, hash)| *hash),
            Some(sha256(bytes).as_str()),
            "{file}: the pin does not match; the generated table is\n{}",
            table.join("\n")
        );
        let committed = std::fs::read(dir().join(file)).expect("committed fixture");
        assert!(
            &committed == bytes,
            "{file}: the committed file is not the generated one"
        );
    }
    assert_eq!(PINNED.len(), fixtures.len(), "a pin names no fixture");
}

/// The fixtures read back through the reader's own PDF layer: the values
/// sit in widget fields on the pages a real export puts them on.
#[test]
fn the_fixtures_read_back() {
    for character in characters::all() {
        let bytes = generator::sheet(&character);
        let doc = Document::from_bytes(&bytes).expect("parses");
        let expected = generator::all_values(&character);
        let pages = doc.pages();
        let want_pages =
            3 + usize::from(character.features.len() > 3) + usize::from(character.spells.is_some());
        assert_eq!(pages.len(), want_pages, "{}", character.file);
        let mut read = std::collections::BTreeMap::new();
        for page in &pages {
            for field in doc.form_fields(page) {
                if !field.value.is_empty() {
                    read.insert(field.name, field.value);
                }
            }
        }
        for (name, value) in &read {
            assert_eq!(
                expected.get(name),
                Some(value),
                "{}: {name:?} read back differently",
                character.file
            );
        }
        assert_eq!(read["CharacterName"], character.name, "{}", character.file);
    }
    let other = Document::from_bytes(&generator::not_a_sheet()).expect("parses");
    assert_eq!(other.page_count(), 1);
    assert!(other.form_fields(&other.pages()[0]).is_empty());
}
