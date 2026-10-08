//! Spec 084 T006: the replay guarantees of contracts/pack-roll-facets.md.

use super::*;
use crate::{ResolutionKind, resolve};

/// Answers a fixed list of draws, then panics: a test that must draw
/// nothing more says so by running out.
struct Draws {
    values: Vec<u32>,
    pos: usize,
}

impl Draws {
    fn new(values: Vec<u32>) -> Self {
        Draws { values, pos: 0 }
    }
}

impl rand_core::TryRng for Draws {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let v = *self
            .values
            .get(self.pos)
            .expect("the replay drew a die it should not have");
        self.pos += 1;
        Ok(v)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(u64::from(self.try_next_u32()?))
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for b in dest.iter_mut() {
            *b = self.try_next_u32()? as u8;
        }
        Ok(())
    }
}

/// A die face `n` on a die of any size, as a draw (`1 + v % sides`).
fn face(n: u32) -> u32 {
    n - 1
}

fn roll(formula: &str, draws: Vec<u32>) -> RollResolution {
    let parsed = DiceFormula::parse(formula).unwrap();
    resolve(&parsed, &bindings(), &mut Draws::new(draws)).unwrap()
}

fn bindings() -> PlaceholderBindings {
    PlaceholderBindings::from([("MODIFIER".to_string(), 5.0), ("STAT".to_string(), 2.0)])
}

fn replay_of(
    formula: &str,
    resolution: &RollResolution,
    reshaped: Option<&str>,
    edit: ReplayEdit,
    draws: Vec<u32>,
) -> Result<RollResolution, FormulaError> {
    let b = bindings();
    replay(
        Recorded {
            formula,
            bindings: &b,
            resolution,
        },
        reshaped,
        edit,
        &mut Draws::new(draws),
    )
}

fn total(resolution: &RollResolution) -> f64 {
    match resolution.kind {
        ResolutionKind::Total(t) => t,
        ResolutionKind::SuccessCount(n) => n as f64,
    }
}

#[test]
fn an_identity_replay_draws_nothing_and_changes_nothing() {
    // A small deterministic generator, so each formula sees varied faces.
    let mut state = 0x2545_f491_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let corpus = [
        "1d20 + MODIFIER",
        "4d6kh3",
        "4d6kl2",
        "4d6dh1",
        "4d6dl1 + STAT",
        "2d20kh1 + MODIFIER",
        "2d6r1",
        "2d6r<3",
        "2d6rr1",
        "1d6x",
        "1d6x>=5",
        "1d6xo",
        "2d6min3 + 1d8min3 + 3",
        "2d6max4",
        "8d10cs>=7",
        "8d10cs>=7cf1",
        "6d6sf<=2",
        "6d6eo",
        "3d6ms10",
        "4dF",
        "2dc",
        "{2d6, 1d8}kh1",
        "(1d4)d6",
        "2d20r1kh1",
    ];
    for formula in corpus {
        for _ in 0..20 {
            let draws: Vec<u32> = (0..400).map(|_| next()).collect();
            let original = roll(formula, draws);
            let again = replay_of(formula, &original, None, ReplayEdit::None, Vec::new());
            assert_eq!(again.as_ref(), Ok(&original), "{formula}");
        }
    }
}

#[test]
fn rerolling_a_die_under_advantage_can_move_the_keep() {
    // 2d20kh1: a 15 kept, a 4 dropped.
    let original = roll("2d20kh1 + MODIFIER", vec![face(15), face(4)]);
    assert_eq!(total(&original), 20.0);
    let out = replay_of(
        "2d20kh1 + MODIFIER",
        &original,
        None,
        ReplayEdit::RerollDie(1),
        vec![face(18)],
    )
    .unwrap();
    assert_eq!(out.dice[0].rolls, vec![15]);
    assert!(!out.dice[0].kept);
    assert_eq!(out.dice[1].rolls, vec![4, 18]);
    assert_eq!(out.dice[1].steps, vec![ChainStep::Reroll]);
    assert_eq!(out.dice[1].final_value, 18);
    assert!(out.dice[1].kept);
    assert_eq!(total(&out), 23.0);
}

#[test]
fn rerolling_a_die_under_a_clamp_clamps_the_new_face() {
    let original = roll("2d6min3", vec![face(5), face(4)]);
    let out = replay_of(
        "2d6min3",
        &original,
        None,
        ReplayEdit::RerollDie(0),
        vec![face(1)],
    )
    .unwrap();
    assert_eq!(out.dice[0].rolls, vec![5, 1]);
    assert_eq!(out.dice[0].final_value, 3);
    assert_eq!(out.dice[1].rolls, vec![4]);
    assert_eq!(total(&out), 7.0);
}

#[test]
fn the_new_face_is_used_even_when_the_term_would_reroll_it() {
    // r1: the first 1 was rerolled into a 3; Inspiration then rolls a 1.
    let original = roll("1d20r1", vec![face(1), face(3)]);
    assert_eq!(original.dice[0].rolls, vec![1, 3]);
    let out = replay_of(
        "1d20r1",
        &original,
        None,
        ReplayEdit::RerollDie(0),
        vec![face(1)],
    )
    .unwrap();
    assert_eq!(out.dice[0].rolls, vec![1, 3, 1]);
    assert_eq!(
        out.dice[0].steps,
        vec![ChainStep::Reroll, ChainStep::Reroll]
    );
    assert_eq!(out.dice[0].final_value, 1);
}

#[test]
fn a_reshape_keeps_the_recorded_die_and_draws_the_new_one() {
    let original = roll("1d20 + 5", vec![face(7)]);
    let out = replay_of(
        "1d20 + 5",
        &original,
        Some("2d20kh1 + 5"),
        ReplayEdit::None,
        vec![face(16)],
    )
    .unwrap();
    assert_eq!(out.formula, "2d20kh1 + 5");
    assert_eq!(out.dice[0].rolls, vec![7]);
    assert!(!out.dice[0].kept);
    assert_eq!(out.dice[1].rolls, vec![16]);
    assert_eq!(total(&out), 21.0);
}

#[test]
fn a_fresh_die_in_a_reshape_takes_the_term_modifiers() {
    let original = roll("2d20r1kh1", vec![face(9), face(12)]);
    let out = replay_of(
        "2d20r1kh1",
        &original,
        Some("3d20r1kh1"),
        ReplayEdit::None,
        vec![face(1), face(19)],
    )
    .unwrap();
    assert_eq!(out.dice[0].rolls, vec![9]);
    assert_eq!(out.dice[1].rolls, vec![12]);
    assert_eq!(out.dice[2].rolls, vec![1, 19]);
    assert_eq!(total(&out), 19.0);
}

#[test]
fn a_reshape_that_does_not_line_up_is_refused() {
    let original = roll("2d20kh1 + 1d6", vec![face(9), face(12), face(3)]);
    for reshaped in [
        "1d20 + 1d6",
        "2d20kh1",
        "2d20kh1 + 1d6 + 1d4",
        "2d20kh1 + 1d8",
    ] {
        let out = replay_of(
            "2d20kh1 + 1d6",
            &original,
            Some(reshaped),
            ReplayEdit::None,
            vec![face(1)],
        );
        assert!(
            matches!(out, Err(FormulaError::ReplayMismatch(_))),
            "{reshaped}: {out:?}"
        );
    }
}

#[test]
fn a_recording_that_does_not_fit_its_formula_is_refused() {
    let original = roll("2d20kh1", vec![face(9), face(12)]);
    let out = replay_of("1d20", &original, None, ReplayEdit::None, Vec::new());
    assert!(matches!(out, Err(FormulaError::ReplayMismatch(_))));
    let out = replay_of("3d20", &original, None, ReplayEdit::None, Vec::new());
    assert!(matches!(out, Err(FormulaError::ReplayMismatch(_))));
    let out = replay_of(
        "2d20kh1",
        &original,
        None,
        ReplayEdit::RerollDie(2),
        Vec::new(),
    );
    assert!(matches!(out, Err(FormulaError::ReplayMismatch(_))));
}

#[test]
fn the_lowest_d20_is_the_first_of_a_tie_and_ignores_other_dice() {
    let r = roll(
        "2d20 + 1d6 + 1d20",
        vec![face(6), face(11), face(1), face(6)],
    );
    assert_eq!(lowest_die(&r, 20), Some(0));
    assert_eq!(lowest_die(&r, 6), Some(2));
    assert_eq!(lowest_die(&r, 8), None);
}

#[test]
fn a_chain_stored_before_steps_existed_gets_no_steps() {
    let mut original = roll("1d20r1", vec![face(1), face(3)]);
    original.dice[0].steps.clear();
    let out = replay_of(
        "1d20r1",
        &original,
        None,
        ReplayEdit::RerollDie(0),
        vec![face(8)],
    )
    .unwrap();
    assert_eq!(out.dice[0].rolls, vec![1, 3, 8]);
    assert!(out.dice[0].steps.is_empty());
}
