use super::*;

fn systems_dir() -> String {
    format!("{}/../../packs/systems", env!("CARGO_MANIFEST_DIR"))
}

/// Every bundled pack loads: a malformed `sheetImport` block fails here,
/// before it can fail at import time.
#[test]
fn every_bundled_packs_declaration_loads() {
    let dir = systems_dir();
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("packs/systems exists") {
        let entry = entry.unwrap();
        if !entry.path().join("system.json").exists() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Err(error) = mapping_for_system(&dir, &id) {
            panic!("{id}: {error}");
        }
        seen += 1;
    }
    assert!(seen >= 8, "found {seen} packs");
}

#[test]
fn a_pack_with_no_block_takes_no_sheets() {
    let dir = tempdir("none");
    write(&dir, "plain", r#"{ "id": "plain", "data_types": {} }"#);
    assert_eq!(mapping_for_system(&dir, "plain"), Ok(None));
    assert_eq!(mapping_for_system(&dir, "absent"), Ok(None));
}

#[test]
fn a_malformed_block_fails_to_load() {
    let dir = tempdir("bad");
    write(
        &dir,
        "bad",
        r#"{
            "id": "bad",
            "data_types": { "trait_data": { "properties": { "notes": {} } } },
            "sheetImport": {
                "readers": ["r"],
                "fields": { "abilities.str": "ability_data.strength" },
                "notes": "trait_data.notes"
            }
        }"#,
    );
    let error = mapping_for_system(&dir, "bad").unwrap_err();
    assert!(
        error.to_string().contains("ability_data.strength"),
        "{error}"
    );
}

fn tempdir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("tf-sheet-mapping-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().into_owned()
}

fn write(dir: &str, id: &str, manifest: &str) {
    let pack = std::path::Path::new(dir).join(id);
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("system.json"), manifest).unwrap();
}
