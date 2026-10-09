//! Spec 048 T033: each fixture's reading, planned onto an empty 5e actor
//! through the pack's own declaration and refine hook
//! (contracts/sheet-mapping-5e.md).

use super::{refine, refine_json, SHEET_IMPORT};
use crate::validators::{
    validate_ability_data, validate_proficiency_data, validate_resource_data, validate_spell_data,
    validate_trait_data,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{
    plan, ActorSnapshot, ContentChange, ContentIndex, ContentTarget, Corrections, Field,
    ImportPlan, ImportedCharacter, Indexed, PlanCertainty, SheetMapping, SheetReader, SkillMark,
};
use thunderforge_system_dnd5e_sheet::DdbPdf;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sheet/tests/fixtures");

/// Every fixture the reader takes as a D&D Beyond sheet.
const SHEETS: [&str; 7] = [
    "fighter-5.pdf",
    "fighter3-wizard2.pdf",
    "fighter3-wizard2-l6.pdf",
    "cleric-7.pdf",
    "rogue-4.pdf",
    "warforged-defences.pdf",
    "uncertain-mark.pdf",
];

fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{file}")).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn reading(file: &str) -> ImportedCharacter {
    let doc = Document::from_bytes(&bytes(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    DdbPdf.read(&doc).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn manifest() -> Value {
    serde_json::from_str(include_str!("../../system.json")).expect("system.json parses")
}

fn mapping() -> SheetMapping {
    SheetMapping::from_manifest(&manifest())
        .expect("the 5e sheetImport block is valid")
        .expect("the 5e manifest declares sheetImport")
}

/// A world with nothing in it: every piece of content is new.
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

/// What the plan writes to `target`, which it must write.
fn new(plan: &ImportPlan, target: &str) -> Value {
    let change = plan
        .fields
        .iter()
        .find(|f| f.target == target)
        .unwrap_or_else(|| panic!("no change to {target}"));
    change
        .new
        .clone()
        .unwrap_or_else(|| panic!("{target} is planned with no value"))
}

fn content<'a>(plan: &'a ImportPlan, kind: &str, name: &str) -> &'a ContentChange {
    plan.content
        .iter()
        .find(|c| c.kind == kind && c.name == name)
        .unwrap_or_else(|| panic!("no {kind} {name}"))
}

/// The data types an apply onto an empty actor would write.
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
fn the_declaration_parses_against_the_manifest() {
    let mapping = mapping();
    assert_eq!(mapping.readers, ["ddb-pdf"]);
    assert_eq!(mapping.notes, "trait_data.notes");
    assert_eq!(
        mapping.content.get("attack"),
        Some(&ContentTarget::Refine),
        "the hook decides whether an attack is an item or an ability"
    );
    assert_eq!(
        mapping.content.get("item"),
        Some(&ContentTarget::Item),
        "equipment lands as items"
    );
}

#[test]
fn a_fighter_lands_on_every_target_in_the_table() {
    let plan = planned("fighter-5.pdf");

    assert_eq!(new(&plan, "actor.label"), json!("Brannoc Vell"));
    assert_eq!(new(&plan, "trait_data.race"), json!("Human"));
    assert_eq!(new(&plan, "trait_data.background"), json!("Soldier"));
    assert_eq!(new(&plan, "trait_data.alignment"), json!("Lawful Neutral"));
    assert_eq!(new(&plan, "trait_data.size"), json!("medium"));
    assert_eq!(new(&plan, "trait_data.experience"), json!(6500));
    assert_eq!(new(&plan, "trait_data.level"), json!(5));
    assert_eq!(new(&plan, "trait_data.class"), json!("Fighter"));
    for target in [
        "ability_data.strength",
        "ability_data.dexterity",
        "ability_data.constitution",
        "ability_data.intelligence",
        "ability_data.wisdom",
        "ability_data.charisma",
        "ability_data.armor_class",
        "proficiency_data.armor",
        "proficiency_data.weapons",
        "proficiency_data.tools",
        "proficiency_data.languages",
        "resource_data.max_hp",
        "resource_data.temporary_hp",
        "resource_data.death_save_successes",
        "resource_data.death_save_failures",
        "resource_data.coins",
        "trait_data.inspiration",
        "trait_data.speed_walk",
        "trait_data.resistances",
        "trait_data.immunities",
        "trait_data.vulnerabilities",
        "trait_data.condition_immunities",
    ] {
        new(&plan, target);
    }
    for (target, value) in [
        ("trait_data.age", "34"),
        ("trait_data.eyes", "Grey"),
        ("trait_data.hair", "Black, cropped"),
        ("trait_data.skin", "Weathered"),
        ("trait_data.gender", "Male"),
        ("trait_data.faith", "The Lantern Keeper"),
        ("trait_data.allies_and_organizations", "The Ford Wardens"),
    ] {
        assert_eq!(new(&plan, target), json!(value), "{target}");
    }
    for target in [
        "trait_data.height",
        "trait_data.weight",
        "trait_data.personality_traits",
        "trait_data.ideals",
        "trait_data.bonds",
        "trait_data.flaws",
        "trait_data.backstory",
    ] {
        new(&plan, target);
    }

    // Saves under the pack's own ability names, skills as yes or no.
    let saves = new(&plan, "proficiency_data.saving_throw_proficiencies");
    let saves = saves.as_object().expect("an object");
    assert_eq!(saves.len(), 6);
    assert!(saves.keys().all(|k| k.len() > 3), "{saves:?}");
    let skills = new(&plan, "proficiency_data.skill_proficiencies");
    assert!(skills
        .as_object()
        .expect("an object")
        .values()
        .all(Value::is_boolean));

    // The export leaves current hit points blank: unread, never invented.
    let current = plan
        .fields
        .iter()
        .find(|f| f.target == "resource_data.current_hp")
        .expect("current hit points are planned");
    assert_eq!(current.certainty, PlanCertainty::Unread);
    assert_eq!(current.new, None);

    // Derived numbers are checked, never written.
    assert!(plan
        .checked
        .contains(&"derived.proficiency_bonus".to_string()));
    assert!(plan
        .checked
        .contains(&"derived.skill.athletics".to_string()));
    assert!(plan
        .fields
        .iter()
        .all(|f| !f.target.ends_with("proficiency_bonus")));
    // The player's name is read and kept nowhere.
    assert_eq!(plan.ignored, ["identity.player_name"]);
    // Appearance has no field: it goes to the notes, labelled.
    let appearance = plan
        .unmapped
        .iter()
        .find(|u| u.path == "persona.appearance")
        .expect("appearance is unmapped");
    assert_eq!(appearance.goes_to, "trait_data.notes");

    assert!(plan.content.iter().all(|c| c.kind != "spell"));
    assert_eq!(content(&plan, "feat", "Alert").target, ability("feat"));
    assert_eq!(
        content(&plan, "item", "Chain Mail").target,
        ContentTarget::Item
    );
}

fn ability(vocabulary: &str) -> ContentTarget {
    ContentTarget::Ability {
        vocabulary: vocabulary.to_string(),
    }
}

#[test]
fn a_multiclass_level_is_the_sum_of_its_classes() {
    let plan = planned("fighter3-wizard2.pdf");
    assert_eq!(new(&plan, "trait_data.level"), json!(5));
    assert_eq!(new(&plan, "trait_data.class"), json!("Fighter"));
    assert_eq!(
        new(&plan, "trait_data.classes"),
        json!([
            { "name": "Fighter", "level": 3, "hit_die": "d10" },
            { "name": "Wizard", "level": 2, "hit_die": "d6" }
        ])
    );

    let six = planned("fighter3-wizard2-l6.pdf");
    assert_eq!(new(&six, "trait_data.level"), json!(6));
}

#[test]
fn hit_dice_fold_into_the_legacy_string_largest_first() {
    let plan = planned("fighter3-wizard2.pdf");
    assert_eq!(
        new(&plan, "resource_data.hit_dice_pools"),
        json!([
            { "die": "d10", "total": 3, "used": 0 },
            { "die": "d6", "total": 2, "used": 0 }
        ])
    );
    assert_eq!(new(&plan, "resource_data.hit_dice"), json!("3d10 + 2d6"));
    assert_eq!(new(&plan, "resource_data.hit_dice_used"), json!(0));
}

#[test]
fn used_hit_dice_are_the_table_s_and_carry_over() {
    let mut current = ActorSnapshot {
        is_reimport: true,
        ..ActorSnapshot::default()
    };
    current.values.insert(
        "resource_data.hit_dice_pools".to_string(),
        json!([{ "die": "d10", "total": 3, "used": 2 }]),
    );
    let plan = plan_onto(&reading("fighter3-wizard2.pdf"), &current);
    assert_eq!(
        new(&plan, "resource_data.hit_dice_pools"),
        json!([
            { "die": "d10", "total": 3, "used": 2 },
            { "die": "d6", "total": 2, "used": 0 }
        ])
    );
    assert_eq!(new(&plan, "resource_data.hit_dice_used"), json!(2));
}

#[test]
fn spellcasting_lands_per_class_and_the_first_is_the_actor_s() {
    let plan = planned("fighter3-wizard2.pdf");
    assert_eq!(
        new(&plan, "spell_data.spellcasting_classes"),
        json!([{ "class": "Wizard", "ability": "intelligence", "save_dc": 14, "attack_bonus": 6 }])
    );
    assert_eq!(
        new(&plan, "spell_data.spellcasting_ability"),
        json!("intelligence")
    );
    assert_eq!(new(&plan, "spell_data.spell_save_dc"), json!(14));
    assert_eq!(new(&plan, "spell_data.spell_attack_bonus"), json!(6));
    assert_eq!(
        new(&plan, "spell_data.spell_slots"),
        json!({ "level_1": 3 })
    );

    let cleric = planned("cleric-7.pdf");
    assert_eq!(
        new(&cleric, "spell_data.spell_slots"),
        json!({ "level_1": 4, "level_2": 3, "level_3": 3, "level_4": 1 })
    );
    assert_eq!(
        new(&cleric, "spell_data.spellcasting_ability"),
        json!("wisdom")
    );
    let bless = content(&cleric, "spell", "Bless");
    assert_eq!(bless.target, ability("spell"));
    assert_eq!(bless.link.granted_by, ["Life Domain"]);
    assert_eq!(bless.link.prepared, Some(true));
    assert_eq!(
        content(&cleric, "spell", "Guiding Bolt").link.prepared,
        Some(false)
    );
}

#[test]
fn a_weapon_in_the_equipment_carries_its_attack() {
    let plan = planned("fighter-5.pdf");
    let sword = content(&plan, "item", "Longsword");
    assert_eq!(sword.target, ContentTarget::Item);
    let attack = &sword.fields["attack"];
    assert_eq!(
        attack["effects"],
        json!([
            { "effect_type": "ATTACK_ROLL", "formula": "1d20+6" },
            { "effect_type": "DAMAGE", "formula": "1d8+3" }
        ])
    );
    assert_eq!(attack["reach"], json!(5));
    assert_eq!(attack["range_normal"], Value::Null);
    assert_eq!(attack["damage_type"], json!("slashing"));
    assert_eq!(attack["to_hit"], json!(6));
    // The row is the item's: it is not a second piece of content.
    assert!(!plan
        .content
        .iter()
        .any(|c| c.kind == "attack" && c.name == "Longsword"));
    assert_eq!(sword.from.len(), 2, "the item's row and the attack's row");

    let crossbow = &content(&plan, "item", "Light Crossbow").fields["attack"];
    assert_eq!(crossbow["reach"], Value::Null);
    assert_eq!(crossbow["range_normal"], json!(80));
    assert_eq!(crossbow["range_long"], json!(320));

    // A reshaped item is hashed again: the hash is of what will be staged.
    assert_eq!(
        sword.content_hash,
        thunderforge_sheet_import::content_hash("item", &sword.fields)
    );
}

#[test]
fn an_attack_with_nothing_to_ride_on_is_an_ability() {
    let plan = planned("fighter-5.pdf");
    let unarmed = content(&plan, "attack", "Unarmed Strike");
    assert_eq!(unarmed.target, ability("feature"));
    assert_eq!(
        unarmed.fields["attack"]["effects"],
        json!([
            { "effect_type": "ATTACK_ROLL", "formula": "1d20+6" },
            { "effect_type": "DAMAGE", "formula": "4" }
        ])
    );
    assert!(plan.unmapped.iter().all(|u| u.label != "Unarmed Strike"));

    let rogue = planned("rogue-4.pdf");
    let bow = content(&rogue, "attack", "Shortbow");
    assert_eq!(bow.target, ability("feature"));
    assert_eq!(bow.fields["attack"]["range_normal"], json!(80));
    assert!(content(&rogue, "item", "Shortsword").fields["attack"].is_object());
}

#[test]
fn a_cantrip_s_attack_rides_on_the_spell() {
    let plan = planned("fighter3-wizard2.pdf");
    let bolt = content(&plan, "spell", "Fire Bolt");
    assert_eq!(
        bolt.fields["attack"]["effects"][0]["formula"],
        json!("1d20+6")
    );
    assert_eq!(bolt.fields["attack"]["range_normal"], json!(120));
    assert!(!plan.content.iter().any(|c| c.kind == "attack"));

    // A save, not a roll to hit: no attack roll, and the save kept.
    let cleric = planned("cleric-7.pdf");
    let flame = &content(&cleric, "spell", "Sacred Flame").fields["attack"];
    assert_eq!(
        flame["effects"],
        json!([{ "effect_type": "DAMAGE", "formula": "2d8" }])
    );
    assert_eq!(flame["save_ability"], json!("dexterity"));
    assert_eq!(flame["save_dc"], json!(15));
}

#[test]
fn a_spell_two_classes_grant_is_one_change() {
    let mut sheet = reading("fighter3-wizard2.pdf");
    let mut again = sheet
        .content
        .iter()
        .find(|c| c.kind == "spell" && c.name.value.as_deref() == Some("Magic Missile"))
        .expect("the fixture has Magic Missile")
        .clone();
    again.link.granted_by = vec!["Sorcerer".to_string()];
    sheet.content.push(again);

    let plan = plan_onto(&sheet, &ActorSnapshot::default());
    let missiles: Vec<_> = plan
        .content
        .iter()
        .filter(|c| c.name == "Magic Missile")
        .collect();
    assert_eq!(missiles.len(), 1);
    assert_eq!(missiles[0].link.granted_by, ["Sorcerer", "Wizard"]);
}

#[test]
fn expertise_is_its_own_list_and_half_proficiency_is_a_note() {
    let plan = planned("rogue-4.pdf");
    let skills = new(&plan, "proficiency_data.skill_proficiencies");
    assert_eq!(skills["stealth"], json!(true));
    assert_eq!(skills["acrobatics"], json!(true));
    assert_eq!(skills["athletics"], json!(false));
    assert_eq!(
        new(&plan, "proficiency_data.skill_expertise"),
        json!(["sleight_of_hand", "stealth"])
    );
    assert_eq!(new(&plan, "trait_data.size"), json!("small"));

    let mut bard = reading("rogue-4.pdf");
    bard.proficiencies
        .skills
        .insert("history".to_string(), Field::read(SkillMark::Half, None));
    let plan = plan_onto(&bard, &ActorSnapshot::default());
    assert_eq!(
        new(&plan, "proficiency_data.skill_proficiencies")["history"],
        json!(false)
    );
    let half = plan
        .unmapped
        .iter()
        .find(|u| u.path == "proficiencies.skills.history")
        .expect("half proficiency is a note");
    assert_eq!(half.goes_to, "trait_data.notes");
}

#[test]
fn a_smudged_mark_is_uncertain_and_nothing_is_invented() {
    let plan = planned("uncertain-mark.pdf");
    let skills = plan
        .fields
        .iter()
        .find(|f| f.target == "proficiency_data.skill_proficiencies")
        .expect("skills are planned");
    assert_eq!(skills.certainty, PlanCertainty::Uncertain);
    assert!(skills
        .reason
        .as_deref()
        .is_some_and(|r| r.contains("Athletics")));
    let value = skills.new.as_ref().expect("the other skills are read");
    assert!(value.get("athletics").is_none(), "{value}");
}

#[test]
fn a_warforged_s_poison_is_a_resistance() {
    let plan = planned("warforged-defences.pdf");
    assert_eq!(new(&plan, "trait_data.resistances"), json!(["poison"]));
}

#[test]
fn a_printed_number_the_rules_disagree_with_makes_its_base_uncertain() {
    // The fixtures print what the rules give: nothing to flag.
    for file in SHEETS {
        assert!(planned(file).cross_checks.is_empty(), "{file}");
    }

    let mut sheet = reading("fighter3-wizard2.pdf");
    sheet
        .derived
        .insert("proficiency_bonus".to_string(), Field::read(4, None));
    sheet
        .derived
        .insert("modifier.str".to_string(), Field::read(5, None));
    let plan = plan_onto(&sheet, &ActorSnapshot::default());
    let paths: Vec<&str> = plan.cross_checks.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["derived.modifier.str", "derived.proficiency_bonus"]);
    for path in ["classes", "abilities.str"] {
        let field = plan.fields.iter().find(|f| f.path == path).expect(path);
        assert_eq!(field.certainty, PlanCertainty::Uncertain, "{path}");
    }
}

#[test]
fn a_reimport_keeps_play_state() {
    let mut current = ActorSnapshot {
        is_reimport: true,
        ..ActorSnapshot::default()
    };
    current
        .values
        .insert("resource_data.temporary_hp".to_string(), json!(7));
    let plan = plan_onto(&reading("fighter-5.pdf"), &current);
    let temp = plan
        .fields
        .iter()
        .find(|f| f.target == "resource_data.temporary_hp")
        .expect("temporary hit points are planned");
    assert!(temp.play_state);
    assert!(plan
        .kept_in_play
        .iter()
        .any(|k| k.target == "resource_data.temporary_hp"));
}

#[test]
fn every_sheet_plans_to_data_the_pack_s_validators_take() {
    for file in SHEETS {
        let plan = planned(file);
        let slots = written(&plan);
        let empty = json!({});
        let slot = |name: &str| slots.get(name).unwrap_or(&empty).clone();
        let checks: [(&str, fn(&Value) -> Result<(), crate::ValidationError>); 5] = [
            ("ability_data", validate_ability_data),
            ("resource_data", validate_resource_data),
            ("proficiency_data", validate_proficiency_data),
            ("trait_data", validate_trait_data),
            ("spell_data", validate_spell_data),
        ];
        for (name, validate) in checks {
            if let Err(error) = validate(&slot(name)) {
                panic!("{file}: {name}: {}: {}", error.field, error.message);
            }
        }
    }
}

#[test]
fn the_hook_over_json_is_the_hook() {
    let sheet = reading("cleric-7.pdf");
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
    let mut over_json = serde_json::to_value(&bare).expect("a plan serialises");
    refine_json(
        &serde_json::to_value(&sheet).expect("a reading serialises"),
        &serde_json::to_value(&current).expect("a snapshot serialises"),
        &mut over_json,
    );
    let over_json: ImportPlan = serde_json::from_value(over_json).expect("a plan comes back");
    assert_eq!(over_json, typed);
}

#[test]
fn the_slot_reads_sheets_and_names_each_refusal() {
    let reader = SHEET_IMPORT
        .reader("ddb-pdf")
        .expect("the slot has ddb-pdf");
    assert_eq!(reader.version(), thunderforge_system_dnd5e_sheet::VERSION);
    let json = reader.read(&bytes("fighter-5.pdf")).expect("a sheet reads");
    let back: ImportedCharacter = serde_json::from_str(&json).expect("a reading");
    assert_eq!(back, reading("fighter-5.pdf"));

    let code = |bytes: &[u8]| reader.read(bytes).expect_err("refused").code;
    assert_eq!(code(&bytes("not-a-ddb-sheet.pdf")), "SHEET_NOT_RECOGNISED");
    assert_eq!(code(b"not a pdf at all"), "SHEET_UNREADABLE");
    assert_eq!(code(&vec![b' '; 10 * 1024 * 1024 + 1]), "SHEET_TOO_LARGE");
    assert!(SHEET_IMPORT.refine.is_some());

    let contributed =
        thunderforge_canvas_core::system_contribution::contribution_for(crate::SYSTEM_ID)
            .expect("5e contributes");
    assert!(contributed
        .sheet_import
        .and_then(|import| import.reader("ddb-pdf"))
        .is_some());
}
