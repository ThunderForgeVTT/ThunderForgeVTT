//! Spec 048 T026: the 5e fields a whole character sheet adds
//! (data-model.md, "5e pack: fields added").

use crate::validators::{validate_resource_data, validate_spell_data, validate_trait_data};
use serde_json::{json, Value};

fn refused_at(result: Result<(), crate::validators::ValidationError>) -> String {
    result.expect_err("must be refused").field
}

fn class(name: &str, level: i64, die: &str) -> Value {
    json!({ "name": name, "level": level, "hit_die": die })
}

#[test]
fn classes_take_a_name_a_level_and_a_hit_die() {
    let one = json!({ "classes": [class("Fighter", 5, "d10")] });
    assert!(validate_trait_data(&one).is_ok());
    let two = json!({
        "classes": [
            { "name": "Fighter", "subclass": "Champion", "level": 3, "hit_die": "d10" },
            class("Wizard", 2, "d6")
        ],
        "level": 5,
        "class": "Fighter"
    });
    assert!(validate_trait_data(&two).is_ok());
    for die in ["d6", "d8", "d10", "d12"] {
        let ok = json!({ "classes": [class("Any", 1, die)] });
        assert!(validate_trait_data(&ok).is_ok(), "{die}");
    }
}

#[test]
fn a_class_level_is_one_to_twenty() {
    for level in [1, 20] {
        let ok = json!({ "classes": [class("Fighter", level, "d10")] });
        assert!(validate_trait_data(&ok).is_ok(), "{level}");
    }
    for level in [0, 21] {
        let bad = json!({ "classes": [class("Fighter", level, "d10")] });
        assert_eq!(
            refused_at(validate_trait_data(&bad)),
            "trait_data.classes[0].level"
        );
    }
}

#[test]
fn a_hit_die_is_one_the_game_has() {
    let bad = json!({ "classes": [class("Fighter", 3, "d20")] });
    assert_eq!(
        refused_at(validate_trait_data(&bad)),
        "trait_data.classes[0].hit_die"
    );
}

#[test]
fn a_class_needs_a_name() {
    let bad = json!({ "classes": [{ "name": "", "level": 1, "hit_die": "d8" }] });
    assert_eq!(
        refused_at(validate_trait_data(&bad)),
        "trait_data.classes[0].name"
    );
    let unknown =
        json!({ "classes": [{ "name": "Bard", "level": 1, "hit_die": "d8", "colour": "red" }] });
    assert_eq!(
        refused_at(validate_trait_data(&unknown)),
        "trait_data.classes[0]"
    );
}

#[test]
fn level_is_the_sum_of_the_classes() {
    let wrong = json!({
        "classes": [class("Fighter", 3, "d10"), class("Wizard", 2, "d6")],
        "level": 6
    });
    assert_eq!(refused_at(validate_trait_data(&wrong)), "trait_data.level");
    let over = json!({
        "classes": [class("Fighter", 15, "d10"), class("Wizard", 6, "d6")]
    });
    assert_eq!(refused_at(validate_trait_data(&over)), "trait_data.classes");
}

#[test]
fn class_is_the_first_class_named() {
    let wrong = json!({
        "classes": [class("Fighter", 3, "d10"), class("Wizard", 2, "d6")],
        "class": "Wizard"
    });
    assert_eq!(refused_at(validate_trait_data(&wrong)), "trait_data.class");
}

#[test]
fn persona_text_has_a_length_limit() {
    let at = |key: &str, length: usize| json!({ key: "x".repeat(length) });
    for key in [
        "age", "height", "weight", "eyes", "skin", "hair", "gender", "faith",
    ] {
        assert!(validate_trait_data(&at(key, 100)).is_ok(), "{key}");
        assert_eq!(
            refused_at(validate_trait_data(&at(key, 101))),
            format!("trait_data.{key}")
        );
        assert!(validate_trait_data(&json!({ key: 5 })).is_err(), "{key}");
    }
    for key in [
        "personality_traits",
        "ideals",
        "bonds",
        "flaws",
        "backstory",
        "allies_and_organizations",
    ] {
        assert!(validate_trait_data(&at(key, 8000)).is_ok(), "{key}");
        assert_eq!(
            refused_at(validate_trait_data(&at(key, 8001))),
            format!("trait_data.{key}")
        );
    }
}

#[test]
fn defences_name_damage_types_the_pack_declares() {
    for key in ["resistances", "immunities", "vulnerabilities"] {
        let ok = json!({ key: ["poison", "fire"] });
        assert!(validate_trait_data(&ok).is_ok(), "{key}");
        let unknown = json!({ key: ["poison", "sadness"] });
        assert_eq!(
            refused_at(validate_trait_data(&unknown)),
            format!("trait_data.{key}[1]")
        );
        assert!(validate_trait_data(&json!({ key: "poison" })).is_err());
    }
}

#[test]
fn the_thirteen_damage_types_are_declared() {
    let all = json!({ "resistances": [
        "acid", "bludgeoning", "cold", "fire", "force", "lightning", "necrotic",
        "piercing", "poison", "psychic", "radiant", "slashing", "thunder"
    ]});
    assert!(validate_trait_data(&all).is_ok());
}

#[test]
fn condition_immunities_name_declared_conditions() {
    let ok = json!({ "condition_immunities": ["poisoned", "charmed"] });
    assert!(validate_trait_data(&ok).is_ok());
    let bad = json!({ "condition_immunities": ["disease"] });
    assert_eq!(
        refused_at(validate_trait_data(&bad)),
        "trait_data.condition_immunities[0]"
    );
}

#[test]
fn hit_dice_pools_never_spend_more_than_they_hold() {
    let ok = json!({ "max_hp": 5, "hit_dice_pools": [
        { "die": "d10", "total": 3, "used": 3 },
        { "die": "d6", "total": 2, "used": 0 }
    ]});
    assert!(validate_resource_data(&ok).is_ok());
    let over = json!({ "max_hp": 5, "hit_dice_pools": [{ "die": "d10", "total": 3, "used": 4 }] });
    assert_eq!(
        refused_at(validate_resource_data(&over)),
        "resource_data.hit_dice_pools[0].used"
    );
    let negative =
        json!({ "max_hp": 5, "hit_dice_pools": [{ "die": "d10", "total": 3, "used": -1 }] });
    assert!(validate_resource_data(&negative).is_err());
    let die = json!({ "max_hp": 5, "hit_dice_pools": [{ "die": "d4", "total": 1, "used": 0 }] });
    assert_eq!(
        refused_at(validate_resource_data(&die)),
        "resource_data.hit_dice_pools[0].die"
    );
}

#[test]
fn a_multiclass_hit_dice_line_is_accepted() {
    for line in ["3d10 + 2d6", "3d10+2d6", "5d10", "8d10+24", "3d6-3"] {
        let data = json!({ "max_hp": 5, "hit_dice": line });
        assert!(validate_resource_data(&data).is_ok(), "{line}");
    }
    for line in ["3d10 +", "d10", "3d10 + 2d6 + 2d6 + x", "+3d10"] {
        let data = json!({ "max_hp": 5, "hit_dice": line });
        assert!(validate_resource_data(&data).is_err(), "{line}");
    }
}

#[test]
fn coins_are_whole_and_never_negative() {
    let ok = json!({ "max_hp": 5, "coins": { "cp": 0, "sp": 40, "ep": 0, "gp": 1215, "pp": 3 } });
    assert!(validate_resource_data(&ok).is_ok());
    let some = json!({ "max_hp": 5, "coins": { "gp": 10 } });
    assert!(validate_resource_data(&some).is_ok());
    let negative = json!({ "max_hp": 5, "coins": { "gp": -1 } });
    assert_eq!(
        refused_at(validate_resource_data(&negative)),
        "resource_data.coins.gp"
    );
    let fraction = json!({ "max_hp": 5, "coins": { "gp": 1.5 } });
    assert!(validate_resource_data(&fraction).is_err());
    let unknown = json!({ "max_hp": 5, "coins": { "doubloons": 3 } });
    assert_eq!(
        refused_at(validate_resource_data(&unknown)),
        "resource_data.coins.doubloons"
    );
}

#[test]
fn spellcasting_classes_name_one_of_the_six_abilities() {
    let ok = json!({ "spellcasting_classes": [
        { "class": "Wizard", "ability": "intelligence", "save_dc": 14, "attack_bonus": 6 },
        { "class": "Cleric", "ability": "wisdom", "save_dc": 13, "attack_bonus": 5 }
    ]});
    assert!(validate_spell_data(&ok).is_ok());
    let bad = json!({ "spellcasting_classes": [
        { "class": "Wizard", "ability": "luck", "save_dc": 14, "attack_bonus": 6 }
    ]});
    assert_eq!(
        refused_at(validate_spell_data(&bad)),
        "spell_data.spellcasting_classes[0].ability"
    );
    let dc = json!({ "spellcasting_classes": [
        { "class": "Wizard", "ability": "intelligence", "save_dc": 4, "attack_bonus": 6 }
    ]});
    assert_eq!(
        refused_at(validate_spell_data(&dc)),
        "spell_data.spellcasting_classes[0].save_dc"
    );
}

#[test]
fn pact_slots_are_level_one_to_five_and_never_overspent() {
    for level in [1, 5] {
        let ok = json!({ "pact_slots": { "level": level, "total": 2, "used": 2 } });
        assert!(validate_spell_data(&ok).is_ok(), "{level}");
    }
    for level in [0, 6] {
        let bad = json!({ "pact_slots": { "level": level, "total": 2, "used": 0 } });
        assert_eq!(
            refused_at(validate_spell_data(&bad)),
            "spell_data.pact_slots.level"
        );
    }
    let over = json!({ "pact_slots": { "level": 3, "total": 2, "used": 3 } });
    assert_eq!(
        refused_at(validate_spell_data(&over)),
        "spell_data.pact_slots.used"
    );
}

#[test]
fn the_manifest_declares_every_new_field() {
    let manifest: Value = serde_json::from_str(include_str!("../../system.json")).unwrap();
    let declared = |slot: &str, field: &str| {
        manifest
            .pointer(&format!("/data_types/{slot}/properties/{field}"))
            .is_some()
    };
    for field in [
        "classes",
        "age",
        "height",
        "weight",
        "eyes",
        "skin",
        "hair",
        "gender",
        "faith",
        "personality_traits",
        "ideals",
        "bonds",
        "flaws",
        "backstory",
        "allies_and_organizations",
        "resistances",
        "immunities",
        "vulnerabilities",
        "condition_immunities",
    ] {
        assert!(declared("trait_data", field), "trait_data.{field}");
    }
    for field in ["hit_dice_pools", "coins"] {
        assert!(declared("resource_data", field), "resource_data.{field}");
    }
    for field in ["spellcasting_classes", "pact_slots"] {
        assert!(declared("spell_data", field), "spell_data.{field}");
    }
    let types: Vec<&str> = manifest["abilityVocabulary"]["types"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str())
        .collect();
    assert!(types.contains(&"feature") && types.contains(&"species_trait"));
    assert_eq!(manifest["damageTypes"].as_array().map(Vec::len), Some(13));
}
