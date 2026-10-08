//! Spec 084 T004: `rewrite_dice_terms` and the canonical printer.

use super::*;
use crate::parser::parse;

fn first_d20(edit: TermEdit) -> impl FnMut(&TermView) -> Option<TermEdit> {
    let mut edit = Some(edit);
    move |term: &TermView| {
        if term.sides == Some(20) {
            edit.take()
        } else {
            None
        }
    }
}

#[test]
fn no_edit_gives_the_formula_back_byte_for_byte() {
    for formula in [
        "1d20+MODIFIER",
        "  d20 +  STR ",
        "{2d6, 1d8}kh1",
        "(2d4)d8 * 2",
    ] {
        assert_eq!(rewrite_dice_terms(formula, |_| None).unwrap(), formula);
    }
}

#[test]
fn advantage_and_disadvantage_double_the_d20() {
    let advantage = rewrite_dice_terms(
        "1d20 + MODIFIER",
        first_d20(TermEdit {
            count: Some(2),
            add: vec![AddModifier::KeepHighest(1)],
        }),
    );
    assert_eq!(advantage.unwrap(), "2d20kh1 + MODIFIER");
    let disadvantage = rewrite_dice_terms(
        "1d20 + MODIFIER",
        first_d20(TermEdit {
            count: Some(2),
            add: vec![AddModifier::KeepLowest(1)],
        }),
    );
    assert_eq!(disadvantage.unwrap(), "2d20kl1 + MODIFIER");
}

#[test]
fn rerolls_print_before_keeps() {
    let luck = rewrite_dice_terms(
        "1d20+MODIFIER",
        first_d20(TermEdit {
            count: None,
            add: vec![AddModifier::RerollOnceEq(1)],
        }),
    )
    .unwrap();
    assert_eq!(luck, "1d20r1 + MODIFIER");
    assert!(parse(&luck).is_ok());

    let both = rewrite_dice_terms(
        "1d20 + 3",
        first_d20(TermEdit {
            count: Some(2),
            add: vec![AddModifier::KeepHighest(1), AddModifier::RerollOnceEq(1)],
        }),
    )
    .unwrap();
    assert_eq!(both, "2d20r1kh1 + 3");
    assert!(parse(&both).is_ok());
}

#[test]
fn every_term_can_take_a_clamp() {
    let views = std::cell::RefCell::new(Vec::new());
    let out = rewrite_dice_terms("2d6 + 1d8 + 3", |term| {
        views
            .borrow_mut()
            .push((term.index, term.count, term.sides));
        Some(TermEdit {
            count: None,
            add: vec![AddModifier::Min(3)],
        })
    })
    .unwrap();
    assert_eq!(out, "2d6min3 + 1d8min3 + 3");
    assert_eq!(
        views.into_inner(),
        vec![(0, Some(2), Some(6)), (1, Some(1), Some(8))]
    );
}

#[test]
fn only_the_first_d20_is_touched() {
    let out = rewrite_dice_terms(
        "1d20 + 1d20",
        first_d20(TermEdit {
            count: Some(2),
            add: vec![AddModifier::KeepHighest(1)],
        }),
    )
    .unwrap();
    assert_eq!(out, "2d20kh1 + 1d20");
}

#[test]
fn the_view_reports_what_a_term_already_has() {
    let mut seen = Vec::new();
    rewrite_dice_terms("2d20kh1 + 1d20r1 + 2d6min2 + 4dF", |term| {
        seen.push((term.keeps, term.rerolls, term.clamps, term.sides));
        None
    })
    .unwrap();
    assert_eq!(
        seen,
        vec![
            (true, false, false, Some(20)),
            (false, true, false, Some(20)),
            (false, false, true, Some(6)),
            (false, false, false, None),
        ]
    );
}

#[test]
fn print_then_parse_gives_the_same_tree() {
    let corpus = [
        "4d6kh3",
        "4d6kl2",
        "4d6dh1",
        "4d6dl1",
        "2d6r1",
        "2d6r<3",
        "2d6rr1",
        "1d6x",
        "1d6x>=5",
        "1d6xo",
        "1d6xo=6",
        "2d6min2",
        "2d6max5",
        "8d10cs>=7",
        "8d10cs>=7cf1",
        "6d6sf<=2",
        "6d6eo",
        "6d6od",
        "3d6ms10",
        "1d6x kh1",
        "4dF",
        "2dc",
        "4dF kh2",
        "(2d4)d8",
        "1d(1d20)",
        "{2d6, 1d8, 3}kh1",
        "-(1d4 + 2) * 3 / 2",
        "1d20 - (2 - 1)",
        "floor(1d6 / 2) + ceil(1.5) + round(2.5) + abs(-3)",
        "1d20 + STR + MODIFIER",
        "2d20r1kh1min2",
    ];
    for formula in corpus {
        let ast = parse(formula).unwrap();
        let printed = print_expr(&ast);
        assert_eq!(
            parse(&printed).unwrap(),
            ast,
            "{formula} printed as {printed}"
        );
    }
}
