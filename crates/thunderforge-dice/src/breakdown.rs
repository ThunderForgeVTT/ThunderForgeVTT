//! Spec 083 FR-004, FR-005: a resolved roll as arithmetic a player can
//! follow.
//!
//! `breakdown` re-reads the formula and pairs its dice terms with the
//! resolution's dice, in the order `resolve` rolled them. When the formula
//! is a sum or difference of dice terms and constants, the answer is the
//! kept dice and the constants as signed addends. When it is a single
//! success-counting term, the answer is which dice succeeded. Anything else,
//! a product, a pool, a function, a nested die, is nothing: the caller shows
//! `formula = total` rather than arithmetic that would not add up.
//!
//! It is pure and derived (AGENTS.md section 5): the server sends no readout,
//! and whoever shows one computes it here.

use crate::ast::{BinOp, Condition, DiceTerm, Expr, Modifier, Sides};
use crate::eval::condition_matches;
use crate::{DiceFormula, DieSides, PlaceholderBindings, ResolutionKind, RollResolution};

/// A resolved roll, explained.
#[derive(Debug, Clone, PartialEq)]
pub enum Breakdown {
    /// A sum or difference of dice terms and constants.
    Sum(Vec<Addend>),
    /// A success-count roll: for each die in resolution order, whether it
    /// succeeded.
    Successes(Vec<bool>),
}

/// One term of a sum. `negative` is its sign as it reaches the total, so a
/// bonus of `-1` is `negative: true` with a value of `1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Addend {
    pub negative: bool,
    pub kind: AddendKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddendKind {
    /// A kept die, by its index in `RollResolution::dice`.
    Die { index: usize, value: i64 },
    /// A literal, or a substituted placeholder (with its name). Never
    /// negative: the sign is the addend's.
    Constant {
        value: f64,
        placeholder: Option<String>,
    },
}

/// Explains `resolution`, the result of resolving `formula` with `bindings`.
/// `None` when the formula is not a plain sum or a single success count, or
/// when the resolution does not match the formula.
pub fn breakdown(
    formula: &str,
    resolution: &RollResolution,
    bindings: &PlaceholderBindings,
) -> Option<Breakdown> {
    let parsed = DiceFormula::parse(formula).ok()?;
    let mut walk = Walk {
        resolution,
        bindings,
        cursor: 0,
        addends: Vec::new(),
    };

    if let ResolutionKind::SuccessCount(_) = resolution.kind {
        let Expr::Dice(term) = &parsed.ast else {
            return None;
        };
        let marks = walk.successes(term)?;
        return (walk.cursor == resolution.dice.len()).then_some(Breakdown::Successes(marks));
    }

    walk.sum(&parsed.ast, false)?;
    (walk.cursor == resolution.dice.len()).then_some(Breakdown::Sum(walk.addends))
}

struct Walk<'a> {
    resolution: &'a RollResolution,
    bindings: &'a PlaceholderBindings,
    /// The next die of the resolution a dice term will claim.
    cursor: usize,
    addends: Vec<Addend>,
}

impl Walk<'_> {
    fn sum(&mut self, expr: &Expr, negative: bool) -> Option<()> {
        match expr {
            Expr::Number(n) => self.constant(*n, None, negative),
            Expr::Placeholder(name) => {
                let value = *self.bindings.get(name)?;
                self.constant(value, Some(name.clone()), negative)
            }
            Expr::Neg(inner) => self.sum(inner, !negative),
            Expr::BinOp(lhs, op, rhs) => match op {
                BinOp::Add => {
                    self.sum(lhs, negative)?;
                    self.sum(rhs, negative)
                }
                BinOp::Sub => {
                    self.sum(lhs, negative)?;
                    self.sum(rhs, !negative)
                }
                BinOp::Mul | BinOp::Div => None,
            },
            Expr::Dice(term) => {
                if term.modifiers.iter().any(counts_successes) {
                    return None;
                }
                let start = self.claim(term)?;
                for index in start..self.cursor {
                    let die = &self.resolution.dice[index];
                    if die.kept {
                        self.addends.push(Addend {
                            negative,
                            kind: AddendKind::Die {
                                index,
                                value: die.final_value,
                            },
                        });
                    }
                }
                Some(())
            }
            Expr::MathFn(..) | Expr::Pool(..) => None,
        }
    }

    fn constant(&mut self, value: f64, placeholder: Option<String>, negative: bool) -> Option<()> {
        if !value.is_finite() {
            return None;
        }
        self.addends.push(Addend {
            negative: negative != (value < 0.0),
            kind: AddendKind::Constant {
                value: value.abs(),
                placeholder,
            },
        });
        Some(())
    }

    /// A single term counting successes (`cs`) and failures (`cf`): whether
    /// each of its dice counted for the roll. Other pool readings (margin,
    /// even, odd, subtracted failures) have no per-die mark and give `None`.
    fn successes(&mut self, term: &DiceTerm) -> Option<Vec<bool>> {
        let mut success = None;
        let mut failure = None;
        for modifier in &term.modifiers {
            match modifier {
                Modifier::CountSuccesses(c) => success = Some(*c),
                Modifier::CountFailures(c) => failure = Some(*c),
                m if counts_successes(m) => return None,
                _ => {}
            }
        }
        if success.is_none() && failure.is_none() {
            return None;
        }
        let start = self.claim(term)?;
        let marks = self.resolution.dice[start..self.cursor]
            .iter()
            .map(|die| {
                let max_face = max_face(die.sides);
                let hit = |c: Option<Condition>| {
                    c.is_some_and(|c| condition_matches(c, die.final_value, max_face))
                };
                die.kept
                    && (success.is_some() && hit(success) || success.is_none() && !hit(failure))
            })
            .collect();
        Some(marks)
    }

    /// Claims the term's dice from the resolution, checking their count and
    /// sides. Answers the index of the first.
    fn claim(&mut self, term: &DiceTerm) -> Option<usize> {
        let count = self.whole(&term.count)?;
        let sides = match &term.sides {
            Sides::Fate => DieSides::Fate,
            Sides::Coin => DieSides::Coin,
            Sides::Numeric(expr) => DieSides::Numeric(u32::try_from(self.whole(expr)?).ok()?),
        };
        let count = usize::try_from(count).ok()?;
        let start = self.cursor;
        let end = start.checked_add(count)?;
        let dice = self.resolution.dice.get(start..end)?;
        if dice.iter().any(|die| die.sides != sides) {
            return None;
        }
        self.cursor = end;
        Some(start)
    }

    /// A dice count or size: a number or a bound placeholder. A nested die
    /// would put its own dice in front of this term's, so it is refused.
    fn whole(&self, expr: &Expr) -> Option<i64> {
        let value = match expr {
            Expr::Number(n) => *n,
            Expr::Placeholder(name) => *self.bindings.get(name)?,
            _ => return None,
        };
        value.is_finite().then(|| value.round() as i64)
    }
}

fn counts_successes(modifier: &Modifier) -> bool {
    matches!(
        modifier,
        Modifier::CountSuccesses(_)
            | Modifier::CountFailures(_)
            | Modifier::SubtractFailureValue(_)
            | Modifier::Even
            | Modifier::Odd
            | Modifier::MarginOfSuccess(_)
    )
}

fn max_face(sides: DieSides) -> i64 {
    match sides {
        DieSides::Numeric(n) => i64::from(n),
        DieSides::Fate | DieSides::Coin => 1,
    }
}

#[cfg(test)]
#[path = "breakdown_tests.rs"]
mod tests;
