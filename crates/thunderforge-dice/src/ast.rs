//! Spec 014 (FR-004 through FR-009a): the parsed formula AST. Internal
//! to the crate — never crosses the GraphQL boundary directly (only
//! `RollResolution`/`DieOutcome`, the *result* types in `lib.rs`, do).

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Placeholder(String),
    Dice(DiceTerm),
    /// FR-009: a dice-pool grouping (`{term, term, ...}modifier`) — a
    /// shared keep/drop modifier applied across the grouped terms'
    /// totals (each term's own dice still contribute their individual
    /// `DieOutcome`s to the resolution).
    Pool(Vec<Expr>, Vec<Modifier>),
    Neg(Box<Expr>),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    MathFn(MathFn, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathFn {
    Floor,
    Ceil,
    Round,
    Abs,
}

/// A `NdM`-family term: `count` and `sides` are themselves `Expr` so
/// FR-009's nested-dice-size case (`(2d4)d8`, `1d(1d20)`) is representable
/// without a special-cased AST shape — the common case (`4d6`) is just
/// `count = Number(4.0)`, `sides = Sides::Numeric(Number(6.0))`.
#[derive(Debug, Clone, PartialEq)]
pub struct DiceTerm {
    pub count: Box<Expr>,
    pub sides: Sides,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sides {
    Numeric(Box<Expr>),
    /// FR-009a: Fate/Genesis die — three faces, +1/-1/blank(0).
    Fate,
    /// A two-face coin die (heads=1/tails=0), part of this grammar's
    /// die-type breadth alongside numeric and Fate dice.
    Coin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Modifier {
    KeepHighest(u32),
    KeepLowest(u32),
    DropHighest(u32),
    DropLowest(u32),
    /// FR-006: a single reroll — replaces the die's value exactly once
    /// when `condition` matches.
    Reroll(Condition),
    /// FR-006: recursive/repeated reroll — keeps rerolling while
    /// `condition` matches (bounded by the per-die iteration cap).
    RerollRecursive(Condition),
    /// FR-006: exploding — an extra die is added (recursively, bounded
    /// by the per-die iteration cap) whenever `condition` matches.
    Explode(Condition),
    /// FR-006: exploding, but only ever once per original die.
    ExplodeOnce(Condition),
    Min(i64),
    Max(i64),
    /// FR-008: count dice matching `condition` as successes.
    CountSuccesses(Condition),
    /// FR-008: count dice matching `condition` as failures. Combined
    /// with `CountSuccesses` on the same term, the aggregate becomes
    /// successes minus failures (deduct-failures).
    CountFailures(Condition),
    /// FR-008: subtract-failures-by-face-value — dice matching
    /// `condition` subtract their face value from the aggregate instead
    /// of counting as a flat -1.
    SubtractFailureValue(Condition),
    Even,
    Odd,
    /// FR-008: margin of success — the aggregate becomes
    /// `(sum of kept dice) - n`.
    MarginOfSuccess(i64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Condition {
    Eq(i64),
    Gt(i64),
    Gte(i64),
    Lt(i64),
    Lte(i64),
    /// Default condition for a bare `x`/`xo` with no explicit comparison
    /// — "matches the die's own maximum possible face".
    MaxFace,
}
