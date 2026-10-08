//! Spec 083 FR-004, FR-005: a resolved roll as the arithmetic a player can
//! follow, or nothing when it is not a plain sum.

use super::*;
use crate::{DieOutcome, DieSides, ResolutionKind, RollResolution};

fn die(sides: u32, value: i64, kept: bool) -> DieOutcome {
    DieOutcome {
        sides: DieSides::Numeric(sides),
        rolls: vec![value],
        steps: vec![],
        kept,
        final_value: value,
    }
}

fn total(formula: &str, dice: Vec<DieOutcome>, total: f64) -> RollResolution {
    RollResolution {
        formula: formula.to_string(),
        dice,
        kind: ResolutionKind::Total(total),
    }
}

fn of(formula: &str, resolution: &RollResolution, bindings: &[(&str, f64)]) -> Option<Breakdown> {
    let bindings: PlaceholderBindings = bindings.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    breakdown(formula, resolution, &bindings)
}

fn d(index: usize, value: i64) -> Addend {
    Addend {
        negative: false,
        kind: AddendKind::Die { index, value },
    }
}

fn c(value: f64, negative: bool) -> Addend {
    Addend {
        negative,
        kind: AddendKind::Constant {
            value,
            placeholder: None,
        },
    }
}

#[test]
fn a_die_plus_a_bonus() {
    let r = total("1d20 + 5", vec![die(20, 17, true)], 22.0);
    assert_eq!(
        of("1d20 + 5", &r, &[]),
        Some(Breakdown::Sum(vec![d(0, 17), c(5.0, false)]))
    );
}

#[test]
fn mixed_dice_keep_the_resolutions_order() {
    let r = total(
        "2d6 + 1d8 + 3",
        vec![die(6, 4, true), die(6, 6, true), die(8, 7, true)],
        20.0,
    );
    assert_eq!(
        of("2d6 + 1d8 + 3", &r, &[]),
        Some(Breakdown::Sum(vec![
            d(0, 4),
            d(1, 6),
            d(2, 7),
            c(3.0, false)
        ]))
    );
}

#[test]
fn a_subtraction_and_an_added_negative_are_both_negative_constants() {
    let r = total("1d20 - 2", vec![die(20, 12, true)], 10.0);
    assert_eq!(
        of("1d20 - 2", &r, &[]),
        Some(Breakdown::Sum(vec![d(0, 12), c(2.0, true)]))
    );
    let r = total("1d20 + -1", vec![die(20, 12, true)], 11.0);
    assert_eq!(
        of("1d20 + -1", &r, &[]),
        Some(Breakdown::Sum(vec![d(0, 12), c(1.0, true)]))
    );
}

#[test]
fn a_negated_die() {
    let r = total("-1d4 + 10", vec![die(4, 3, true)], 7.0);
    assert_eq!(
        of("-1d4 + 10", &r, &[]),
        Some(Breakdown::Sum(vec![
            Addend {
                negative: true,
                kind: AddendKind::Die { index: 0, value: 3 }
            },
            c(10.0, false)
        ]))
    );
}

#[test]
fn a_placeholder_is_its_bound_value_and_keeps_its_name() {
    let r = total("1d20 + MODIFIER", vec![die(20, 13, true)], 16.0);
    assert_eq!(
        of("1d20 + MODIFIER", &r, &[("MODIFIER", 3.0)]),
        Some(Breakdown::Sum(vec![
            d(0, 13),
            Addend {
                negative: false,
                kind: AddendKind::Constant {
                    value: 3.0,
                    placeholder: Some("MODIFIER".to_string())
                }
            }
        ]))
    );
}

#[test]
fn a_negative_binding_is_a_negative_constant() {
    let r = total("1d20 + MODIFIER", vec![die(20, 12, true)], 11.0);
    let Some(Breakdown::Sum(addends)) = of("1d20 + MODIFIER", &r, &[("MODIFIER", -1.0)]) else {
        panic!("a sum");
    };
    assert!(addends[1].negative);
    assert!(matches!(
        addends[1].kind,
        AddendKind::Constant { value, .. } if value == 1.0
    ));
}

#[test]
fn a_missing_binding_is_no_breakdown() {
    let r = total("1d20 + MODIFIER", vec![die(20, 13, true)], 16.0);
    assert_eq!(of("1d20 + MODIFIER", &r, &[]), None);
}

#[test]
fn a_dropped_die_is_not_an_addend() {
    let r = total(
        "2d20kh1 + 4",
        vec![die(20, 4, false), die(20, 17, true)],
        21.0,
    );
    assert_eq!(
        of("2d20kh1 + 4", &r, &[]),
        Some(Breakdown::Sum(vec![d(1, 17), c(4.0, false)]))
    );
}

#[test]
fn a_bound_dice_count_consumes_that_many_dice() {
    let r = total(
        "(X)d6 + 1d4",
        vec![die(6, 2, true), die(6, 5, true), die(4, 1, true)],
        8.0,
    );
    assert_eq!(
        of("(X)d6 + 1d4", &r, &[("X", 2.0)]),
        Some(Breakdown::Sum(vec![d(0, 2), d(1, 5), d(2, 1)]))
    );
}

#[test]
fn products_pools_and_functions_are_no_breakdown() {
    let r = total("(1d6 + 2) * 2", vec![die(6, 5, true)], 14.0);
    assert_eq!(of("(1d6 + 2) * 2", &r, &[]), None);

    let r = total(
        "{1d20, 1d20}kh1 + 4",
        vec![die(20, 3, true), die(20, 15, true)],
        19.0,
    );
    assert_eq!(of("{1d20, 1d20}kh1 + 4", &r, &[]), None);

    let r = total("floor(1d6 / 2)", vec![die(6, 5, true)], 2.0);
    assert_eq!(of("floor(1d6 / 2)", &r, &[]), None);
}

#[test]
fn a_success_count_marks_each_die() {
    let values = [9, 3, 8, 10, 1, 7];
    let r = RollResolution {
        formula: "6d10cs>=8".to_string(),
        dice: values.iter().map(|v| die(10, *v, true)).collect(),
        kind: ResolutionKind::SuccessCount(3),
    };
    assert_eq!(
        of("6d10cs>=8", &r, &[]),
        Some(Breakdown::Successes(vec![
            true, false, true, true, false, false
        ]))
    );
}

#[test]
fn fewer_dice_than_the_formula_names_is_no_breakdown() {
    let r = total("2d6 + 3", vec![die(6, 4, true)], 7.0);
    assert_eq!(of("2d6 + 3", &r, &[]), None);
    let r = total("1d6", vec![die(6, 4, true), die(6, 2, true)], 4.0);
    assert_eq!(of("1d6", &r, &[]), None);
}

#[test]
fn an_unparseable_formula_is_no_breakdown() {
    let r = total("1d20 +", vec![die(20, 4, true)], 4.0);
    assert_eq!(of("1d20 +", &r, &[]), None);
}
