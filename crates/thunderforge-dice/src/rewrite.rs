//! Spec 084: rewrite a formula's dice terms, and print a formula back out.
//!
//! A pack shapes a roll (advantage, Halfling Luck, Great Weapon Fighting)
//! by editing the dice terms of the formula it declared, never by building
//! a formula string by hand. The edit sees each dice term in order and may
//! change its count and add modifiers. The tree is then printed back out in
//! one canonical order: rerolls, then explosions, then keep and drop, then
//! clamps, then counting. That order changes nothing about what a formula
//! means, because the evaluator applies each family at its own stage.

use crate::ast::{BinOp, Condition, DiceTerm, Expr, MathFn, Modifier, Sides};
use crate::error::FormulaError;
use crate::parser::parse;

/// What an edit sees of one dice term.
#[derive(Debug, Clone, PartialEq)]
pub struct TermView {
    /// The term's place among the formula's dice terms, from 0, left to right.
    pub index: usize,
    /// The number of dice, when it is written as a number.
    pub count: Option<u32>,
    /// The die's faces, when it is a numeric die written as a number.
    pub sides: Option<u32>,
    pub keeps: bool,
    pub rerolls: bool,
    pub clamps: bool,
}

/// A modifier an edit may add to a term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddModifier {
    KeepHighest(u32),
    KeepLowest(u32),
    RerollOnceEq(i64),
    Min(i64),
}

/// What to change about one term.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TermEdit {
    /// A new number of dice, or `None` to keep the count.
    pub count: Option<u32>,
    pub add: Vec<AddModifier>,
}

/// Rewrite `formula`'s dice terms with `edit`.
///
/// When `edit` answers `None` for every term the formula comes back byte for
/// byte. Otherwise the whole formula is printed canonically.
pub fn rewrite_dice_terms(
    formula: &str,
    mut edit: impl FnMut(&TermView) -> Option<TermEdit>,
) -> Result<String, FormulaError> {
    let mut ast = parse(formula)?;
    let mut index = 0;
    let mut changed = false;
    visit_terms(&mut ast, &mut |term| {
        let view = view_of(term, index);
        index += 1;
        if let Some(change) = edit(&view) {
            changed = true;
            apply(term, change);
        }
    });
    if changed {
        Ok(print_expr(&ast))
    } else {
        Ok(formula.to_string())
    }
}

fn visit_terms(expr: &mut Expr, f: &mut impl FnMut(&mut DiceTerm)) {
    match expr {
        Expr::Number(_) | Expr::Placeholder(_) => {}
        Expr::Dice(term) => {
            f(term);
            visit_terms(&mut term.count, f);
            if let Sides::Numeric(sides) = &mut term.sides {
                visit_terms(sides, f);
            }
        }
        Expr::Pool(items, _) => items.iter_mut().for_each(|item| visit_terms(item, f)),
        Expr::Neg(inner) | Expr::MathFn(_, inner) => visit_terms(inner, f),
        Expr::BinOp(lhs, _, rhs) => {
            visit_terms(lhs, f);
            visit_terms(rhs, f);
        }
    }
}

fn literal(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::Number(n) if *n >= 0.0 && n.fract() == 0.0 => Some(*n as u32),
        _ => None,
    }
}

fn view_of(term: &DiceTerm, index: usize) -> TermView {
    let has = |f: fn(&Modifier) -> bool| term.modifiers.iter().any(f);
    TermView {
        index,
        count: literal(&term.count),
        sides: match &term.sides {
            Sides::Numeric(expr) => literal(expr),
            Sides::Fate | Sides::Coin => None,
        },
        keeps: has(|m| family(m) == 2),
        rerolls: has(|m| family(m) == 0),
        clamps: has(|m| family(m) == 3),
    }
}

fn apply(term: &mut DiceTerm, change: TermEdit) {
    if let Some(count) = change.count {
        *term.count = Expr::Number(f64::from(count));
    }
    term.modifiers
        .extend(change.add.into_iter().map(|add| match add {
            AddModifier::KeepHighest(n) => Modifier::KeepHighest(n),
            AddModifier::KeepLowest(n) => Modifier::KeepLowest(n),
            AddModifier::RerollOnceEq(n) => Modifier::Reroll(Condition::Eq(n)),
            AddModifier::Min(n) => Modifier::Min(n),
        }));
    // A stable sort, so modifiers of one family keep their order.
    term.modifiers.sort_by_key(family);
}

/// The canonical print order: rerolls, explosions, keep and drop, clamps,
/// counting.
fn family(modifier: &Modifier) -> u8 {
    match modifier {
        Modifier::Reroll(_) | Modifier::RerollRecursive(_) => 0,
        Modifier::Explode(_) | Modifier::ExplodeOnce(_) => 1,
        Modifier::KeepHighest(_)
        | Modifier::KeepLowest(_)
        | Modifier::DropHighest(_)
        | Modifier::DropLowest(_) => 2,
        Modifier::Min(_) | Modifier::Max(_) => 3,
        Modifier::CountSuccesses(_)
        | Modifier::CountFailures(_)
        | Modifier::SubtractFailureValue(_)
        | Modifier::Even
        | Modifier::Odd
        | Modifier::MarginOfSuccess(_) => 4,
    }
}

/// Print a parsed formula so that parsing the text gives the same tree.
pub(crate) fn print_expr(expr: &Expr) -> String {
    let mut out = String::new();
    write_expr(&mut out, expr);
    out
}

fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::BinOp(_, BinOp::Add | BinOp::Sub, _) => 1,
        Expr::BinOp(_, BinOp::Mul | BinOp::Div, _) => 2,
        _ => 3,
    }
}

fn write_wrapped(out: &mut String, expr: &Expr, wrap: bool) {
    if wrap {
        out.push('(');
        write_expr(out, expr);
        out.push(')');
    } else {
        write_expr(out, expr);
    }
}

fn write_expr(out: &mut String, expr: &Expr) {
    match expr {
        Expr::Number(n) => write_number(out, *n),
        Expr::Placeholder(name) => out.push_str(name),
        Expr::Dice(term) => write_term(out, term),
        Expr::Pool(items, modifiers) => {
            out.push('{');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_expr(out, item);
            }
            out.push('}');
            write_modifiers(out, modifiers);
        }
        Expr::Neg(inner) => {
            out.push('-');
            write_wrapped(out, inner, precedence(inner) < 3);
        }
        Expr::BinOp(lhs, op, rhs) => {
            let mine = precedence(expr);
            write_wrapped(out, lhs, precedence(lhs) < mine);
            out.push_str(match op {
                BinOp::Add => " + ",
                BinOp::Sub => " - ",
                BinOp::Mul => " * ",
                BinOp::Div => " / ",
            });
            // Left-associative: an equal-precedence right side needs parens.
            write_wrapped(out, rhs, precedence(rhs) <= mine);
        }
        Expr::MathFn(kind, inner) => {
            out.push_str(match kind {
                MathFn::Floor => "floor(",
                MathFn::Ceil => "ceil(",
                MathFn::Round => "round(",
                MathFn::Abs => "abs(",
            });
            write_expr(out, inner);
            out.push(')');
        }
    }
}

fn write_number(out: &mut String, n: f64) {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        out.push_str(&format!("{}", n as i64));
    } else {
        out.push_str(&format!("{n}"));
    }
}

/// A count or a size: a bare number, or anything else in parentheses.
fn write_operand(out: &mut String, expr: &Expr) {
    let bare = matches!(expr, Expr::Number(n) if *n >= 0.0);
    write_wrapped(out, expr, !bare);
}

fn write_term(out: &mut String, term: &DiceTerm) {
    write_operand(out, &term.count);
    out.push('d');
    // `F` and `c` are letters, so a modifier straight after one would be
    // read as part of the same word.
    let mut ends_in_letter = true;
    match &term.sides {
        Sides::Fate => out.push('F'),
        Sides::Coin => out.push('c'),
        Sides::Numeric(sides) => {
            write_operand(out, sides);
            ends_in_letter = false;
        }
    }
    if ends_in_letter && !term.modifiers.is_empty() {
        out.push(' ');
    }
    write_modifiers(out, &term.modifiers);
}

fn write_modifiers(out: &mut String, modifiers: &[Modifier]) {
    for (i, modifier) in modifiers.iter().enumerate() {
        // A modifier that ends in a letter (`x`, `eo`) would run into the
        // next one's keyword.
        if i > 0 && out.ends_with(|c: char| c.is_ascii_alphabetic()) {
            out.push(' ');
        }
        write_modifier(out, modifier);
    }
}

fn write_modifier(out: &mut String, modifier: &Modifier) {
    let (keyword, condition, count) = match modifier {
        Modifier::KeepHighest(n) => ("kh", None, Some(i64::from(*n))),
        Modifier::KeepLowest(n) => ("kl", None, Some(i64::from(*n))),
        Modifier::DropHighest(n) => ("dh", None, Some(i64::from(*n))),
        Modifier::DropLowest(n) => ("dl", None, Some(i64::from(*n))),
        Modifier::Reroll(c) => ("r", Some(*c), None),
        Modifier::RerollRecursive(c) => ("rr", Some(*c), None),
        Modifier::Explode(c) => ("x", Some(*c), None),
        Modifier::ExplodeOnce(c) => ("xo", Some(*c), None),
        Modifier::Min(n) => ("min", None, Some(*n)),
        Modifier::Max(n) => ("max", None, Some(*n)),
        Modifier::CountSuccesses(c) => ("cs", Some(*c), None),
        Modifier::CountFailures(c) => ("cf", Some(*c), None),
        Modifier::SubtractFailureValue(c) => ("sf", Some(*c), None),
        Modifier::Even => ("eo", None, None),
        Modifier::Odd => ("od", None, None),
        Modifier::MarginOfSuccess(n) => ("ms", None, Some(*n)),
    };
    out.push_str(keyword);
    if let Some(n) = count {
        out.push_str(&n.to_string());
    }
    if let Some(condition) = condition {
        let (op, n) = match condition {
            Condition::Eq(n) => ("", n),
            Condition::Gt(n) => (">", n),
            Condition::Gte(n) => (">=", n),
            Condition::Lt(n) => ("<", n),
            Condition::Lte(n) => ("<=", n),
            Condition::MaxFace => return,
        };
        out.push_str(op);
        out.push_str(&n.to_string());
    }
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
