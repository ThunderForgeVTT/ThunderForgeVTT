//! One test per numbered rule in `data-model.md`, plus the two that are easy
//! to get wrong and the one that cannot happen.

use crate::validators::{validate_resource_data, validate_trait_data};
use serde_json::json;

fn refusal(result: Result<(), crate::validators::ValidationError>) -> String {
    result.expect_err("expected a refusal").to_string()
}

// ============================================================================
// resource_data
// ============================================================================

#[test]
fn r1_resource_data_must_be_an_object() {
    assert_eq!(
        refusal(validate_resource_data(&json!([]))),
        "resource_data: must be a JSON object"
    );
}

#[test]
fn r2_xp_must_be_an_integer() {
    assert_eq!(
        refusal(validate_resource_data(&json!({ "xp": "three" }))),
        "xp: must be an integer"
    );
    assert_eq!(
        refusal(validate_resource_data(&json!({ "xp": 1.5 }))),
        "xp: must be an integer"
    );
}

#[test]
fn r3_xp_must_not_be_negative() {
    assert_eq!(
        refusal(validate_resource_data(&json!({ "xp": -1 }))),
        "xp: must not be negative"
    );
}

#[test]
fn an_absent_balance_is_a_character_who_has_never_failed() {
    assert!(validate_resource_data(&json!({})).is_ok());
    assert!(validate_resource_data(&json!({ "xp": 0 })).is_ok());
    assert!(validate_resource_data(&json!({ "xp": 17 })).is_ok());
}

// ============================================================================
// trait_data
// ============================================================================

fn root() -> serde_json::Value {
    json!({ "id": "s1", "name": "Do Anything", "level": 1, "parentId": null })
}

#[test]
fn t1_trait_data_must_be_an_object() {
    assert_eq!(
        refusal(validate_trait_data(&json!("nope"))),
        "trait_data: must be a JSON object"
    );
}

#[test]
fn t2_description_must_be_text() {
    assert_eq!(
        refusal(validate_trait_data(&json!({ "description": 7 }))),
        "description: must be text"
    );
    assert!(validate_trait_data(&json!({ "description": "Owns one shoe." })).is_ok());
}

#[test]
fn t3_skills_must_be_a_list() {
    assert_eq!(
        refusal(validate_trait_data(&json!({ "skills": {} }))),
        "skills: must be a list"
    );
}

#[test]
fn t4_an_entry_needs_id_name_and_level() {
    assert_eq!(
        refusal(validate_trait_data(&json!({ "skills": ["Do Anything"] }))),
        "skills[0]: must be an object with id, name and level"
    );
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "skills": [{ "id": "s1", "name": "Do Anything" }] })
        )),
        "skills[0]: must be an object with id, name and level"
    );
}

#[test]
fn t5_an_id_is_present_and_unique() {
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "skills": [{ "id": "", "name": "Do Anything", "level": 1 }] })
        )),
        "skills[0].id: must be unique and not empty"
    );

    let duplicated = json!({ "skills": [
        root(),
        { "id": "s1", "name": "Kick A Door Down", "level": 2, "parentId": "s1" },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&duplicated)),
        "skills[1].id: must be unique and not empty"
    );
}

#[test]
fn t6_a_name_must_not_be_empty() {
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "skills": [{ "id": "s1", "name": "   ", "level": 1 }] })
        )),
        "skills[0].name: must not be empty"
    );
}

#[test]
fn t7_a_level_is_a_whole_number_of_at_least_one() {
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "skills": [{ "id": "s1", "name": "Do Anything", "level": 0 }] })
        )),
        "skills[0].level: must be a whole number of at least 1"
    );
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "skills": [{ "id": "s1", "name": "Do Anything", "level": 1.5 }] })
        )),
        "skills[0].level: must be a whole number of at least 1"
    );
}

#[test]
fn t8_a_parent_must_be_another_skill_in_the_list() {
    let stranger = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Kick A Door Down", "level": 2, "parentId": "nobody" },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&stranger)),
        "skills[1].parentId: must name another skill"
    );

    let itself = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Kick A Door Down", "level": 2, "parentId": "s2" },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&itself)),
        "skills[1].parentId: must name another skill"
    );
}

/// The rule that is easy to get wrong: a skill sits *exactly* one level above
/// the one it grew out of, not merely somewhere above it.
#[test]
fn t9_a_level_is_one_higher_than_its_parent() {
    let leapt = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Kick A Door Down", "level": 3, "parentId": "s1" },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&leapt)),
        "skills[1].level: must be one higher than its parent"
    );
}

/// The other one: a character has a single starting skill, however long the
/// lineage grows.
#[test]
fn t10_a_list_holds_exactly_one_starting_skill() {
    let two_roots = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Run Away", "level": 1, "parentId": null },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&two_roots)),
        "skills: must have exactly one starting skill"
    );
}

#[test]
fn t11_the_starting_skill_is_at_level_one() {
    let high_root = json!({ "skills": [
        { "id": "s1", "name": "Do Anything", "level": 2, "parentId": null },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&high_root)),
        "skills: the starting skill must be at level 1"
    );
}

/// A cycle is unreachable rather than forbidden: T9 makes the level strictly
/// increase along every parent link, so a loop would need a level above
/// itself. The validator never walks the lineage, and this is why it does not
/// have to.
#[test]
fn a_cycle_is_refused_by_the_level_relation_alone() {
    let cycle = json!({ "skills": [
        { "id": "s1", "name": "Do Anything", "level": 1, "parentId": "s2" },
        { "id": "s2", "name": "Kick A Door Down", "level": 2, "parentId": "s1" },
    ]});

    // Refused, and refused *before* any traversal — the failure is arithmetic.
    assert_eq!(
        refusal(validate_trait_data(&cycle)),
        "skills[0].level: must be one higher than its parent"
    );
}

#[test]
fn a_lineage_several_levels_deep_is_accepted_in_any_order() {
    let deep = json!({ "skills": [
        { "id": "s3", "name": "Kick A Cellar Door Down", "level": 3, "parentId": "s2" },
        root(),
        { "id": "s2", "name": "Kick A Door Down", "level": 2, "parentId": "s1" },
    ]});
    assert!(validate_trait_data(&deep).is_ok());
}

/// Specificity and relevance are the table's call, and a level has no cap.
#[test]
fn a_name_is_never_judged_and_a_level_is_never_capped() {
    let absurd = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Do Literally Anything At All", "level": 2, "parentId": "s1" },
    ]});
    assert!(validate_trait_data(&absurd).is_ok());

    let towering = json!({ "skills": [
        root(),
        { "id": "s2", "name": "Tall", "level": 2, "parentId": "s1" },
        { "id": "s3", "name": "Taller", "level": 3, "parentId": "s2" },
    ]});
    assert!(validate_trait_data(&towering).is_ok());
}

#[test]
fn an_empty_list_is_a_character_who_has_not_been_opened_yet() {
    assert!(validate_trait_data(&json!({ "skills": [] })).is_ok());
    assert!(validate_trait_data(&json!({})).is_ok());
}
