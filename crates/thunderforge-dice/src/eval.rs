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
            let l = eval_expr(ctx, lhs)?.value;
            let r = eval_expr(ctx, rhs)?.value;
            let value = match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r == 0.0 {
                        return Err(FormulaError::DivisionByZero);
                    }
                    l / r
                }
            };
            if !value.is_finite() {
                return Err(FormulaError::NonFiniteResult);
            }
            Ok(ExprValue::total(value))
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
}
