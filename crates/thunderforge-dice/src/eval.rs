//! Spec 014: `resolve()` — the crate's entire reason to exist. Walks the
//! AST, substitutes placeholders, rolls dice via an injected `RngCore`
//! (never its own entropy — research.md §3), applies every modifier, and
//! enforces the FR-012 bounded-iteration guarantee continuously during
//! evaluation, not just up front (research.md §4).

use std::collections::HashMap;

use rand_core::Rng;

use crate::ast::{BinOp, Condition, DiceTerm, Expr, MathFn, Modifier, Sides};
use crate::error::FormulaError;
use crate::{DiceFormula, DieOutcome, DieSides, ResolutionKind, RollResolution};

/// FR-012: hard cap on total dice rolled (base + every reroll/explosion)
/// across one resolution. Generous enough for any real tabletop formula
/// (spec.md's Assumptions: the exact number is an implementation detail).
pub const MAX_TOTAL_DICE: u32 = 1_000;

/// FR-012: hard cap on reroll/explosion iterations for a single die —
/// what actually makes a formula like `1d6x>=1` terminate instead of
/// hanging.
pub const MAX_ITERATIONS_PER_DIE: u32 = 100;

pub type PlaceholderBindings = HashMap<String, f64>;

struct EvalCtx<'a, R: Rng> {
    rng: &'a mut R,
    bindings: &'a PlaceholderBindings,
    dice: Vec<DieOutcome>,
    total_dice_rolled: u32,
}

impl<R: Rng> EvalCtx<'_, R> {
    fn take_dice_budget(&mut self, count: u32) -> Result<(), FormulaError> {
        self.total_dice_rolled = self
            .total_dice_rolled
            .checked_add(count)
            .ok_or(FormulaError::DiceCountExceeded)?;
        if self.total_dice_rolled > MAX_TOTAL_DICE {
            return Err(FormulaError::DiceCountExceeded);
        }
        Ok(())
    }

    /// Rolls one die of `sides` faces (1..=sides), or the special Fate
    /// (-1/0/1) / Coin (0/1) faces.
    fn roll_face(&mut self, sides: &DieSides) -> i64 {
        match sides {
            DieSides::Numeric(n) => 1 + (self.rng.next_u32() % (*n).max(1)) as i64,
            DieSides::Fate => (self.rng.next_u32() % 3) as i64 - 1,
            DieSides::Coin => (self.rng.next_u32() % 2) as i64,
        }
    }

    fn max_face(&self, sides: &DieSides) -> i64 {
        match sides {
            DieSides::Numeric(n) => *n as i64,
            DieSides::Fate => 1,
            DieSides::Coin => 1,
        }
    }
}

/// The result of evaluating one `Expr` node: a numeric value, plus (only
/// for a dice/pool node carrying a success-counting modifier) the
/// success-count interpretation of that same node — used by `resolve()`
/// to decide the top-level `ResolutionKind` (spec.md Edge Cases: a
/// formula's own notation decides its result shape, never a caller's
/// expectation).
struct ExprValue {
    value: f64,
    success_count: Option<i64>,
}

impl ExprValue {
    fn total(value: f64) -> Self {
        ExprValue { value, success_count: None }
    }
}

fn condition_matches(condition: Condition, value: i64, max_face: i64) -> bool {
    match condition {
        Condition::Eq(n) => value == n,
        Condition::Gt(n) => value > n,
        Condition::Gte(n) => value >= n,
        Condition::Lt(n) => value < n,
        Condition::Lte(n) => value <= n,
        Condition::MaxFace => value == max_face,
    }
}

/// Resolves `formula` (with `bindings` substituted for every placeholder)
/// into a full `RollResolution`, using `rng` as the sole source of
/// randomness (research.md §3: the crate never owns entropy).
pub fn resolve<R: Rng>(
    formula: &DiceFormula,
    bindings: &PlaceholderBindings,
    rng: &mut R,
) -> Result<RollResolution, FormulaError> {
    let mut ctx = EvalCtx { rng, bindings, dice: Vec::new(), total_dice_rolled: 0 };
    let result = eval_expr(&mut ctx, &formula.ast)?;

    if !result.value.is_finite() {
        return Err(FormulaError::NonFiniteResult);
    }

    let kind = match result.success_count {
        Some(count) => ResolutionKind::SuccessCount(count),
        None => ResolutionKind::Total(result.value),
    };

    Ok(RollResolution { formula: formula.source.clone(), dice: ctx.dice, kind })
}

fn eval_expr<R: Rng>(ctx: &mut EvalCtx<R>, expr: &Expr) -> Result<ExprValue, FormulaError> {
    match expr {
        Expr::Number(n) => Ok(ExprValue::total(*n)),
        Expr::Placeholder(name) => {
            let value = ctx
                .bindings
                .get(name)
                .copied()
                .ok_or_else(|| FormulaError::MissingPlaceholder(name.clone()))?;
            Ok(ExprValue::total(value))
        }
        Expr::Neg(inner) => {
            let v = eval_expr(ctx, inner)?;
            Ok(ExprValue::total(-v.value))
        }
        Expr::BinOp(lhs, op, rhs) => {
            let l = eval_expr(ctx, lhs)?;
            let r = eval_expr(ctx, rhs)?;
            let value = match op {
                BinOp::Add => l.value + r.value,
                BinOp::Sub => l.value - r.value,
                BinOp::Mul => l.value * r.value,
                BinOp::Div => {
                    if r.value == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    l.value / r.value
                }
            };
            if !value.is_finite() {
                return Err(FormulaError::NonFiniteResult);
            }
            // Bug fix (found while building the year_zero_engine pack,
            // spec 018): a formula like `NdXcs>=T + MdYcs>=T` — two
            // independently success-counting pools summed — used to lose
            // both sides' success_count here, silently degrading to
            // ResolutionKind::Total. Only `+` has an unambiguous meaning
            // for combining success counts (successes from both pools,
            // summed); Sub/Mul/Div on a success-counting operand have no
            // sensible success-count semantics, so they still degrade to a
            // plain numeric total, same as before.
            let success_count = match op {
                BinOp::Add => match (l.success_count, r.success_count) {
                    (Some(a), Some(b)) => Some(a + b),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                },
                _ => None,
            };
            Ok(ExprValue { value, success_count })
        }
        Expr::MathFn(kind, inner) => {
            let v = eval_expr(ctx, inner)?.value;
            let value = match kind {
                MathFn::Floor => v.floor(),
                MathFn::Ceil => v.ceil(),
                MathFn::Round => v.round(),
                MathFn::Abs => v.abs(),
            };
            if !value.is_finite() {
                return Err(FormulaError::NonFiniteResult);
            }
            Ok(ExprValue::total(value))
        }
        Expr::Dice(term) => eval_dice_term(ctx, term),
        Expr::Pool(items, modifiers) => eval_pool(ctx, items, modifiers),
    }
}

fn eval_int_expr<R: Rng>(ctx: &mut EvalCtx<R>, expr: &Expr) -> Result<i64, FormulaError> {
    let v = eval_expr(ctx, expr)?.value;
    if !v.is_finite() {
        return Err(FormulaError::NonFiniteResult);
    }
    Ok(v.round() as i64)
}

fn eval_dice_term<R: Rng>(ctx: &mut EvalCtx<R>, term: &DiceTerm) -> Result<ExprValue, FormulaError> {
    let count = eval_int_expr(ctx, &term.count)?;
    if count < 0 {
        return Err(FormulaError::ParseError {
            message: "dice count must not be negative".to_string(),
            position: 0,
        });
    }
    let count = count as u32;

    let sides = match &term.sides {
        Sides::Fate => DieSides::Fate,
        Sides::Coin => DieSides::Coin,
        Sides::Numeric(expr) => {
            let n = eval_int_expr(ctx, expr)?;
            if n < 1 {
                return Err(FormulaError::ParseError {
                    message: "die size must be at least 1".to_string(),
                    position: 0,
                });
            }
            DieSides::Numeric(n as u32)
        }
    };

    ctx.take_dice_budget(count)?;

    let max_face = ctx.max_face(&sides);

    // Roll every base die, applying reroll/explode modifiers as each
    // die's own bounded chain (FR-012, FR-013's "full chain, not just
    // the final kept value").
    let reroll_once = find_condition(&term.modifiers, |m| match m {
        Modifier::Reroll(c) => Some(*c),
        _ => None,
    });
    let reroll_recursive = find_condition(&term.modifiers, |m| match m {
        Modifier::RerollRecursive(c) => Some(*c),
        _ => None,
    });
    let explode = find_condition(&term.modifiers, |m| match m {
        Modifier::Explode(c) => Some(*c),
        _ => None,
    });
    let explode_once = find_condition(&term.modifiers, |m| match m {
        Modifier::ExplodeOnce(c) => Some(*c),
        _ => None,
    });

    let mut outcomes: Vec<DieOutcome> = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let mut rolls = vec![ctx.roll_face(&sides)];
        let mut iterations = 0u32;

        if let Some(cond) = reroll_once
            && condition_matches(cond, *rolls.last().unwrap(), max_face)
        {
            ctx.take_dice_budget(1)?;
            rolls.push(ctx.roll_face(&sides));
        }

        if let Some(cond) = reroll_recursive {
            while condition_matches(cond, *rolls.last().unwrap(), max_face) {
                iterations += 1;
                if iterations > MAX_ITERATIONS_PER_DIE {
                    return Err(FormulaError::IterationCapExceeded);
                }
                ctx.take_dice_budget(1)?;
                rolls.push(ctx.roll_face(&sides));
            }
        }

        if let Some(cond) = explode_once
            && condition_matches(cond, *rolls.last().unwrap(), max_face)
        {
            ctx.take_dice_budget(1)?;
            rolls.push(ctx.roll_face(&sides));
        }

        if let Some(cond) = explode {
            while condition_matches(cond, *rolls.last().unwrap(), max_face) {
                iterations += 1;
                if iterations > MAX_ITERATIONS_PER_DIE {
                    return Err(FormulaError::IterationCapExceeded);
                }
                ctx.take_dice_budget(1)?;
                rolls.push(ctx.roll_face(&sides));
            }
        }

        let final_value = *rolls.last().unwrap();
        outcomes.push(DieOutcome { sides, rolls, kept: true, final_value });
    }

    apply_keep_drop(&mut outcomes, &term.modifiers);
    apply_clamp(&mut outcomes, &term.modifiers);

    let success_count = compute_success_count(&outcomes, &term.modifiers, max_face);

    let value = match success_count {
        Some(n) => n as f64,
        None => outcomes.iter().filter(|d| d.kept).map(|d| d.final_value).sum::<i64>() as f64,
    };

    ctx.dice.extend(outcomes);

    Ok(ExprValue { value, success_count })
}

fn find_condition<F>(modifiers: &[Modifier], f: F) -> Option<Condition>
where
    F: Fn(&Modifier) -> Option<Condition>,
{
    modifiers.iter().find_map(f)
}

fn apply_keep_drop(outcomes: &mut [DieOutcome], modifiers: &[Modifier]) {
    let mut indices: Vec<usize> = (0..outcomes.len()).collect();

    for modifier in modifiers {
        match modifier {
            Modifier::KeepHighest(n) => {
                indices.sort_by_key(|&i| std::cmp::Reverse(outcomes[i].final_value));
                mark_drop_after(outcomes, &indices, *n as usize);
            }
            Modifier::KeepLowest(n) => {
                indices.sort_by_key(|&i| outcomes[i].final_value);
                mark_drop_after(outcomes, &indices, *n as usize);
            }
            Modifier::DropHighest(n) => {
                indices.sort_by_key(|&i| std::cmp::Reverse(outcomes[i].final_value));
                mark_drop_first(outcomes, &indices, *n as usize);
            }
            Modifier::DropLowest(n) => {
                indices.sort_by_key(|&i| outcomes[i].final_value);
                mark_drop_first(outcomes, &indices, *n as usize);
            }
            _ => {}
        }
    }
}

fn mark_drop_after(outcomes: &mut [DieOutcome], sorted_indices: &[usize], keep_n: usize) {
    for &i in sorted_indices.iter().skip(keep_n) {
        outcomes[i].kept = false;
    }
}

fn mark_drop_first(outcomes: &mut [DieOutcome], sorted_indices: &[usize], drop_n: usize) {
    for &i in sorted_indices.iter().take(drop_n) {
        outcomes[i].kept = false;
    }
}

fn apply_clamp(outcomes: &mut [DieOutcome], modifiers: &[Modifier]) {
    for modifier in modifiers {
        match modifier {
            Modifier::Min(n) => {
                for die in outcomes.iter_mut() {
                    if die.final_value < *n {
                        die.final_value = *n;
                    }
                }
            }
            Modifier::Max(n) => {
                for die in outcomes.iter_mut() {
                    if die.final_value > *n {
                        die.final_value = *n;
                    }
                }
            }
            _ => {}
        }
    }
}

/// FR-008: returns `Some(count)` when the term has any success-counting
/// modifier, combining successes/failures/margin-of-success/even-odd
/// per this crate's chosen semantics (research.md/parser.rs doc
/// comments record these as this crate's own concrete notation choices,
/// the spec itself only requires the underlying capability).
fn compute_success_count(outcomes: &[DieOutcome], modifiers: &[Modifier], max_face: i64) -> Option<i64> {
    let kept: Vec<&DieOutcome> = outcomes.iter().filter(|d| d.kept).collect();

    let successes = find_condition(modifiers, |m| match m {
        Modifier::CountSuccesses(c) => Some(*c),
        _ => None,
    });
    let failures = find_condition(modifiers, |m| match m {
        Modifier::CountFailures(c) => Some(*c),
        _ => None,
    });
    let subtract_failure_value = find_condition(modifiers, |m| match m {
        Modifier::SubtractFailureValue(c) => Some(*c),
        _ => None,
    });
    let even = modifiers.iter().any(|m| matches!(m, Modifier::Even));
    let odd = modifiers.iter().any(|m| matches!(m, Modifier::Odd));
    let margin = modifiers.iter().find_map(|m| match m {
        Modifier::MarginOfSuccess(n) => Some(*n),
        _ => None,
    });

    if let Some(n) = margin {
        let sum: i64 = kept.iter().map(|d| d.final_value).sum();
        return Some(sum - n);
    }

    if even || odd {
        let want_even = even;
        let count = kept.iter().filter(|d| (d.final_value % 2 == 0) == want_even).count() as i64;
        return Some(count);
    }

    if successes.is_none() && failures.is_none() && subtract_failure_value.is_none() {
        return None;
    }

    let mut total = 0i64;
    if let Some(cond) = successes {
        total += kept.iter().filter(|d| condition_matches(cond, d.final_value, max_face)).count() as i64;
    }
    if let Some(cond) = failures {
        total -= kept.iter().filter(|d| condition_matches(cond, d.final_value, max_face)).count() as i64;
    }
    if let Some(cond) = subtract_failure_value {
        total -= kept
            .iter()
            .filter(|d| condition_matches(cond, d.final_value, max_face))
            .map(|d| d.final_value)
            .sum::<i64>();
    }
    Some(total)
}

fn eval_pool<R: Rng>(
    ctx: &mut EvalCtx<R>,
    items: &[Expr],
    modifiers: &[Modifier],
) -> Result<ExprValue, FormulaError> {
    let mut totals = Vec::with_capacity(items.len());
    for item in items {
        totals.push(eval_expr(ctx, item)?.value);
    }

    // Pool-level keep/drop applies to each grouped term's *total*
    // (FR-009's "shared modifier applied across grouped terms" — this
    // crate's chosen granularity for pool composition, documented in
    // parser.rs).
    let mut indices: Vec<usize> = (0..totals.len()).collect();
    let mut kept = vec![true; totals.len()];

    for modifier in modifiers {
        match modifier {
            Modifier::KeepHighest(n) => {
                indices.sort_by(|&a, &b| totals[b].partial_cmp(&totals[a]).unwrap());
                for &i in indices.iter().skip(*n as usize) {
                    kept[i] = false;
                }
            }
            Modifier::KeepLowest(n) => {
                indices.sort_by(|&a, &b| totals[a].partial_cmp(&totals[b]).unwrap());
                for &i in indices.iter().skip(*n as usize) {
                    kept[i] = false;
                }
            }
            Modifier::DropHighest(n) => {
                indices.sort_by(|&a, &b| totals[b].partial_cmp(&totals[a]).unwrap());
                for &i in indices.iter().take(*n as usize) {
                    kept[i] = false;
                }
            }
            Modifier::DropLowest(n) => {
                indices.sort_by(|&a, &b| totals[a].partial_cmp(&totals[b]).unwrap());
                for &i in indices.iter().take(*n as usize) {
                    kept[i] = false;
                }
            }
            _ => {}
        }
    }

    let value = totals.iter().zip(kept.iter()).filter(|&(_, &k)| k).map(|(v, _)| *v).sum();
    Ok(ExprValue::total(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiceFormula;

    /// Deterministic mock RNG: cycles through a fixed sequence of
    /// pre-chosen `next_u32` values, so tests can pin exact die results
    /// without any OS entropy (Constitution Principle II — independently
    /// testable with no external dependency).
    struct ScriptedRng {
        values: Vec<u32>,
        pos: usize,
    }

    impl ScriptedRng {
        fn new(values: Vec<u32>) -> Self {
            ScriptedRng { values, pos: 0 }
        }
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

    fn resolve_str(source: &str, values: Vec<u32>) -> Result<RollResolution, FormulaError> {
        let formula = DiceFormula::parse(source)?;
        let mut rng = ScriptedRng::new(values);
        resolve(&formula, &PlaceholderBindings::new(), &mut rng)
    }

    #[test]
    fn arithmetic_4d6_plus_2() {
        // rolls (0-indexed face = value-1): 3,4,2,5 -> faces 4,5,3,6
        let result = resolve_str("4d6+2", vec![3, 4, 2, 5]).unwrap();
        assert_eq!(result.dice.len(), 4);
        assert_eq!(result.kind, ResolutionKind::Total(4.0 + 5.0 + 3.0 + 6.0 + 2.0));
    }

    #[test]
    fn keep_highest_drops_are_shown_not_hidden() {
        // faces: 6,1,4,2 -> keep highest 3 (6,4,2), drop the 1
        let result = resolve_str("4d6kh3", vec![5, 0, 3, 1]).unwrap();
        assert_eq!(result.dice.len(), 4);
        let kept: Vec<_> = result.dice.iter().filter(|d| d.kept).collect();
        assert_eq!(kept.len(), 3);
        let dropped: Vec<_> = result.dice.iter().filter(|d| !d.kept).collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].final_value, 1);
    }

    #[test]
    fn reroll_shows_multi_entry_rolls_chain() {
        // first die: face 1 (value 1) triggers reroll (r1), reroll -> face 5 (value 5)
        // second die: face 4 (value 4), no reroll
        let result = resolve_str("2d6r1", vec![0, 4, 3]).unwrap();
        let rerolled = result.dice.iter().find(|d| d.rolls.len() > 1).expect("one die should have rerolled");
        assert_eq!(rerolled.rolls[0], 1);
        assert_eq!(rerolled.final_value, 5);
    }

    #[test]
    fn explode_chain_on_max_face() {
        // d6 exploding on max: face 6 (value 5) -> explode -> face 6 (5) -> explode -> face 2 (1)
        let result = resolve_str("1d6x", vec![5, 5, 1]).unwrap();
        assert_eq!(result.dice.len(), 1);
        assert_eq!(result.dice[0].rolls, vec![6, 6, 2]);
        assert_eq!(result.dice[0].final_value, 2);
    }

    #[test]
    fn success_counting_pool() {
        // 8 d10s, count successes >= 7. faces (value+1): 7,3,8,10,1,7,6,9 -> 5 successes (7,8,10,7,9)
        let faces = vec![6, 2, 7, 9, 0, 6, 5, 8];
        let result = resolve_str("8d10cs>=7", faces).unwrap();
        assert_eq!(result.kind, ResolutionKind::SuccessCount(5));
    }

    #[test]
    fn fate_die_faces_in_range() {
        // face values 0,1,2 map to -1,0,1
        let result = resolve_str("4dF", vec![0, 1, 2, 1]).unwrap();
        assert_eq!(result.dice.len(), 4);
        for die in &result.dice {
            assert!(die.final_value >= -1 && die.final_value <= 1);
            assert_eq!(die.sides, DieSides::Fate);
        }
        assert_eq!(result.kind, ResolutionKind::Total(-1.0 + 0.0 + 1.0 + 0.0));
    }

    #[test]
    fn fate_core_ladder_roll_with_skill_placeholder() {
        // Fate Core's resolution formula: 4dF + skill rating on the Ladder.
        // Faces 0,1,2,1 -> -1,0,1,0 (sum -0.0) plus a Good (+3) skill.
        let formula = DiceFormula::parse("4dF + SKILL").unwrap();
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("SKILL".to_string(), 3.0);
        let mut rng = ScriptedRng::new(vec![0, 1, 2, 1]);
        let result = resolve(&formula, &bindings, &mut rng).unwrap();
        assert_eq!(result.dice.len(), 4);
        for die in &result.dice {
            assert!(die.final_value >= -1 && die.final_value <= 1);
            assert_eq!(die.sides, DieSides::Fate);
        }
        assert_eq!(result.kind, ResolutionKind::Total(-1.0 + 0.0 + 1.0 + 0.0 + 3.0));
    }

    #[test]
    fn nested_dice_size() {
        // (2d4) -> faces 2,3 (values 2,3) sum=5 dice -> 5d8, each rolls face value+1
        let result = resolve_str("(2d4)d8", vec![1, 2, 0, 1, 2, 3, 4]).unwrap();
        // 2 dice for the (2d4) plus 5 dice for the d8 outer roll
        assert_eq!(result.dice.len(), 7);
    }

    #[test]
    fn math_function_floor() {
        // 1d20 face value 9 (index 8) -> floor(9/2) = 4
        let result = resolve_str("floor(1d20/2)", vec![8]).unwrap();
        assert_eq!(result.kind, ResolutionKind::Total(4.0));
    }

    #[test]
    fn malformed_formula_rolls_no_dice() {
        let err = DiceFormula::parse("1d20 +").unwrap_err();
        assert!(matches!(err, FormulaError::ParseError { .. }));
    }

    #[test]
    fn unbounded_explode_condition_is_capped_not_hung() {
        // 1d6x>=1 always matches, forcing the iteration cap to trigger.
        let mut infinite_faces = vec![0u32; (MAX_ITERATIONS_PER_DIE as usize) + 10];
        infinite_faces.iter_mut().for_each(|v| *v = 0); // face value 1 every time
        let err = resolve_str("1d6x>=1", infinite_faces).unwrap_err();
        assert_eq!(err, FormulaError::IterationCapExceeded);
    }

    #[test]
    fn placeholder_substitution_changes_result_by_exact_delta() {
        let formula = DiceFormula::parse("1d20 + STAT").unwrap();
        let mut bindings_low = PlaceholderBindings::new();
        bindings_low.insert("STAT".to_string(), 3.0);
        let mut rng = ScriptedRng::new(vec![9]);
        let low = resolve(&formula, &bindings_low, &mut rng).unwrap();

        let mut bindings_high = PlaceholderBindings::new();
        bindings_high.insert("STAT".to_string(), 8.0);
        let mut rng = ScriptedRng::new(vec![9]);
        let high = resolve(&formula, &bindings_high, &mut rng).unwrap();

        let ResolutionKind::Total(low_v) = low.kind else { panic!("expected Total") };
        let ResolutionKind::Total(high_v) = high.kind else { panic!("expected Total") };
        assert_eq!(high_v - low_v, 5.0);
    }

    #[test]
    fn missing_placeholder_is_rejected_not_defaulted() {
        let formula = DiceFormula::parse("1d20 + STAT").unwrap();
        let mut rng = ScriptedRng::new(vec![9]);
        let err = resolve(&formula, &PlaceholderBindings::new(), &mut rng).unwrap_err();
        assert_eq!(err, FormulaError::MissingPlaceholder("STAT".to_string()));
    }

    #[test]
    fn formula_without_placeholders_ignores_bindings() {
        let formula = DiceFormula::parse("2d8").unwrap();
        let mut rng_a = ScriptedRng::new(vec![3, 5]);
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("UNUSED".to_string(), 99.0);
        let with_bindings = resolve(&formula, &bindings, &mut rng_a).unwrap();

        let mut rng_b = ScriptedRng::new(vec![3, 5]);
        let without_bindings = resolve(&formula, &PlaceholderBindings::new(), &mut rng_b).unwrap();

        assert_eq!(with_bindings.kind, without_bindings.kind);
    }

    /// Spec 018 (Genie) User Story 1 / T019: the Manifestation roll —
    /// keep-highest, exploding, and success-counting composed together in
    /// one formula, with a placeholder driving the dice count. This is
    /// the exact "hardest three features at once" composition spec 018
    /// exists to exercise (not new engine capability, just new coverage).
    #[test]
    fn genie_manifestation_roll_composes_keep_explode_and_success_count() {
        // 4 dice (bound via the `skill` placeholder), keep top 3, explode
        // on 6, count successes at 4+.
        let formula = DiceFormula::parse("(skill)d6kh3x=6cs>=4").unwrap();
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("skill".to_string(), 4.0);

        // die1: raw 5 -> face 6, explodes; explosion raw 2 -> face 3 (stop)
        // die2: raw 3 -> face 4
        // die3: raw 1 -> face 2
        // die4: raw 0 -> face 1
        let mut rng = ScriptedRng::new(vec![5, 2, 3, 1, 0]);
        let result = resolve(&formula, &bindings, &mut rng).unwrap();

        // Exploding a die extends its own `rolls` chain rather than
        // adding a new pool entry, so 4 dice were requested and 4
        // `DieOutcome`s are present overall.
        assert_eq!(result.dice.len(), 4);

        let exploded = result.dice.iter().find(|d| d.rolls.len() > 1).expect("one die should have exploded");
        assert_eq!(exploded.rolls, vec![6, 3], "full chain (original 6 plus every explosion) must be recorded");
        assert_eq!(exploded.final_value, 3);
        assert!(exploded.kept, "the exploded die's final value (3) is in the top 3 and should be kept");

        let kept: Vec<_> = result.dice.iter().filter(|d| d.kept).collect();
        let dropped: Vec<_> = result.dice.iter().filter(|d| !d.kept).collect();
        assert_eq!(kept.len(), 3, "kh3 should keep exactly 3 of the 4 dice");
        assert_eq!(dropped.len(), 1, "kh3 should drop exactly 1 of the 4 dice");
        assert_eq!(dropped[0].final_value, 1, "the lowest die (face 1) should be the one dropped");

        // Successes: kept final values are 4 (die2), 3 (exploded die1),
        // 2 (die3) -> only the 4 counts as a success at cs>=4.
        assert_eq!(result.kind, ResolutionKind::SuccessCount(1));
    }

    /// packs/systems/pathfinder2e system pack: confirms the pack's
    /// `system.json` "coreCheck" formula (`1d20+modifier`) is a real,
    /// resolvable formula in this engine's grammar. `modifier` here
    /// stands in for PF2e's already-summed total (ability modifier +
    /// proficiency bonus + any circumstance/status/item bonuses —
    /// Player Core "Checks," p.400-401); the dice engine only cares
    /// that it's one placeholder bound to a single flat number. Degree
    /// of success (critical success/success/failure/critical failure,
    /// per the DC-comparison table in Player Core p.401) is application
    /// logic layered on top of this raw d20+modifier total, not a
    /// notation this engine has (no success-threshold-vs-DC comparator
    /// exists in the grammar — `cs{cond}`/`cf{cond}` count successes
    /// across a *pool* of dice, which doesn't model a single d20 vs a
    /// scalar DC).
    #[test]
    fn pathfinder2e_core_check_formula_resolves_d20_plus_modifier() {
        let formula = DiceFormula::parse("1d20+modifier").unwrap();
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("modifier".to_string(), 7.0);

        // raw 14 -> face 15 (ScriptedRng values are 0-indexed raw d20 rolls)
        let mut rng = ScriptedRng::new(vec![14]);
        let result = resolve(&formula, &bindings, &mut rng).unwrap();

        let ResolutionKind::Total(total) = result.kind else { panic!("expected Total") };
        assert_eq!(total, 22.0, "1d20(15) + modifier(7) = 22");
        assert_eq!(result.dice.len(), 1);
        assert_eq!(result.dice[0].final_value, 15);
    }

    /// packs/systems/cypher_system: confirms `system.json`'s
    /// `taskResolution.formula` ("1d20") is a real, resolvable formula.
    /// The Cypher System's target number (difficulty * 3) is dynamic per
    /// roll (the GM sets difficulty 1-10 per task), and this grammar's
    /// only success-threshold notation (`cs{cond}`) requires a literal
    /// numeric condition rather than a placeholder-driven one — so the
    /// meets-or-beats-target-number comparison is deliberately left to
    /// application logic layered on top of a plain d20 roll, not
    /// expressed inside the formula string itself.
    #[test]
    fn cypher_system_task_resolution_formula_resolves_plain_1d20() {
        let formula = DiceFormula::parse("1d20").unwrap();
        // raw 16 -> face 17 (ScriptedRng values are 0-indexed raw d20 rolls)
        let mut rng = ScriptedRng::new(vec![16]);
        let result = resolve(&formula, &PlaceholderBindings::new(), &mut rng).unwrap();

        assert_eq!(result.dice.len(), 1);
        assert_eq!(result.dice[0].final_value, 17);
        assert_eq!(result.kind, ResolutionKind::Total(17.0));

        // Application-side comparison: a difficulty-4 task has target
        // number 12 (4 * 3); a roll of 17 meets/beats it and succeeds.
        let target_number = 4 * 3;
        assert!(17 >= target_number, "roll of 17 should meet/beat target number 12");
    }

    /// packs/systems/blades_in_the_dark: confirms `system.json`'s
    /// `actionRoll.formula` ("(rating)d6kh1") is a real, resolvable
    /// formula for Blades' core action roll — roll a pool of d6s equal to
    /// the action rating (bound via the `rating` placeholder) and take
    /// the single highest die. The zero-rating special case (2d6, keep
    /// the LOWEST die) is a separate fixed formula ("2d6kl1") chosen by
    /// application logic rather than expressed here, since this grammar
    /// has no conditional-on-placeholder-value branching.
    #[test]
    fn blades_in_the_dark_action_roll_formula_keeps_single_highest_die() {
        let formula = DiceFormula::parse("(rating)d6kh1").unwrap();
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("rating".to_string(), 3.0);

        // raw 1,4,2 -> faces 2,5,3 -> kh1 keeps the highest (5)
        let mut rng = ScriptedRng::new(vec![1, 4, 2]);
        let result = resolve(&formula, &bindings, &mut rng).unwrap();

        assert_eq!(result.dice.len(), 3, "a rating of 3 should roll a pool of 3d6");
        let kept: Vec<_> = result.dice.iter().filter(|d| d.kept).collect();
        assert_eq!(kept.len(), 1, "kh1 should keep exactly one die");
        assert_eq!(kept[0].final_value, 5, "the highest of faces 2,5,3 is 5");
        assert_eq!(result.kind, ResolutionKind::Total(5.0));
    }

    /// packs/systems/year_zero_engine: confirms `system.json`'s `skillRoll.formula`
    /// (`"(attribute+skill)d6cs>=6"`) is a real, resolvable formula in this engine's
    /// grammar, and that it implements YZE's core dice-pool mechanic (standard d6-pool
    /// variant): roll a pool of Base Dice (attribute) + Skill Dice (skill), all d6,
    /// counting each 6 as a success. `attribute` and `skill` are two placeholders
    /// summed inside the parenthesized dice-count expression — the same real grammar
    /// feature already exercised by the `(skill)d6kh3x=6cs>=4` Genie test above — which
    /// produces one combined d6 pool so `cs>=6` reports a single ResolutionKind::SuccessCount
    /// over the whole pool.
    #[test]
    fn year_zero_engine_skill_roll_formula_counts_sixes_across_combined_pool() {
        let formula = DiceFormula::parse("(attribute+skill)d6cs>=6").unwrap();
        let mut bindings = PlaceholderBindings::new();
        bindings.insert("attribute".to_string(), 2.0);
        bindings.insert("skill".to_string(), 1.0);

        // Pool of 3 (attribute 2 + skill 1) d6. Raw values 5,5,0 -> faces 6,6,1:
        // two sixes (successes), one non-six.
        let mut rng = ScriptedRng::new(vec![5, 5, 0]);
        let result = resolve(&formula, &bindings, &mut rng).unwrap();

        assert_eq!(result.dice.len(), 3);
        assert_eq!(result.kind, ResolutionKind::SuccessCount(2));
    }

    /// Regression test for a bug found while building the year_zero_engine pack
    /// (spec 018): two independently success-counting dice pools added together
    /// (`NdXcs>=T + MdYcs>=T`) used to have `eval_expr`'s `BinOp::Add` arm discard
    /// both sides' `success_count`, silently reporting `ResolutionKind::Total`
    /// instead of the combined success count. Fixed by summing each side's
    /// `success_count` when both are present (and propagating whichever side has
    /// one, if only one does) for `+` specifically — the only operator with an
    /// unambiguous "combine these two pools' successes" meaning.
    #[test]
    fn adding_two_independently_success_counting_pools_sums_their_success_counts() {
        let formula = DiceFormula::parse("2d6cs>=6+1d6cs>=6").unwrap();

        // Pool A (2d6): raw 5,0 -> faces 6,1 -> one success.
        // Pool B (1d6): raw 5 -> face 6 -> one success.
        // Combined: 2 successes total, not a plain numeric total of the faces.
        let mut rng = ScriptedRng::new(vec![5, 0, 5]);
        let result = resolve(&formula, &PlaceholderBindings::new(), &mut rng).unwrap();

        assert_eq!(result.dice.len(), 3);
        assert_eq!(
            result.kind,
            ResolutionKind::SuccessCount(2),
            "two separately-thresholded pools added together must sum their success counts, not degrade to a Total"
        );
    }

    /// A success-counting pool added to a plain flat number (no success_count on
    /// that side) should still report the pool's own success count, not silently
    /// drop it just because the other operand wasn't itself success-counting.
    #[test]
    fn success_counting_pool_plus_flat_number_still_reports_success_count() {
        let formula = DiceFormula::parse("2d6cs>=6+3").unwrap();

        // 2d6: raw 5,0 -> faces 6,1 -> one success. The "+3" is flavor/bonus text
        // some systems might tack on, not meaningful as a success-count offset.
        let mut rng = ScriptedRng::new(vec![5, 0]);
        let result = resolve(&formula, &PlaceholderBindings::new(), &mut rng).unwrap();

        assert_eq!(result.kind, ResolutionKind::SuccessCount(1));
    }
}
