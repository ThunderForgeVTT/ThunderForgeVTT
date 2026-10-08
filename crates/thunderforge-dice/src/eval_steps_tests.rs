//! Spec 083 FR-002: each value in a die's chain after the first says why it
//! was rolled, a reroll or an explosion. A clamp is not a step: a die was
//! clamped exactly when its final value differs from its chain's last value.

use super::*;
use crate::{ChainStep, DiceFormula};

/// Cycles through fixed `next_u32` values, so a face is `value % sides + 1`.
struct ScriptedRng {
    values: Vec<u32>,
    pos: usize,
}

impl rand_core::TryRng for ScriptedRng {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let v = self.values[self.pos % self.values.len()];
        self.pos += 1;
        Ok(v)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.try_next_u32()? as u64)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for b in dest.iter_mut() {
            *b = self.try_next_u32()? as u8;
        }
        Ok(())
    }
}

fn roll(source: &str, values: Vec<u32>) -> RollResolution {
    let formula = DiceFormula::parse(source).unwrap();
    let mut rng = ScriptedRng { values, pos: 0 };
    resolve(&formula, &PlaceholderBindings::new(), &mut rng).unwrap()
}

#[test]
fn a_reroll_once_is_one_reroll_step() {
    // `r<7` always matches a d6, so the die rerolls exactly once.
    let r = roll("1d6r<7", vec![0, 4]);
    assert_eq!(r.dice[0].rolls, vec![1, 5]);
    assert_eq!(r.dice[0].steps, vec![ChainStep::Reroll]);
}

#[test]
fn a_recursive_reroll_is_only_reroll_steps() {
    let r = roll("1d6rr<3", vec![0, 1, 4]);
    assert_eq!(r.dice[0].rolls, vec![1, 2, 5]);
    assert_eq!(r.dice[0].steps, vec![ChainStep::Reroll, ChainStep::Reroll]);
}

#[test]
fn an_explode_once_is_one_explode_step() {
    let r = roll("1d6xo>0", vec![2, 3]);
    assert_eq!(r.dice[0].rolls, vec![3, 4]);
    assert_eq!(r.dice[0].steps, vec![ChainStep::Explode]);
}

#[test]
fn an_explosion_is_only_explode_steps() {
    let r = roll("1d6x", vec![5, 5, 1]);
    assert_eq!(r.dice[0].rolls, vec![6, 6, 2]);
    assert_eq!(
        r.dice[0].steps,
        vec![ChainStep::Explode, ChainStep::Explode]
    );
}

#[test]
fn a_reroll_then_an_explosion_keeps_both_kinds_in_order() {
    // 1 rerolls to 6, which explodes into 3.
    let r = roll("1d6r1x", vec![0, 5, 2]);
    assert_eq!(r.dice[0].rolls, vec![1, 6, 3]);
    assert_eq!(r.dice[0].steps, vec![ChainStep::Reroll, ChainStep::Explode]);
}

#[test]
fn every_die_has_one_step_per_extra_value() {
    for (formula, values) in [
        ("4d6r1x", vec![0, 5, 5, 2, 3, 0, 1, 4]),
        ("3d10rr<3", vec![0, 1, 9, 4, 2, 0, 7]),
        ("6d6kh3", vec![0, 1, 2, 3, 4, 5]),
    ] {
        let r = roll(formula, values);
        for die in &r.dice {
            assert_eq!(die.steps.len(), die.rolls.len() - 1, "{formula}: {die:?}");
        }
    }
}

#[test]
fn a_clamp_changes_the_final_value_and_adds_no_step() {
    let r = roll("1d20min21", vec![4]);
    let die = &r.dice[0];
    assert_eq!(die.rolls, vec![5]);
    assert_eq!(die.final_value, 21);
    assert_ne!(die.final_value, *die.rolls.last().unwrap());
    assert!(die.steps.is_empty());
}

#[test]
fn a_die_stored_before_steps_reads_as_none() {
    let json = r#"{"sides":{"Numeric":6},"rolls":[1,5],"kept":true,"final_value":5}"#;
    let die: DieOutcome = serde_json::from_str(json).unwrap();
    assert!(die.steps.is_empty());
    assert_eq!(die.rolls, vec![1, 5]);
}

#[test]
fn a_resolution_round_trips_through_json() {
    let r = roll("2d6r1x + 3", vec![0, 5, 2, 3]);
    let json = serde_json::to_string(&r).unwrap();
    let back: RollResolution = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r);
    assert!(
        json.contains("\"steps\":[\"Reroll\",\"Explode\"]"),
        "{json}"
    );
}
