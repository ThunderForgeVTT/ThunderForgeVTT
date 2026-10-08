//! Spec 083 FR-004, FR-005: the readout is the server's arithmetic, in ASCII.

use thunderforge_dice::{ChainStep, DieOutcome, DieSides, ResolutionKind};

use super::super::expand_tests::{outcome, spec};
use super::*;

fn d20(v: i64) -> DieOutcome {
    outcome(DieSides::Numeric(20), &[v], &[])
}

fn labelled(mut s: ThrowSpec, label: &str) -> ThrowSpec {
    s.label = Some(label.to_string());
    s
}

fn bound(mut s: ThrowSpec, name: &str, value: f64) -> ThrowSpec {
    s.bindings.insert(name.to_string(), value);
    s
}

#[test]
fn a_labelled_roll() {
    let s = labelled(
        spec("1d20 + 5", vec![d20(17)], ResolutionKind::Total(22.0)),
        "Stealth",
    );
    assert_eq!(readout(&s), "Ayla: Stealth   17 + 5 = 22");
}

#[test]
fn an_unlabelled_roll() {
    let s = spec("1d20 + 5", vec![d20(17)], ResolutionKind::Total(22.0));
    assert_eq!(readout(&s), "Ayla   17 + 5 = 22");
}

#[test]
fn a_negative_constant_is_a_minus() {
    let s = spec("1d20 - 1", vec![d20(12)], ResolutionKind::Total(11.0));
    assert_eq!(readout(&s), "Ayla   12 - 1 = 11");
    let s = spec("1d20 + -1", vec![d20(12)], ResolutionKind::Total(11.0));
    assert_eq!(readout(&s), "Ayla   12 - 1 = 11");
}

#[test]
fn addends_that_miss_the_total_fall_back_to_the_formula() {
    let s = spec("1d20 + 5", vec![d20(17)], ResolutionKind::Total(30.0));
    assert_eq!(readout(&s), "Ayla   1d20 + 5 = 30");
}

#[test]
fn a_fractional_total_prints_two_decimals() {
    let s = spec(
        "1d6 / 4",
        vec![outcome(DieSides::Numeric(6), &[5], &[])],
        ResolutionKind::Total(1.25),
    );
    assert_eq!(readout(&s), "Ayla   1d6 / 4 = 1.25");
    let s = spec(
        "1d6 / 3",
        vec![outcome(DieSides::Numeric(6), &[5], &[])],
        ResolutionKind::Total(5.0 / 3.0),
    );
    assert_eq!(readout(&s), "Ayla   1d6 / 3 = 1.67");
}

#[test]
fn every_byte_is_ascii() {
    let mut s = labelled(
        spec("1d20 + 5", vec![d20(17)], ResolutionKind::Total(22.0)),
        "Pérception · ✦",
    );
    s.roller = "Zoë".to_string();
    let text = readout(&s);
    assert!(text.is_ascii(), "{text}");
    assert!(text.ends_with("17 + 5 = 22"));
}

#[test]
fn keep_highest_sums_only_the_kept_die() {
    let mut low = d20(4);
    low.kept = false;
    let s = spec(
        "2d20kh1 + 4",
        vec![low, d20(17)],
        ResolutionKind::Total(21.0),
    );
    assert_eq!(readout(&s), "Ayla   17 + 4 = 21");
}

#[test]
fn a_success_count_reads_successes() {
    let d10 = |v| outcome(DieSides::Numeric(10), &[v], &[]);
    let s = spec(
        "6d10cs>=8",
        [9, 3, 8, 10, 1, 7].map(d10).to_vec(),
        ResolutionKind::SuccessCount(3),
    );
    assert_eq!(readout(&s), "Ayla   3 successes");
    let s = spec(
        "2d10cs>=8",
        vec![d10(9), d10(2)],
        ResolutionKind::SuccessCount(1),
    );
    assert_eq!(readout(&s), "Ayla   1 success");
}

#[test]
fn an_exploded_chain_reads_the_crates_final_value() {
    // The crate totals `1d6x` rolling 6, 6, 2 as 2 (research R4); the board
    // agrees with the chat.
    let s = spec(
        "1d6x + 1",
        vec![outcome(
            DieSides::Numeric(6),
            &[6, 6, 2],
            &[ChainStep::Explode, ChainStep::Explode],
        )],
        ResolutionKind::Total(3.0),
    );
    assert_eq!(readout(&s), "Ayla   2 + 1 = 3");
}

#[test]
fn a_placeholder_reads_as_its_bound_value() {
    let s = bound(
        spec(
            "1d20 + MODIFIER",
            vec![d20(13)],
            ResolutionKind::Total(16.0),
        ),
        "MODIFIER",
        3.0,
    );
    assert_eq!(readout(&s), "Ayla   13 + 3 = 16");
}

#[test]
fn a_fallback_substitutes_whole_identifiers() {
    let s = bound(
        bound(
            spec(
                "(1d6 + MOD) * 2 + MODX",
                vec![outcome(DieSides::Numeric(6), &[5], &[])],
                ResolutionKind::Total(14.0),
            ),
            "MOD",
            2.0,
        ),
        "MODY",
        9.0,
    );
    assert_eq!(readout(&s), "Ayla   (1d6 + 2) * 2 + MODX = 14");
}

#[test]
fn a_roll_stored_without_bindings_reads_its_formula() {
    let s = spec(
        "1d20 + MODIFIER",
        vec![d20(13)],
        ResolutionKind::Total(16.0),
    );
    assert_eq!(readout(&s), "Ayla   1d20 + MODIFIER = 16");
}

#[test]
fn the_chip_counts_undrawn_dice() {
    assert_eq!(chip(20, 40).as_deref(), Some("+20 more"));
    assert_eq!(chip(3, 3), None);
}
