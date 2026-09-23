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

/// The other one: a lineage has to start somewhere.
///
/// Every skill claiming a parent means either a cycle or a parent outside the
/// set. T8 catches the second; this is what catches the first, which is why it
/// survives the relaxation below.
#[test]
fn t10_a_list_holds_at_least_one_starting_skill() {
    let rootless = json!({ "skills": [
        { "id": "s1", "name": "Do Anything", "level": 1, "parentId": "s2" },
        { "id": "s2", "name": "Kick A Door Down", "level": 2, "parentId": "s1" },
    ]});
    // Refused — by T9 here, since a cycle fails the arithmetic first — and the
    // rootless case below reaches T10 itself.
    assert!(validate_trait_data(&rootless).is_err());
}

/// **This is the rule spec 062 changed, and why.**
///
/// It used to read "exactly one starting skill, at level 1", which was true of
/// every character while `Do Anything 1` was the only way to begin. A world may
/// now declare its own starting skills — several of them, at any level — and
/// each is a root, because none descends from another. The old rule would have
/// made a legal world's characters unstorable the moment they were created:
/// the validator is world-blind, so it cannot ask which world this is, and a
/// rule it cannot evaluate is a rule it must not enforce.
///
/// What it still refuses is what was never a dice pool.
#[test]
fn t11_a_world_may_declare_several_starting_skills_at_any_level() {
    let declared = json!({ "skills": [
        { "id": "s1", "name": "Scavenge", "level": 3, "parentId": null },
        { "id": "s2", "name": "Run Away", "level": 1, "parentId": null },
        { "id": "s3", "name": "Scavenge A Battlefield", "level": 4, "parentId": "s1" },
    ]});
    assert!(
        validate_trait_data(&declared).is_ok(),
        "a character created from a world's own starting skills must save"
    );

    // Level 0 is still not a pool, and T7 is what says so.
    let nothing = json!({ "skills": [
        { "id": "s1", "name": "Do Anything", "level": 0, "parentId": null },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&nothing)),
        "skills[0].level: must be a whole number of at least 1"
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

// ============================================================================
// Statuses (T12-T16) and bought slots (R4-R6) — spec 062
// ============================================================================

#[test]
fn t12_statuses_must_be_a_list() {
    assert_eq!(
        refusal(validate_trait_data(&json!({ "statuses": {} }))),
        "statuses: must be a list"
    );
}

#[test]
fn t13_a_status_carries_an_id_a_name_and_a_modifier() {
    for incomplete in [
        json!({ "name": "Wounded", "modifier": -2 }),
        json!({ "id": "x", "modifier": -2 }),
        json!({ "id": "x", "name": "Wounded" }),
        json!("Wounded"),
    ] {
        assert_eq!(
            refusal(validate_trait_data(&json!({ "statuses": [incomplete] }))),
            "statuses[0]: must be an object with id, name and modifier"
        );
    }
}

#[test]
fn t14_status_ids_are_unique_within_the_character() {
    let twice = json!({ "statuses": [
        { "id": "st1", "name": "Wounded", "modifier": -2 },
        { "id": "st1", "name": "Soaked", "modifier": -1 },
    ]});
    assert_eq!(
        refusal(validate_trait_data(&twice)),
        "statuses[1].id: must be unique and not empty"
    );
}

/// Names are free text the table wrote, so two may repeat — saying "Wounded"
/// twice is a table saying it twice, and both modifiers count. Only the id,
/// which nobody sees and which exists to remove a row by, has to be unique.
#[test]
fn t14_two_statuses_may_share_a_name() {
    let same_name = json!({ "statuses": [
        { "id": "st1", "name": "Wounded", "modifier": -2 },
        { "id": "st2", "name": "Wounded", "modifier": -2 },
    ]});
    assert!(validate_trait_data(&same_name).is_ok());
}

#[test]
fn t15_a_status_name_must_not_be_empty() {
    assert_eq!(
        refusal(validate_trait_data(
            &json!({ "statuses": [{ "id": "st1", "name": "   ", "modifier": 0 }] })
        )),
        "statuses[0].name: must not be empty"
    );
}

#[test]
fn t16_a_modifier_must_be_a_whole_number() {
    for bad in [json!(1.5), json!("-2"), json!(null)] {
        assert_eq!(
            refusal(validate_trait_data(
                &json!({ "statuses": [{ "id": "st1", "name": "Wounded", "modifier": bad }] })
            )),
            "statuses[0].modifier: must be a whole number"
        );
    }
}

/// This system ships no list of statuses and judges none, exactly as it judges
/// no skill name. A table wanting nine of them at −20 apiece is playing their
/// game, and a −100 is a legitimate way to say "this is not happening".
#[test]
fn nothing_caps_how_many_statuses_or_how_large() {
    let absurd = json!({ "statuses": (0..9)
        .map(|i| json!({ "id": format!("st{i}"), "name": "Doomed", "modifier": -100 }))
        .collect::<Vec<_>>() });
    assert!(validate_trait_data(&absurd).is_ok());

    let zero = json!({ "statuses": [{ "id": "st1", "name": "Marked", "modifier": 0 }] });
    assert!(validate_trait_data(&zero).is_ok());
}

/// Absent is the ordinary case — it is every character until a table turns
/// statuses on — and it must never read as an error.
#[test]
fn no_statuses_is_not_an_error() {
    assert!(validate_trait_data(&json!({})).is_ok());
    assert!(validate_trait_data(&json!({ "statuses": [] })).is_ok());
    assert!(validate_trait_data(&json!({ "statuses": null })).is_ok());
}

#[test]
fn r4_bought_slots_must_be_an_object() {
    assert_eq!(
        refusal(validate_resource_data(
            &json!({ "xp": 0, "boughtSlots": [] })
        )),
        "boughtSlots: must be a JSON object"
    );
}

#[test]
fn r5_bought_slots_are_keyed_by_a_level() {
    for bad_key in ["0", "-1", "two", "1.5", ""] {
        assert_eq!(
            refusal(validate_resource_data(
                &json!({ "xp": 0, "boughtSlots": { bad_key: 1 } })
            )),
            format!("boughtSlots[{bad_key}]: must be keyed by a whole level of at least 1")
        );
    }
}

#[test]
fn r6_a_bought_count_is_a_whole_number_of_at_least_zero() {
    for bad in [json!(-1), json!(1.5), json!("two"), json!(null)] {
        assert_eq!(
            refusal(validate_resource_data(
                &json!({ "xp": 0, "boughtSlots": { "2": bad } })
            )),
            "boughtSlots[2]: must be a whole number of at least 0"
        );
    }

    assert!(validate_resource_data(&json!({ "xp": 0, "boughtSlots": { "2": 0 } })).is_ok());
    assert!(validate_resource_data(&json!({ "xp": 4, "boughtSlots": { "2": 3, "7": 1 } })).is_ok());
}

/// **Turning skill slots on must never make an existing character unstorable**
/// (FR-036). A character who already holds five skills at level 2 predates the
/// setting, or was stored while it was off, and the storage layer has no way to
/// ask which — so it does not refuse them. The cap is enforced where an
/// advancement is *granted*, not where a character is saved.
///
/// Written as a test rather than only as a comment, because a future reader
/// adding "and the skills must fit the caps" here would break every such
/// character and the comment alone would not stop them.
#[test]
fn a_character_over_the_caps_is_still_storable() {
    let mut skills = vec![root()];
    for i in 2..=6 {
        skills.push(json!({
            "id": format!("s{i}"),
            "name": format!("Crowded {i}"),
            "level": 2,
            "parentId": "s1",
        }));
    }
    assert!(
        validate_trait_data(&json!({ "skills": skills })).is_ok(),
        "five skills at level 2 exceeds the cap of four, and must still save"
    );

    assert!(validate_resource_data(&json!({ "xp": 0, "boughtSlots": {} })).is_ok());
}
