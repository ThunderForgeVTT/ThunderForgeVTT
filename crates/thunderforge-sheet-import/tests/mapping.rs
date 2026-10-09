//! A `sheetImport` declaration is checked when the pack loads.

use serde_json::{Value, json};
use thunderforge_sheet_import::{ContentTarget, SheetMapping};

fn manifest(block: Value) -> Value {
    json!({
        "data_types": {
            "ability_data": { "properties": { "strength": {} } },
            "trait_data": { "properties": { "notes": {} } }
        },
        "abilityVocabulary": { "types": [ { "id": "spell" } ] },
        "sheetImport": block
    })
}

fn good() -> Value {
    json!({
        "readers": ["r"],
        "fields": { "abilities.str": "ability_data.strength", "identity.name": "actor.label" },
        "content": { "spell": { "ability": "spell" }, "item": { "item": true }, "attack": { "refine": true } },
        "derived": ["skill.*"],
        "playState": ["resource_data.hit_dice_pools[].used"],
        "notes": "trait_data.notes"
    })
}

fn error(block: Value) -> String {
    SheetMapping::from_manifest(&manifest(block))
        .expect_err("must fail to load")
        .to_string()
}

#[test]
fn no_block_is_no_mapping() {
    let none = SheetMapping::from_manifest(&json!({ "data_types": {} })).unwrap();
    assert!(none.is_none());
}

#[test]
fn a_good_block_parses() {
    let mapping = SheetMapping::from_manifest(&manifest(good()))
        .unwrap()
        .unwrap();
    assert_eq!(mapping.readers, vec!["r"]);
    assert_eq!(
        mapping.content["spell"],
        ContentTarget::Ability {
            vocabulary: "spell".into()
        }
    );
    assert_eq!(mapping.content["item"], ContentTarget::Item);
    assert_eq!(mapping.content["attack"], ContentTarget::Refine);
    assert!(mapping.is_derived("skill.perception"));
    assert!(!mapping.is_derived("save.str"));
}

#[test]
fn a_target_the_pack_does_not_declare_fails() {
    let mut block = good();
    block["fields"]["abilities.dex"] = json!("ability_data.dexterity");
    assert!(error(block).contains("ability_data.dexterity"));
}

#[test]
fn an_unknown_data_type_fails() {
    let mut block = good();
    block["fields"]["abilities.str"] = json!("nonsense_data.strength");
    assert!(error(block).contains("nonsense_data"));
}

#[test]
fn an_unknown_ability_type_fails() {
    let mut block = good();
    block["content"]["feat"] = json!({ "ability": "feat" });
    assert!(error(block).contains("feat"));
}

#[test]
fn a_malformed_content_target_fails() {
    let mut block = good();
    block["content"]["spell"] = json!({ "item": "yes" });
    assert!(error(block).contains("content.spell"));
}

#[test]
fn a_path_that_does_not_parse_fails() {
    let mut block = good();
    block["derived"] = json!(["Skill Perception"]);
    assert!(error(block).contains("derived"));
    let mut block = good();
    block["playState"] = json!(["a..b"]);
    assert!(error(block).contains("playState"));
}

#[test]
fn notes_must_name_a_declared_field() {
    let mut block = good();
    block["notes"] = json!("trait_data.backstory");
    assert!(error(block).contains("notes"));
    let mut block = good();
    block.as_object_mut().unwrap().remove("notes");
    assert!(error(block).contains("notes"));
}

#[test]
fn an_unknown_key_fails() {
    let mut block = good();
    block["fieldz"] = json!({});
    assert!(error(block).contains("fieldz"));
}

#[test]
fn a_reader_must_be_named() {
    let mut block = good();
    block["readers"] = json!([]);
    assert!(error(block).contains("readers"));
}

#[test]
fn the_nearest_declared_part_covers_a_path() {
    let mut block = good();
    block["fields"] =
        json!({ "classes": "trait_data.notes", "abilities.str": "ability_data.strength" });
    let mapping = SheetMapping::from_manifest(&manifest(block))
        .unwrap()
        .unwrap();
    assert_eq!(mapping.field_for("classes.0.level").unwrap().0, "classes");
    assert!(mapping.field_for("classesx").is_none());
    assert_eq!(
        mapping.field_for("abilities.str").unwrap().0,
        "abilities.str"
    );
}

#[test]
fn a_reading_with_an_unknown_key_is_refused() {
    let refused = serde_json::from_value::<thunderforge_sheet_import::ImportedCharacter>(json!({
        "identity": { "name": { "value": "x", "certainty": { "kind": "read" } }, "sneaky": 1 }
    }));
    assert!(refused.is_err());
    let fine = serde_json::from_value::<thunderforge_sheet_import::ImportedCharacter>(json!({
        "identity": { "name": { "value": "x", "certainty": { "kind": "read" } } }
    }))
    .unwrap();
    assert_eq!(fine.identity.name.value.as_deref(), Some("x"));
}
