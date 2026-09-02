use super::*;

fn rules() -> DnD5eRules {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../system.json");
    let text = std::fs::read_to_string(path).expect("5e's manifest should be readable");
    DnD5eRules::from_manifest(&serde_json::from_str(&text).expect("valid manifest json"))
}

fn stored(id: &str, value: i32) -> DeclaredValue {
    DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: DeclaredValueKind::Integer(value),
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    }
}

fn list(id: &str, items: &[&str]) -> DeclaredValue {
    DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: DeclaredValueKind::List(items.iter().map(|i| i.to_string()).collect()),
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    }
}

/// A level-5 character with a 16 Dexterity, proficient in Stealth and in
/// Dexterity saves.
fn a_character() -> DeclaredValues {
    DeclaredValues::new([
        stored("strength", 10),
        stored("dexterity", 16),
        stored("constitution", 14),
        stored("intelligence", 8),
        stored("wisdom", 12),
        stored("charisma", 7),
        stored(LEVEL, 5),
        list(SKILL_PROFICIENCIES, &["stealth", "perception"]),
        list(SAVE_PROFICIENCIES, &["dexterity"]),
    ])
}

fn value_of(values: &[DeclaredValue], id: &str) -> Option<i32> {
    values
        .iter()
        .find(|v| v.id == id)
        .and_then(|v| v.value.as_integer())
}

/// **The bug this port was carrying.**
///
/// The staged Rust rule was `(score - 10) / 2`, and Rust's `/` rounds toward
/// zero. That is right for every even score and for odd scores above ten, and
/// wrong for every odd score below it — a 7 gave -1 where the book says -2.
/// It had sat in the repository looking settled, with no test.
#[test]
fn an_odd_score_below_ten_floors_rather_than_truncating() {
    assert_eq!(
        ability_modifier(7),
        -2,
        "the case the truncating version got wrong"
    );
    assert_eq!(ability_modifier(5), -3);
    assert_eq!(ability_modifier(3), -4);
    assert_eq!(ability_modifier(1), -5);
}

#[test]
fn the_modifier_table_matches_the_book_across_its_range() {
    for (score, expected) in [
        (1, -5),
        (2, -4),
        (8, -1),
        (9, -1),
        (10, 0),
        (11, 0),
        (12, 1),
        (16, 3),
        (20, 5),
        (30, 10),
    ] {
        assert_eq!(ability_modifier(score), expected, "score {score}");
    }
}

#[test]
fn the_proficiency_bonus_steps_at_the_levels_the_book_says() {
    for (level, expected) in [
        (1, Some(2)),
        (4, Some(2)),
        (5, Some(3)),
        (12, Some(4)),
        (13, Some(5)),
        (17, Some(6)),
        (20, Some(6)),
    ] {
        assert_eq!(proficiency_bonus(level), expected, "level {level}");
    }
}

/// A level the book does not cover derives nothing rather than defaulting.
#[test]
fn a_level_outside_the_table_yields_no_bonus() {
    assert_eq!(proficiency_bonus(0), None);
    assert_eq!(proficiency_bonus(21), None);
}

#[test]
fn every_ability_gets_a_modifier_from_the_manifests_own_list() {
    let values = rules().derive(&a_character());
    for (ability, expected) in [
        ("strengthMod", 0),
        ("dexterityMod", 3),
        ("constitutionMod", 2),
        ("intelligenceMod", -1),
        ("wisdomMod", 1),
        ("charismaMod", -2),
    ] {
        assert_eq!(value_of(&values, ability), Some(expected), "{ability}");
    }
}

#[test]
fn a_save_adds_proficiency_only_where_the_character_has_it() {
    let values = rules().derive(&a_character());
    // Dexterity: +3 modifier, proficient, +3 bonus at level 5.
    assert_eq!(value_of(&values, "saveDexterity"), Some(6));
    // Constitution: +2 modifier, not proficient.
    assert_eq!(value_of(&values, "saveConstitution"), Some(2));
}

#[test]
fn a_skill_keys_off_the_ability_the_manifest_names() {
    let values = rules().derive(&a_character());
    // Stealth is Dexterity, and proficient: 3 + 3.
    assert_eq!(value_of(&values, "skillStealth"), Some(6));
    // Acrobatics is Dexterity, not proficient.
    assert_eq!(value_of(&values, "skillAcrobatics"), Some(3));
    // Arcana is Intelligence, not proficient: -1.
    assert_eq!(value_of(&values, "skillArcana"), Some(-1));
}

#[test]
fn the_passive_score_is_ten_plus_the_skill() {
    let values = rules().derive(&a_character());
    // Perception is Wisdom (+1), proficient (+3) — so 4, and 14 passive.
    assert_eq!(value_of(&values, "skillPerception"), Some(4));
    assert_eq!(value_of(&values, "passivePerception"), Some(14));
}

/// Nothing that depends on proficiency can be computed without a level, and
/// omitting is the honest answer — a save shown without its bonus is wrong in
/// a way a player cannot see.
#[test]
fn without_a_level_the_modifiers_survive_and_the_rest_does_not() {
    let no_level = DeclaredValues::new([stored("dexterity", 16)]);
    let values = rules().derive(&no_level);

    assert_eq!(value_of(&values, "dexterityMod"), Some(3));
    assert!(value_of(&values, "saveDexterity").is_none());
    assert!(value_of(&values, "proficiencyBonus").is_none());
    assert!(value_of(&values, "skillStealth").is_none());
}

#[test]
fn a_score_nobody_entered_derives_nothing_for_that_ability() {
    let partial = DeclaredValues::new([stored("dexterity", 16), stored(LEVEL, 5)]);
    let values = rules().derive(&partial);

    assert!(value_of(&values, "dexterityMod").is_some());
    assert!(
        value_of(&values, "strengthMod").is_none(),
        "an unfilled sheet is not a character with a strength of nothing"
    );
}

/// The resolver drops anything `derive` returns without declaring, so a
/// mismatch here silently loses a row from every 5e sheet.
#[test]
fn everything_derived_was_declared() {
    let rules = rules();
    let declared: Vec<String> = rules
        .derived_declarations()
        .into_iter()
        .map(|d| d.id)
        .collect();

    for value in rules.derive(&a_character()) {
        assert!(
            declared.contains(&value.id),
            "{} was derived but never declared",
            value.id
        );
    }
}

/// All eighteen, from the manifest rather than from a list here.
#[test]
fn every_skill_the_manifest_declares_is_derived() {
    let rules = rules();
    let values = rules.derive(&a_character());
    assert_eq!(
        rules.skills.len(),
        18,
        "5e declares eighteen skills; if that changed, this rule should be re-read"
    );
    for skill in &rules.skills {
        assert!(
            value_of(&values, &skill_id(&skill.id)).is_some(),
            "{} was not derived",
            skill.id
        );
    }
}

#[test]
fn deriving_twice_from_the_same_input_gives_the_same_answer() {
    let rules = rules();
    let first = rules.derive(&a_character());
    for _ in 0..32 {
        assert_eq!(rules.derive(&a_character()), first);
    }
}
