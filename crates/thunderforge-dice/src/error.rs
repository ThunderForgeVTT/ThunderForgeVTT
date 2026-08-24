//! Spec 014 (FR-010, FR-011, FR-012): every way a formula can fail to
//! parse or resolve. A `FormulaError` is returned instead of ever
//! guessing, defaulting, truncating, or silently mis-evaluating.

use std::fmt;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FormulaError {
    /// Malformed syntax: unbalanced grouping, unknown modifier, a
    /// malformed comparison condition, or any other parse-time failure.
    ParseError { message: String, position: usize },
    /// An arithmetic sub-expression divides by zero.
    DivisionByZero,
    /// A math function (or the final result) produced NaN/infinity.
    NonFiniteResult,
    /// A formula references a placeholder with no supplied binding
    /// (FR-010) — never silently treated as zero.
    MissingPlaceholder(String),
    /// The FR-012 bound on total dice rolled in one resolution would be
    /// exceeded.
    DiceCountExceeded,
    /// The FR-012 bound on reroll/explosion iterations for a single die
    /// would be exceeded.
    IterationCapExceeded,
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormulaError::ParseError { message, position } => {
                write!(f, "formula parse error at position {position}: {message}")
            }
            FormulaError::DivisionByZero => write!(f, "division by zero"),
            FormulaError::NonFiniteResult => write!(f, "result is not a finite number"),
            FormulaError::MissingPlaceholder(name) => {
                write!(f, "missing value for placeholder \"{name}\"")
            }
            FormulaError::DiceCountExceeded => {
                write!(f, "formula would roll more dice than the allowed bound")
            }
            FormulaError::IterationCapExceeded => {
                write!(f, "a die's reroll/explosion chain exceeded the allowed bound")
            }
        }
    }
}

impl std::error::Error for FormulaError {}
