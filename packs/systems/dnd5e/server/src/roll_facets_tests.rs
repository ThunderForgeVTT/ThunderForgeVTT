//! Spec 084: how 5e shapes a roll before it is thrown.

use serde_json::{json, Value};
use thunderforge_canvas_core::roll_facets::{Advantage, RollKind, ShapeInput, Shaped};

use super::{shape, NO_D20};

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
