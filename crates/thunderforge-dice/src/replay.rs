//! Spec 084 (research R3): roll a recorded roll again, keeping its dice.
//!
//! A reroll changes one die, or adds dice to a term, and leaves every other
//! die where it fell. So the recorded dice are fed back through the
//! evaluator term by term, in place of fresh draws: keep, drop, clamps and
//! counting are applied again by the same code that applied them the first
//! time. A recorded die keeps its whole chain, rerolls and explosions
//! included. Only a die the recording does not have is drawn from the rng,
//! and that die takes the term's own modifiers.

use std::collections::VecDeque;

use rand_core::Rng;

use crate::eval::{PlaceholderBindings, resolve_with};
use crate::{ChainStep, DiceFormula, DieOutcome, DieSides, FormulaError, RollResolution};

/// A roll as it was stored.
pub struct Recorded<'a> {
    pub formula: &'a str,
    pub bindings: &'a PlaceholderBindings,
    pub resolution: &'a RollResolution,
}

/// What to change while replaying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayEdit {
    None,
    /// Roll die `i` (an index into the resolution's dice) once more and use
    /// the new face, whatever it is (Heroic Inspiration).
    RerollDie(usize),
}

/// The die with the lowest final value among the `d{sides}`, the first of
/// them on a tie.
pub fn lowest_die(resolution: &RollResolution, sides: u32) -> Option<usize> {
    resolution
        .dice
        .iter()
        .enumerate()
        .filter(|(_, die)| die.sides == DieSides::Numeric(sides))
        .min_by_key(|(i, die)| (die.final_value, *i))
        .map(|(i, _)| i)
}

/// Replay `original` through `reshaped` (or its own formula), applying `edit`.
///
/// `reshaped` must have the same dice terms in the same order, each with at
/// least as many dice as recorded. Anything else is a `ReplayMismatch`.
pub fn replay<R: Rng>(
    original: Recorded<'_>,
    reshaped: Option<&str>,
    edit: ReplayEdit,
    rng: &mut R,
) -> Result<RollResolution, FormulaError> {
    let source = DiceFormula::parse(original.formula)?;
    // Pass one: find how many recorded dice each term rolled. Nothing is
    // drawn, because every die comes from the recording or the pass fails.
    let mut flat = Flat {
        dice: original
            .resolution
            .dice
            .iter()
            .cloned()
            .map(fresh)
            .collect(),
        sizes: Vec::new(),
        left_in_term: 0,
    };
    resolve_with(&source, original.bindings, rng, Some(&mut flat))?;
    if !flat.dice.is_empty() {
        return Err(mismatch("more dice were recorded than the formula rolls"));
    }

    let mut dice: Vec<DieOutcome> = original
        .resolution
        .dice
        .iter()
        .cloned()
        .map(fresh)
        .collect();
    if let ReplayEdit::RerollDie(i) = edit {
        let die = dice
            .get_mut(i)
            .ok_or_else(|| mismatch("there is no such die to reroll"))?;
        reroll(die, rng);
    }

    let mut groups = Vec::with_capacity(flat.sizes.len());
    let mut rest = dice.into_iter();
    for size in &flat.sizes {
        groups.push(rest.by_ref().take(*size as usize).collect::<VecDeque<_>>());
    }
    let mut grouped = Grouped {
        groups,
        next: 0,
        current: None,
    };
    let target = match reshaped {
        Some(formula) => DiceFormula::parse(formula)?,
        None => source,
    };
    let resolution = resolve_with(&target, original.bindings, rng, Some(&mut grouped))?;
    if grouped.next != grouped.groups.len() {
        return Err(mismatch(
            "the formula has fewer dice terms than were recorded",
        ));
    }
    Ok(resolution)
}

/// Where the evaluator takes a term's dice from, before drawing fresh ones.
pub(crate) trait DrawSource {
    /// A dice term is about to roll `count` dice of `sides`.
    fn begin_term(&mut self, count: u32, sides: DieSides) -> Result<(), FormulaError>;
    /// The next recorded die of the current term, or `None` to draw one.
    fn next_die(&mut self) -> Option<DieOutcome>;
}

fn mismatch(why: &str) -> FormulaError {
    FormulaError::ReplayMismatch(why.to_string())
}

/// A recorded die as it came off the table: kept, and showing its last face.
fn fresh(mut die: DieOutcome) -> DieOutcome {
    die.kept = true;
    die.final_value = *die.rolls.last().unwrap_or(&die.final_value);
    die
}

/// One more face for `die`, which it then shows. The term's reroll and
/// explode modifiers do not apply to it: the rule says use the new roll.
fn reroll<R: Rng>(die: &mut DieOutcome, rng: &mut R) {
    // The same mapping as the evaluator's own draw.
    let face = match die.sides {
        DieSides::Numeric(n) => 1 + (rng.next_u32() % n.max(1)) as i64,
        DieSides::Fate => (rng.next_u32() % 3) as i64 - 1,
        DieSides::Coin => (rng.next_u32() % 2) as i64,
    };
    // A chain stored before spec 083 has no steps; leave it without.
    if die.steps.len() + 1 == die.rolls.len() {
        die.steps.push(ChainStep::Reroll);
    }
    die.rolls.push(face);
    die.final_value = face;
}

/// Pass one: the recorded dice in one line, taken a term at a time.
struct Flat {
    dice: VecDeque<DieOutcome>,
    sizes: Vec<u32>,
    left_in_term: u32,
}

impl DrawSource for Flat {
    fn begin_term(&mut self, count: u32, sides: DieSides) -> Result<(), FormulaError> {
        if self.dice.len() < count as usize {
            return Err(mismatch("fewer dice were recorded than the formula rolls"));
        }
        if self
            .dice
            .iter()
            .take(count as usize)
            .any(|d| d.sides != sides)
        {
            return Err(mismatch("a recorded die has the wrong number of sides"));
        }
        self.sizes.push(count);
        self.left_in_term = count;
        Ok(())
    }

    fn next_die(&mut self) -> Option<DieOutcome> {
        if self.left_in_term == 0 {
            return None;
        }
        self.left_in_term -= 1;
        self.dice.pop_front()
    }
}

/// Pass two: each term's recorded dice, then fresh ones if it rolls more.
struct Grouped {
    groups: Vec<VecDeque<DieOutcome>>,
    next: usize,
    current: Option<usize>,
}

impl DrawSource for Grouped {
    fn begin_term(&mut self, count: u32, sides: DieSides) -> Result<(), FormulaError> {
        let group = self
            .groups
            .get(self.next)
            .ok_or_else(|| mismatch("the formula has more dice terms than were recorded"))?;
        if (count as usize) < group.len() {
            return Err(mismatch("a term rolls fewer dice than were recorded"));
        }
        if group.iter().any(|d| d.sides != sides) {
            return Err(mismatch("a term's die has changed its sides"));
        }
        self.current = Some(self.next);
        self.next += 1;
        Ok(())
    }

    fn next_die(&mut self) -> Option<DieOutcome> {
        self.groups[self.current?].pop_front()
    }
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
