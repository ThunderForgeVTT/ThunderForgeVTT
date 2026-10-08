//! Spec 083 data-model.md: a resolution's dice, expanded into drawn dice.

use super::*;
use thunderforge_dice::{DieOutcome, ResolutionKind};

pub(crate) fn outcome(sides: DieSides, rolls: &[i64], steps: &[ChainStep]) -> DieOutcome {
    DieOutcome {
        sides,
        rolls: rolls.to_vec(),
        steps: steps.to_vec(),
        kept: true,
        final_value: *rolls.last().unwrap(),
    }
}

pub(crate) fn spec(formula: &str, dice: Vec<DieOutcome>, kind: ResolutionKind) -> ThrowSpec {
    ThrowSpec {
        roll_id: "0d9b7a5c-0000-7000-8000-000000000001".to_string(),
        roller: "Ayla".to_string(),
        label: None,
        bindings: PlaceholderBindings::new(),
        resolution: RollResolution {
            formula: formula.to_string(),
            dice,
            kind,
        },
    }
}

const D6: DieSides = DieSides::Numeric(6);

#[test]
fn a_reroll_is_one_die_with_its_first_value_struck() {
    let s = spec(
        "1d6r1",
        vec![outcome(D6, &[1, 5], &[ChainStep::Reroll])],
        ResolutionKind::Total(5.0),
    );
    let (dice, hidden) = expand(&s);
    assert_eq!(hidden, 0);
    assert_eq!(dice.len(), 1);
    let segs = &dice[0].segments;
    assert_eq!(segs.len(), 2);
    assert!(segs[0].struck);
    assert!(!segs[1].struck);
    assert_eq!(dice[0].face(), 5);
    assert_eq!(dice[0].rerolled(), vec![1]);
}

#[test]
fn an_explosion_is_new_dice_of_the_same_shape() {
    let s = spec(
        "1d6x",
        vec![outcome(
            D6,
            &[6, 6, 2],
            &[ChainStep::Explode, ChainStep::Explode],
        )],
        ResolutionKind::Total(2.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice.len(), 3);
    assert_eq!(dice[0].explosion_of, None);
    assert_eq!(dice[1].explosion_of, Some(0));
    assert_eq!(dice[2].explosion_of, Some(0));
    assert!(dice.iter().all(|d| d.shape == ShapeKind::D6));
    assert_eq!(
        dice.iter().map(ThrowDie::face).collect::<Vec<_>>(),
        vec![6, 6, 2]
    );
}

#[test]
fn a_die_stored_without_steps_reads_as_rerolls() {
    let s = spec(
        "1d6rr<3",
        vec![outcome(D6, &[1, 2, 5], &[])],
        ResolutionKind::Total(5.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice.len(), 1);
    assert_eq!(dice[0].rerolled(), vec![1, 2]);
}

#[test]
fn a_clamp_strikes_the_rolled_value_with_no_tumble() {
    let mut die = outcome(DieSides::Numeric(20), &[5], &[]);
    die.final_value = 21;
    let s = spec("1d20min21", vec![die], ResolutionKind::Total(21.0));
    let (dice, _) = expand(&s);
    let segs = &dice[0].segments;
    assert_eq!(segs.len(), 2);
    assert!(segs[0].struck);
    assert!(!segs[1].tumble);
    assert_eq!(segs[1].starts_ms, segs[0].lands_ms);
    assert_eq!(dice[0].face(), 21);
    assert_eq!(dice[0].clamped(), Some(5));
    assert!(dice[0].rerolled().is_empty());
}

#[test]
fn forty_dice_draw_twenty() {
    let s = spec(
        "40d6",
        (0..40).map(|i| outcome(D6, &[i % 6 + 1], &[])).collect(),
        ResolutionKind::Total(140.0),
    );
    let (dice, hidden) = expand(&s);
    assert_eq!(dice.len(), MAX_DRAWN);
    assert_eq!(hidden, 20);
}

#[test]
fn explosions_count_toward_the_cap() {
    let mut all = vec![outcome(
        D6,
        &[6, 6, 6, 1],
        &[ChainStep::Explode, ChainStep::Explode, ChainStep::Explode],
    )];
    all.extend((0..18).map(|_| outcome(D6, &[3], &[])));
    let s = spec("19d6x", all, ResolutionKind::Total(55.0));
    let (dice, hidden) = expand(&s);
    assert_eq!(dice.len(), MAX_DRAWN);
    assert_eq!(hidden, 2);
}

#[test]
fn mixed_dice_keep_the_resolutions_order() {
    let s = spec(
        "2d6 + 1d8 + 3",
        vec![
            outcome(D6, &[4], &[]),
            outcome(D6, &[6], &[]),
            outcome(DieSides::Numeric(8), &[7], &[]),
        ],
        ResolutionKind::Total(20.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(
        dice.iter().map(|d| d.shape).collect::<Vec<_>>(),
        vec![ShapeKind::D6, ShapeKind::D6, ShapeKind::D8]
    );
    assert_eq!(
        dice.iter().map(|d| d.outcome).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn a_d100_is_one_drawn_die_and_a_d7_is_a_disc() {
    let s = spec(
        "1d100",
        vec![outcome(DieSides::Numeric(100), &[47], &[])],
        ResolutionKind::Total(47.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice.len(), 1);
    assert_eq!(dice[0].shape, ShapeKind::D100);

    let s = spec(
        "1d7",
        vec![outcome(DieSides::Numeric(7), &[7], &[])],
        ResolutionKind::Total(7.0),
    );
    assert_eq!(expand(&s).0[0].shape, ShapeKind::Disc(7));
}

#[test]
fn a_dropped_die_and_a_failed_success_are_marked() {
    let mut low = outcome(DieSides::Numeric(20), &[4], &[]);
    low.kept = false;
    let s = spec(
        "2d20kh1 + 4",
        vec![low, outcome(DieSides::Numeric(20), &[17], &[])],
        ResolutionKind::Total(21.0),
    );
    let (dice, _) = expand(&s);
    assert!(!dice[0].kept);
    assert!(dice[1].kept);
    assert_eq!(dice[0].succeeded, None);

    let s = spec(
        "2d10cs>=8",
        vec![
            outcome(DieSides::Numeric(10), &[9], &[]),
            outcome(DieSides::Numeric(10), &[3], &[]),
        ],
        ResolutionKind::SuccessCount(1),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice[0].succeeded, Some(true));
    assert_eq!(dice[1].succeeded, Some(false));
}

#[test]
fn a_second_segment_lands_one_step_after_the_tumble() {
    let s = spec(
        "1d6r1",
        vec![outcome(D6, &[1, 5], &[ChainStep::Reroll])],
        ResolutionKind::Total(5.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice[0].lands_ms(), TIMINGS.tumble_ms + TIMINGS.step_ms);
}

#[test]
fn an_explosion_enters_one_step_per_earlier_step() {
    let s = spec(
        "1d6r1x",
        vec![outcome(
            D6,
            &[1, 6, 6, 2],
            &[ChainStep::Reroll, ChainStep::Explode, ChainStep::Explode],
        )],
        ResolutionKind::Total(2.0),
    );
    let (dice, _) = expand(&s);
    assert_eq!(dice.len(), 3);
    // The second step is the first explosion: k = 1.
    assert_eq!(dice[1].enters_ms(), TIMINGS.tumble_ms + TIMINGS.step_ms);
    assert_eq!(dice[2].enters_ms(), TIMINGS.tumble_ms + 2 * TIMINGS.step_ms);
    assert_eq!(lands_ms(&dice), TIMINGS.tumble_ms + 3 * TIMINGS.step_ms);
}

#[test]
fn a_clamp_adds_no_time() {
    let mut die = outcome(DieSides::Numeric(20), &[5], &[]);
    die.final_value = 21;
    let s = spec("1d20min21", vec![die], ResolutionKind::Total(21.0));
    let (dice, _) = expand(&s);
    assert_eq!(dice[0].lands_ms(), TIMINGS.tumble_ms);
}
