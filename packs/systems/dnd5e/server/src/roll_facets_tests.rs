//! Spec 084: how 5e shapes a roll before it is thrown.

use serde_json::{json, Value};
use thunderforge_canvas_core::roll_facets::{
    Advantage, RerollEdit, RerollInput, RollKind, ShapeInput, Shaped,
};

use super::{reroll, shape, NOT_A_D20_TEST, NO_D20};

fn input<'a>(
    kind: RollKind,
    formula: &'a str,
    sheet: &'a Value,
    advantage: Advantage,
) -> ShapeInput<'a> {
    ShapeInput {
        kind,
        formula,
        trait_data: sheet,
        advantage,
        melee: false,
        item_properties: &[],
    }
}

fn shaped(formula: &str, facets: &[&str]) -> Option<Shaped> {
    Some(Shaped {
        formula: formula.to_string(),
        facets: facets.iter().map(|f| f.to_string()).collect(),
    })
}

#[test]
fn a_normal_d20_test_is_untouched() {
    let sheet = Value::Null;
    for kind in [RollKind::Check, RollKind::ToHit] {
        let roll = input(kind, "1d20 + MODIFIER", &sheet, Advantage::Normal);
        assert_eq!(shape(&roll), Ok(None));
    }
}

#[test]
fn advantage_rolls_two_d20s_and_keeps_the_higher() {
    let sheet = json!({});
    for kind in [RollKind::Check, RollKind::ToHit] {
        let roll = input(kind, "1d20 + MODIFIER", &sheet, Advantage::Advantage);
        assert_eq!(
            shape(&roll),
            Ok(shaped("2d20kh1 + MODIFIER", &["advantage"]))
        );
    }
}

#[test]
fn disadvantage_rolls_two_d20s_and_keeps_the_lower() {
    let sheet = json!({});
    let roll = input(
        RollKind::Check,
        "1d20 + MODIFIER",
        &sheet,
        Advantage::Disadvantage,
    );
    assert_eq!(
        shape(&roll),
        Ok(shaped("2d20kl1 + MODIFIER", &["disadvantage"]))
    );
}

#[test]
fn a_roll_with_no_d20_cannot_be_rolled_twice() {
    let sheet = Value::Null;
    let roll = input(
        RollKind::Check,
        "1d6 + MODIFIER",
        &sheet,
        Advantage::Advantage,
    );
    assert_eq!(shape(&roll), Err(NO_D20.to_string()));
    // Nor can a d20 that already keeps one of several.
    let roll = input(RollKind::Check, "2d20kh1", &sheet, Advantage::Disadvantage);
    assert_eq!(shape(&roll), Err(NO_D20.to_string()));
}

#[test]
fn damage_with_normal_is_untouched() {
    let sheet = json!({});
    let roll = input(
        RollKind::Damage,
        "1d8 + MODIFIER",
        &sheet,
        Advantage::Normal,
    );
    assert_eq!(shape(&roll), Ok(None));
}

#[test]
fn damage_never_takes_advantage() {
    let sheet = json!({});
    let roll = input(RollKind::Damage, "1d20", &sheet, Advantage::Advantage);
    assert!(shape(&roll).is_err());
}

#[test]
fn halfling_luck_rerolls_a_natural_one_on_a_d20_test() {
    let halfling = json!({ "facets": ["halfling_luck"] });
    for kind in [RollKind::Check, RollKind::ToHit] {
        let roll = input(kind, "1d20 + MODIFIER", &halfling, Advantage::Normal);
        assert_eq!(
            shape(&roll),
            Ok(shaped("1d20r1 + MODIFIER", &["halfling_luck"]))
        );
    }
    let roll = input(
        RollKind::Check,
        "1d20 + MODIFIER",
        &halfling,
        Advantage::Advantage,
    );
    assert_eq!(
        shape(&roll),
        Ok(shaped(
            "2d20r1kh1 + MODIFIER",
            &["advantage", "halfling_luck"]
        ))
    );
}

#[test]
fn halfling_luck_never_touches_damage() {
    let halfling = json!({ "facets": ["halfling_luck"] });
    let roll = input(RollKind::Damage, "1d20 + 2", &halfling, Advantage::Normal);
    assert_eq!(shape(&roll), Ok(None));
}

#[test]
fn a_sheet_without_halfling_luck_keeps_its_ones() {
    let other = json!({ "facets": ["lucky"] });
    let roll = input(
        RollKind::Check,
        "1d20 + MODIFIER",
        &other,
        Advantage::Normal,
    );
    assert_eq!(shape(&roll), Ok(None));
}

fn spend<'a>(
    spend: &'a str,
    kind: RollKind,
    sheet: &'a Value,
    settings: &'a Value,
    facets: &'a [String],
) -> RerollInput<'a> {
    RerollInput {
        spend,
        kind,
        formula: "1d20 + MODIFIER",
        facets,
        actor_name: "Pip",
        sheet,
        settings,
    }
}

#[test]
fn heroic_inspiration_rerolls_the_lowest_d20_and_is_spent() {
    let sheet = json!({ "trait_data": { "level": 3, "inspiration": true } });
    let settings = json!({ "inspiration": true });
    for kind in [RollKind::Check, RollKind::ToHit] {
        let plan = reroll(&spend("inspiration", kind, &sheet, &settings, &[])).unwrap();
        assert_eq!(plan.edit, RerollEdit::RerollLowest { sides: 20 });
        assert_eq!(plan.trait_data, json!({ "level": 3, "inspiration": false }));
    }
    // A world that never set the setting uses its default, which is on.
    let plan = reroll(&spend(
        "inspiration",
        RollKind::Check,
        &sheet,
        &json!({}),
        &[],
    ));
    assert!(plan.is_ok());
}

#[test]
fn heroic_inspiration_is_refused_when_it_is_not_there_to_spend() {
    let on = json!({ "inspiration": true });
    let without = json!({ "trait_data": { "inspiration": false } });
    assert_eq!(
        reroll(&spend("inspiration", RollKind::Check, &without, &on, &[])),
        Err("Pip has no Heroic Inspiration.".to_string())
    );
    let blank = json!({});
    assert_eq!(
        reroll(&spend("inspiration", RollKind::Check, &blank, &on, &[])),
        Err("Pip has no Heroic Inspiration.".to_string())
    );

    let inspired = json!({ "trait_data": { "inspiration": true } });
    let off = json!({ "inspiration": false });
    assert_eq!(
        reroll(&spend("inspiration", RollKind::Check, &inspired, &off, &[])),
        Err("This table does not use Heroic Inspiration.".to_string())
    );
    assert_eq!(
        reroll(&spend("inspiration", RollKind::Damage, &inspired, &on, &[])),
        Err("Only a d20 test can be rerolled.".to_string())
    );
}

#[test]
fn a_spend_this_system_does_not_have_is_refused() {
    let sheet = json!({ "trait_data": { "inspiration": true } });
    let settings = json!({});
    assert_eq!(
        reroll(&spend("bardic", RollKind::Check, &sheet, &settings, &[])),
        Err("This system has no reroll called \"bardic\".".to_string())
    );
}

fn lucky_spend<'a>(sheet: &'a Value, formula: &'a str, facets: &'a [String]) -> RerollInput<'a> {
    RerollInput {
        spend: "luck_point",
        kind: RollKind::Check,
        formula,
        facets,
        actor_name: "Pip",
        sheet,
        settings: &Value::Null,
    }
}

fn lucky_sheet(level: Value, used: i64) -> Value {
    json!({ "trait_data": { "level": level, "facets": ["lucky"], "luck_points_used": used } })
}

#[test]
fn a_luck_point_adds_a_d20_keeps_the_highest_and_is_counted() {
    let sheet = lucky_sheet(json!(5), 2);
    let plan = reroll(&lucky_spend(&sheet, "1d20 + MODIFIER", &[])).unwrap();
    assert_eq!(
        plan.edit,
        RerollEdit::Reshape {
            formula: "2d20kh1 + MODIFIER".to_string()
        }
    );
    assert_eq!(
        plan.trait_data,
        json!({ "level": 5, "facets": ["lucky"], "luck_points_used": 3 })
    );

    // A roll already keeping the highest only gains a die.
    let plan = reroll(&lucky_spend(&sheet, "2d20r1kh1 + MODIFIER", &[])).unwrap();
    assert_eq!(
        plan.edit,
        RerollEdit::Reshape {
            formula: "3d20r1kh1 + MODIFIER".to_string()
        }
    );

    // Never used is none used.
    let fresh = json!({ "trait_data": { "level": 1, "facets": ["lucky"] } });
    let plan = reroll(&lucky_spend(&fresh, "1d20 + MODIFIER", &[])).unwrap();
    assert_eq!(plan.trait_data["luck_points_used"], json!(1));
}

#[test]
fn a_luck_point_is_refused_when_there_is_none_or_it_would_buy_nothing() {
    let spent = lucky_sheet(json!(5), 3);
    assert_eq!(
        reroll(&lucky_spend(&spent, "1d20 + MODIFIER", &[])),
        Err("Pip has no Luck Points left.".to_string())
    );
    let unlucky = json!({ "trait_data": { "level": 5, "luck_points_used": 0 } });
    assert_eq!(
        reroll(&lucky_spend(&unlucky, "1d20 + MODIFIER", &[])),
        Err("Pip does not have the Lucky feat.".to_string())
    );
    let sheet = lucky_sheet(json!(5), 0);
    let disadvantage = ["disadvantage".to_string()];
    assert_eq!(
        reroll(&lucky_spend(&sheet, "2d20kl1 + MODIFIER", &disadvantage)),
        Err("A Luck Point does nothing on a roll made with disadvantage.".to_string())
    );
    let mut damage = lucky_spend(&sheet, "1d8 + 2", &[]);
    damage.kind = RollKind::Damage;
    assert_eq!(reroll(&damage), Err(NOT_A_D20_TEST.to_string()));
}

#[test]
fn a_creatures_luck_points_come_from_its_challenge() {
    let creature =
        json!({ "trait_data": { "challenge": "5", "facets": ["lucky"], "luck_points_used": 2 } });
    assert!(reroll(&lucky_spend(&creature, "1d20 + MODIFIER", &[])).is_ok());
    let used_up =
        json!({ "trait_data": { "challenge": "5", "facets": ["lucky"], "luck_points_used": 3 } });
    assert_eq!(
        reroll(&lucky_spend(&used_up, "1d20 + MODIFIER", &[])),
        Err("Pip has no Luck Points left.".to_string())
    );
}
