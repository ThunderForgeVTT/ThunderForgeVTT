use super::*;
use serde_json::json;

#[test]
fn armor_class_is_optional_a_whole_number_and_never_negative() {
    let with = |armor_class: serde_json::Value| {
        json!({
            "strength": 10, "dexterity": 12, "constitution": 14,
            "intelligence": 9, "wisdom": 16, "charisma": 11,
            "armor_class": armor_class
        })
    };
    assert!(validate_ability_data(&with(json!(15))).is_ok());
    assert!(validate_ability_data(&with(json!(0))).is_ok());
    let negative = validate_ability_data(&with(json!(-1))).unwrap_err();
    assert_eq!(negative.field, "ability_data.armor_class");
    assert!(validate_ability_data(&with(json!("15"))).is_err());
    assert!(validate_ability_data(&with(json!(12.5))).is_err());
}

#[test]
fn test_validate_ability_data_valid() {
    let data = json!({
        "strength": 10,
        "dexterity": 12,
        "constitution": 14,
        "intelligence": 9,
        "wisdom": 16,
        "charisma": 11
    });
    assert!(validate_ability_data(&data).is_ok());
}

/// **The boundaries themselves**, written as literals.
///
/// A mutation audit on 2026-09-02 narrowed this rule from 1-20 to 2-19 and
/// all thirty-seven tests in this pack still passed: the accept fixture
/// uses 9 to 16, and the reject cases sit outside both ends, so the two
/// scores the rule actually names were never supplied. One is a real
/// character — a 20 is the cap a player spends their whole progression
/// reaching — and the rule refusing it would be discovered at a table.
///
/// Literals rather than the range's own bounds: a test written against the
/// constant asserts the rule accepts whatever the rule is written against,
/// which is true of every range and catches nothing. That mistake was made
/// and caught while fixing this same class of bug in `thunderforge-system-year-zero-engine`.
#[test]
fn ability_data_accepts_the_exact_ends_of_the_range() {
    let at = |score: i64| {
        json!({
            "strength": score, "dexterity": score, "constitution": score,
            "intelligence": score, "wisdom": score, "charisma": score
        })
    };

    assert!(
        validate_ability_data(&at(1)).is_ok(),
        "the lowest score the rule names must be accepted by it"
    );
    assert!(
        validate_ability_data(&at(20)).is_ok(),
        "and a 20 is where a character's whole progression ends up"
    );
    assert!(
        validate_ability_data(&at(30)).is_ok(),
        "thirty is the game's own ceiling, and a stat block may sit on it"
    );
    assert!(
        validate_ability_data(&at(31)).is_err(),
        "and nothing in the game is past it"
    );
}

#[test]
fn test_validate_ability_data_out_of_range() {
    let data = json!({
        "strength": 31,
        "dexterity": 12,
        "constitution": 14,
        "intelligence": 9,
        "wisdom": 16,
        "charisma": 11
    });
    assert!(validate_ability_data(&data).is_err());
}

#[test]
fn test_validate_ability_data_zero() {
    let data = json!({
        "strength": 0,
        "dexterity": 12,
        "constitution": 14,
        "intelligence": 9,
        "wisdom": 16,
        "charisma": 11
    });
    assert!(validate_ability_data(&data).is_err());
}

#[test]
fn test_validate_resource_data_valid() {
    let data = json!({
        "max_hp": 32,
        "current_hp": 28,
        "temporary_hp": 5
    });
    assert!(validate_resource_data(&data).is_ok());
}

#[test]
fn test_validate_resource_data_missing_max_hp() {
    let data = json!({"current_hp": 28});
    assert!(validate_resource_data(&data).is_err());
}

#[test]
fn test_validate_resource_data_negative_current_hp() {
    let data = json!({
        "max_hp": 32,
        "current_hp": -5
    });
    assert!(validate_resource_data(&data).is_err());
}

#[test]
fn test_validate_proficiency_data_valid_skills() {
    let data = json!({
        "skill_proficiencies": {
            "acrobatics": true,
            "arcana": false
        }
    });
    assert!(validate_proficiency_data(&data).is_ok());
}

#[test]
fn test_validate_proficiency_data_invalid_skill() {
    let data = json!({
        "skill_proficiencies": {
            "invalid_skill": true
        }
    });
    assert!(validate_proficiency_data(&data).is_err());
}

#[test]
fn proficiency_data_accepts_the_list_the_rules_read() {
    // The shape `rules.rs` and the roll-check bindings actually consult.
    let data = json!({
        "skill_proficiencies": ["stealth", "perception"],
        "saving_throw_proficiencies": ["dexterity"]
    });
    assert!(validate_proficiency_data(&data).is_ok());
}

#[test]
fn proficiency_data_accepts_expertise_and_rejects_an_unknown_skill_in_it() {
    let data = json!({
        "skill_proficiencies": ["stealth"],
        "skill_expertise": ["stealth"]
    });
    assert!(validate_proficiency_data(&data).is_ok());
    let unknown = json!({ "skill_expertise": ["lockpicking"] });
    assert!(validate_proficiency_data(&unknown).is_err());
}

#[test]
fn proficiency_data_rejects_an_unknown_id_in_either_shape() {
    let as_list = json!({ "skill_proficiencies": ["lockpicking"] });
    assert!(validate_proficiency_data(&as_list).is_err());
    let as_map = json!({ "saving_throw_proficiencies": { "luck": true } });
    assert!(validate_proficiency_data(&as_map).is_err());
    let as_number = json!({ "skill_proficiencies": 3 });
    assert!(validate_proficiency_data(&as_number).is_err());
    let not_a_string = json!({ "skill_proficiencies": [7] });
    assert!(validate_proficiency_data(&not_a_string).is_err());
}

#[test]
fn test_validate_trait_data_valid() {
    let data = json!({
        "class": "Wizard",
        "level": 5,
        "race": "Elf",
        "feats": ["War Caster"]
    });
    assert!(validate_trait_data(&data).is_ok());
}

#[test]
fn a_monster_has_a_size_and_neither_class_nor_level() {
    // What the bestiary writes: the size alone.
    assert!(validate_trait_data(&json!({ "size": "large" })).is_ok());
    assert!(validate_trait_data(&json!({ "level": 5 })).is_ok());
    // Present is still checked.
    let class = validate_trait_data(&json!({ "class": 3 })).unwrap_err();
    assert_eq!(class.field, "trait_data.class");
    let level = validate_trait_data(&json!({ "level": "five" })).unwrap_err();
    assert_eq!(level.field, "trait_data.level");
    let low = validate_trait_data(&json!({ "size": "large", "level": 0 })).unwrap_err();
    assert_eq!(low.field, "trait_data.level");
}

#[test]
fn size_is_optional_and_one_of_the_manifests_declared_sizes() {
    assert_eq!(
        declared_size_ids(),
        ["tiny", "small", "medium", "large", "huge", "gargantuan"],
        "the ids come from combat.sizes in system.json"
    );
    let with = |size: serde_json::Value| json!({ "class": "monster", "level": 1, "size": size });
    for size in declared_size_ids() {
        assert!(validate_trait_data(&with(json!(size))).is_ok(), "{size}");
    }
    assert!(validate_trait_data(&with(serde_json::Value::Null)).is_ok());
    assert!(validate_trait_data(&json!({ "class": "monster", "level": 1 })).is_ok());
    let unknown = validate_trait_data(&with(json!("colossal"))).unwrap_err();
    assert_eq!(unknown.field, "trait_data.size");
    let not_text = validate_trait_data(&with(json!(2))).unwrap_err();
    assert_eq!(not_text.field, "trait_data.size");
}

#[test]
fn legendary_actions_are_optional_whole_and_never_negative() {
    let with = |legendary: serde_json::Value| json!({ "class": "monster", "level": 1, "legendary_actions": legendary });
    assert!(validate_trait_data(&with(json!(3))).is_ok());
    assert!(validate_trait_data(&with(json!(0))).is_ok());
    assert!(validate_trait_data(&with(serde_json::Value::Null)).is_ok());
    let negative = validate_trait_data(&with(json!(-1))).unwrap_err();
    assert_eq!(negative.field, "trait_data.legendary_actions");
    let fraction = validate_trait_data(&with(json!(1.5))).unwrap_err();
    assert_eq!(fraction.field, "trait_data.legendary_actions");
    let text = validate_trait_data(&with(json!("three"))).unwrap_err();
    assert_eq!(text.field, "trait_data.legendary_actions");
}

#[test]
fn test_validate_trait_data_invalid_level() {
    let data = json!({
        "class": "Wizard",
        "level": 21
    });
    assert!(validate_trait_data(&data).is_err());
}

#[test]
fn test_validate_spell_data_valid() {
    let data = json!({
        "spellcasting_ability": "intelligence",
        "spell_save_dc": 14,
        "cantrips_known": ["Fire Bolt"],
        "spells_known": ["Magic Missile"],
        "spell_slots": {
            "level_1": 4,
            "level_2": 2
        }
    });
    assert!(validate_spell_data(&data).is_ok());
}

#[test]
fn test_validate_spell_data_invalid_ability() {
    let data = json!({
        "spellcasting_ability": "invalid_ability"
    });
    assert!(validate_spell_data(&data).is_err());
}

#[test]
fn test_validate_spell_data_invalid_dc() {
    let data = json!({"spell_save_dc": 31});
    assert!(validate_spell_data(&data).is_err());
}

/// An ogre's Strength is 19 and a giant's 25. While the rule stopped at 20 a
/// bestiary creature's stat block could not be saved at all.
#[test]
fn ability_data_accepts_a_score_past_a_characters_cap() {
    let data = json!({
        "strength": 25, "dexterity": 9, "constitution": 23,
        "intelligence": 10, "wisdom": 14, "charisma": 13,
        "armor_class": 18
    });
    assert!(validate_ability_data(&data).is_ok());
}

/// **A sheet saved before these fields were checked is still a sheet.**
///
/// Everything the sheet writes beside the hit points, at the values it
/// writes them.
#[test]
fn resource_data_accepts_what_the_sheet_has_always_written() {
    let data = json!({
        "max_hp": 12, "current_hp": 0, "temporary_hp": 0,
        "hit_dice_used": 2, "death_save_successes": 3, "death_save_failures": 0
    });
    assert!(validate_resource_data(&data).is_ok());
}

#[test]
fn resource_data_refuses_a_fourth_death_save() {
    for key in ["death_save_successes", "death_save_failures"] {
        let mut data = json!({"max_hp": 12});
        data[key] = json!(4);
        let refused = validate_resource_data(&data).expect_err("four is past the track");
        assert_eq!(refused.field, format!("resource_data.{key}"));
        data[key] = json!(-1);
        assert!(validate_resource_data(&data).is_err());
    }
}

#[test]
fn resource_data_reads_hit_dice_as_dice() {
    for good in ["3d6", "8d10+24", "3d6-3", "1d4", "3d6+2d4"] {
        let data = json!({"max_hp": 5, "hit_dice": good});
        assert!(validate_resource_data(&data).is_ok(), "{good}");
    }
    for bad in ["", "d6", "3d", "three d six", "3d6+", "3x6"] {
        let data = json!({"max_hp": 5, "hit_dice": bad});
        assert!(validate_resource_data(&data).is_err(), "{bad:?}");
    }
    assert!(validate_resource_data(&json!({"max_hp": 5, "hit_dice": 6})).is_err());
    assert!(validate_resource_data(&json!({"max_hp": 5, "hit_dice": null})).is_ok());
}

#[test]
fn trait_data_accepts_a_creature() {
    let data = json!({
        "challenge": "1/4", "creature_type": "Fey (Goblinoid)", "size": "small",
        "speed_walk": 30, "speed_fly": 0, "darkvision": 60, "blindsight": 10,
        "alignment": "Chaotic Neutral", "notes": "Nimble Escape.",
        "experience": 50, "inspiration": false
    });
    assert!(validate_trait_data(&data).is_ok());
}

#[test]
fn trait_data_refuses_a_challenge_rating_the_book_does_not_print() {
    for bad in [json!("1/3"), json!("31"), json!(0.25), json!(5)] {
        let refused =
            validate_trait_data(&json!({"challenge": bad})).expect_err("not a challenge rating");
        assert_eq!(refused.field, "trait_data.challenge");
    }
    for good in ["0", "1/8", "1/4", "1/2", "1", "30"] {
        assert!(
            validate_trait_data(&json!({"challenge": good})).is_ok(),
            "{good}"
        );
    }
}

#[test]
fn trait_data_refuses_a_negative_or_wordy_distance() {
    for key in [
        "speed_walk",
        "speed_fly",
        "speed_swim",
        "speed_climb",
        "speed_burrow",
        "darkvision",
        "blindsight",
        "tremorsense",
        "truesight",
        "light_bright",
        "light_dim",
    ] {
        let mut data = json!({});
        data[key] = json!(-5);
        let refused = validate_trait_data(&data).expect_err("a negative distance");
        assert_eq!(refused.field, format!("trait_data.{key}"));
        data[key] = json!("thirty");
        assert!(validate_trait_data(&data).is_err());
        data[key] = json!(0);
        assert!(validate_trait_data(&data).is_ok());
    }
}

#[test]
fn trait_data_refuses_the_wrong_kind_of_value_in_a_sheet_field() {
    assert!(validate_trait_data(&json!({"experience": -1})).is_err());
    assert!(validate_trait_data(&json!({"inspiration": "yes"})).is_err());
    assert!(validate_trait_data(&json!({"notes": 4})).is_err());
    assert!(validate_trait_data(&json!({"alignment": ["good"]})).is_err());
}

#[test]
fn proficiency_data_accepts_a_creatures_bonus_and_refuses_a_tenth() {
    assert!(validate_proficiency_data(&json!({"proficiency_bonus": 9})).is_ok());
    assert!(validate_proficiency_data(&json!({"proficiency_bonus": 10})).is_err());
    assert!(validate_proficiency_data(&json!({"proficiency_bonus": 1})).is_err());
}

#[test]
fn proficiency_data_refuses_a_training_list_that_is_not_text() {
    let fine = json!({"armor": ["Light armor"], "weapons": [], "tools": []});
    assert!(validate_proficiency_data(&fine).is_ok());
    assert!(validate_proficiency_data(&json!({"weapons": [3]})).is_err());
    assert!(validate_proficiency_data(&json!({"tools": "thieves' tools"})).is_err());
}

#[test]
fn spell_data_holds_spent_slots_to_the_same_rule_as_the_slots() {
    assert!(validate_spell_data(&json!({
        "spell_slots": {"level_1": 4}, "spell_slots_used": {"level_1": 1}
    }))
    .is_ok());
    assert!(validate_spell_data(&json!({"spell_slots_used": {"level_1": -1}})).is_err());
    assert!(validate_spell_data(&json!({"spell_slots_used": {"level_10": 1}})).is_err());
    assert!(validate_spell_data(&json!({"spell_slots_used": [1]})).is_err());
}

// Spec 084: the roll facets a character has, and the Luck Points it spent.
#[test]
fn trait_data_facets_are_known_ids_listed_once() {
    let all = json!({ "facets": ["halfling_luck", "great_weapon_fighting", "lucky"] });
    assert!(validate_trait_data(&all).is_ok());
    assert!(validate_trait_data(&json!({ "facets": [] })).is_ok());
    assert!(validate_trait_data(&json!({ "facets": null })).is_ok());

    for bad in [
        json!({ "facets": ["advantage"] }),
        json!({ "facets": ["lucky", "lucky"] }),
        json!({ "facets": [3] }),
        json!({ "facets": "lucky" }),
    ] {
        let error = validate_trait_data(&bad).unwrap_err();
        assert_eq!(error.field, "trait_data.facets", "{bad}");
    }
}

#[test]
fn luck_points_used_is_a_whole_number_of_zero_or_more() {
    for used in [0, 1, 6] {
        assert!(validate_trait_data(&json!({ "luck_points_used": used })).is_ok());
    }
    for bad in [json!(-1), json!(1.5), json!("2")] {
        let error = validate_trait_data(&json!({ "luck_points_used": bad })).unwrap_err();
        assert_eq!(error.field, "trait_data.luck_points_used", "{bad}");
    }
}
