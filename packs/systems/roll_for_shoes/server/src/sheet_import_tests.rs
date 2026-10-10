//! Spec 048 T068: the one-page Roll for Shoes sheet, planned onto an actor
//! through the pack's own declaration and refine hook.

use super::{refine, refine_json, SHEET_IMPORT, SKILLS};
use crate::validators::{validate_resource_data, validate_trait_data};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{
    plan, ActorSnapshot, ContentIndex, Corrections, ImportPlan, ImportedCharacter, Indexed,
    PlanCertainty, SheetMapping, SheetReader,
};
use thunderforge_system_roll_for_shoes_sheet::RfsPdf;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sheet/tests/fixtures");

const SHEETS: [&str; 3] = ["wren-4.pdf", "wren-5.pdf", "misprinted-lineage.pdf"];

fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{file}")).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn reading(file: &str) -> ImportedCharacter {
    let doc = Document::from_bytes(&bytes(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    RfsPdf.read(&doc).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn mapping() -> SheetMapping {
    let manifest: Value =
        serde_json::from_str(include_str!("../../system.json")).expect("system.json parses");
    SheetMapping::from_manifest(&manifest)
        .expect("the sheetImport block is valid")
        .expect("the manifest declares sheetImport")
}

struct EmptyWorld;

impl ContentIndex for EmptyWorld {
    fn lookup(&self, _kind: &str, _normalised: &str) -> Option<Indexed> {
        None
    }
}

fn plan_onto(reading: &ImportedCharacter, current: &ActorSnapshot) -> ImportPlan {
    plan(
        &mapping(),
        Some(refine),
        reading,
        &Corrections::new(),
        current,
        &EmptyWorld,
    )
}

fn planned(file: &str) -> ImportPlan {
    plan_onto(&reading(file), &ActorSnapshot::default())
}

fn skills(plan: &ImportPlan) -> Vec<Value> {
    plan.fields
        .iter()
        .find(|f| f.target == SKILLS)
        .and_then(|f| f.new.clone())
        .and_then(|v| v.as_array().cloned())
        .expect("the skills are planned")
}

/// (name, level, parent's name) per skill, in the sheet's order.
fn lineage(skills: &[Value]) -> Vec<(String, u64, Option<String>)> {
    let name_of = |id: &Value| {
        skills
            .iter()
            .find(|s| &s["id"] == id)
            .map(|s| s["name"].as_str().unwrap().to_string())
    };
    skills
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_string(),
                s["level"].as_u64().unwrap(),
                if s["parentId"].is_null() {
                    None
                } else {
                    name_of(&s["parentId"])
                },
            )
        })
        .collect()
}

fn written(plan: &ImportPlan) -> Map<String, Value> {
    let mut slots = Map::new();
    for change in plan.writes(&BTreeSet::new()) {
        let (slot, field) = change.target.split_once('.').expect("dataType.field");
        if slot == "actor" {
            continue;
        }
        let entry = slots
            .entry(slot.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        entry[field] = change.new.clone().expect("a write has a value");
    }
    slots
}

#[test]
fn the_slot_reads_the_sheet_and_refuses_anything_else() {
    let reader = SHEET_IMPORT.readers[0];
    assert_eq!(reader.id(), "tf-rfs-pdf");
    let json = reader.read(&bytes("wren-4.pdf")).expect("the sheet reads");
    let back: ImportedCharacter = serde_json::from_str(&json).expect("a reading");
    assert_eq!(back.identity.name.value.as_deref(), Some("Wren Ashdown"));
    let refused = reader
        .read(&bytes("not-a-rfs-sheet.pdf"))
        .expect_err("a stat block is not this sheet");
    assert_eq!(refused.code, "SHEET_NOT_RECOGNISED");
}

#[test]
fn a_clean_sheet_lands_its_name_xp_and_lineage() {
    let plan = planned("wren-4.pdf");
    let label = plan
        .fields
        .iter()
        .find(|f| f.target == "actor.label")
        .unwrap();
    assert_eq!(label.new, Some(json!("Wren Ashdown")));
    let xp = plan
        .fields
        .iter()
        .find(|f| f.target == "resource_data.xp")
        .unwrap();
    assert_eq!(xp.new, Some(json!(2)));
    let skills_change = plan.fields.iter().find(|f| f.target == SKILLS).unwrap();
    assert_eq!(skills_change.certainty, PlanCertainty::Read);
    let root = Some("Do Anything".to_string());
    assert_eq!(
        lineage(&skills(&plan)),
        vec![
            ("Do Anything".into(), 1, None),
            ("Sneak".into(), 2, root.clone()),
            ("Hide in Shadows".into(), 3, Some("Sneak".into())),
            // The sheet says Pick Locks grew from row 2, which is Sneak.
            ("Climb".into(), 2, root),
            ("Pick Locks".into(), 3, Some("Sneak".into())),
        ]
    );
    assert!(plan.content.is_empty(), "a skill is staged for no one");
    assert!(plan.unmapped.is_empty(), "nothing is left for the notes");
}

#[test]
fn a_misprinted_lineage_is_uncertain_and_still_valid() {
    let plan = planned("misprinted-lineage.pdf");
    let change = plan.fields.iter().find(|f| f.target == SKILLS).unwrap();
    assert_eq!(change.certainty, PlanCertainty::Uncertain);
    let reason = change.reason.as_deref().unwrap();
    assert!(reason.contains("Bake"), "{reason}");
    assert!(reason.contains("not a number"), "{reason}");
    assert!(
        reason.contains("Bake Under Pressure is level 4"),
        "{reason}"
    );
    assert_eq!(
        lineage(&skills(&plan)),
        vec![
            ("Do Anything".into(), 1, None),
            // "two" is no level: one, under nothing it cannot be one above.
            ("Bake".into(), 1, None),
            ("Bake Under Pressure".into(), 4, None),
        ]
    );
}

#[test]
fn every_sheet_plans_to_data_the_validators_take() {
    for file in SHEETS {
        let slots = written(&planned(file));
        let empty = json!({});
        let slot = |name: &str| slots.get(name).unwrap_or(&empty).clone();
        validate_trait_data(&slot("trait_data"))
            .unwrap_or_else(|e| panic!("{file}: trait_data: {e}"));
        validate_resource_data(&slot("resource_data"))
            .unwrap_or_else(|e| panic!("{file}: resource_data: {e}"));
    }
}

#[test]
fn the_same_sheet_twice_is_identical() {
    let first = planned("wren-4.pdf");
    let mut current = ActorSnapshot {
        is_reimport: true,
        ..ActorSnapshot::default()
    };
    for change in &first.fields {
        if let Some(new) = &change.new {
            current.values.insert(change.target.clone(), new.clone());
        }
    }
    let again = plan_onto(&reading("wren-4.pdf"), &current);
    assert!(
        again.fields.iter().all(|f| f.target != SKILLS),
        "{:?}",
        again.fields
    );
    assert!(again.identical.iter().any(|p| p == "skills"));
}

#[test]
fn a_later_sheet_keeps_the_ids_the_actor_has() {
    let mut current = ActorSnapshot {
        is_reimport: true,
        ..ActorSnapshot::default()
    };
    // Ids the table gave these skills, not the sheet's.
    current.values.insert(
        SKILLS.to_string(),
        json!([
            {"id": "a", "name": "Do Anything", "level": 1, "parentId": null},
            {"id": "b", "name": "Sneak", "level": 2, "parentId": "a"},
            {"id": "c", "name": "Hide in Shadows", "level": 3, "parentId": "b"},
            {"id": "d", "name": "Climb", "level": 2, "parentId": "a"},
            {"id": "e", "name": "Pick Locks", "level": 3, "parentId": "b"},
        ]),
    );
    current
        .values
        .insert("resource_data.xp".to_string(), json!(2));
    let plan = plan_onto(&reading("wren-5.pdf"), &current);
    let skills = skills(&plan);
    let ids: Vec<&str> = skills.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["a", "b", "c", "d", "e", "sheet-6"]);
    assert_eq!(skills[5]["parentId"], json!("c"));
    let xp = plan
        .fields
        .iter()
        .find(|f| f.target == "resource_data.xp")
        .unwrap();
    assert!(xp.play_state, "XP is the table's on a re-import");
    let slots = written(&plan);
    assert!(slots.get("resource_data").is_none(), "{slots:?}");
}

#[test]
fn the_hook_over_json_is_the_hook() {
    let sheet = reading("misprinted-lineage.pdf");
    let current = ActorSnapshot::default();
    let bare = plan(
        &mapping(),
        None,
        &sheet,
        &Corrections::new(),
        &current,
        &EmptyWorld,
    );
    let mut typed = bare.clone();
    refine(&sheet, &current, &mut typed);
    let mut over_json = serde_json::to_value(&bare).unwrap();
    refine_json(
        &serde_json::to_value(&sheet).unwrap(),
        &serde_json::to_value(&current).unwrap(),
        &mut over_json,
    );
    assert_eq!(over_json, serde_json::to_value(&typed).unwrap());
}
