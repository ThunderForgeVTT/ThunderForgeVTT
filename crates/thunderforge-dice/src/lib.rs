//! Spec 014: a standalone, zero-Bevy/zero-wasm-bindgen dice-formula
//! grammar and RNG-agnostic evaluator, importable natively (`src/server`)
//! and under `wasm32-unknown-unknown` (`src/engine`) with no
//! target-specific code inside the crate itself (research.md §3). The
//! evaluator's `resolve()` takes an injected `RngCore` and never reaches
//! for its own entropy — the caller that supplies a real,
//! OS-entropy-backed RNG (only `src/server`'s `rollDice` mutation) is
//! the sole source of an authoritative roll (FR-001).

mod ast;
mod error;
mod eval;
mod parser;

pub use error::FormulaError;
pub use eval::{MAX_ITERATIONS_PER_DIE, MAX_TOTAL_DICE, PlaceholderBindings, resolve};

/// A parsed, validated formula. The only way to obtain one is
/// `DiceFormula::parse` — an unparseable string never produces a value
/// (FR-011).
#[derive(Debug, Clone, PartialEq)]
pub struct DiceFormula {
    pub(crate) source: String,
    pub(crate) ast: ast::Expr,
}

impl DiceFormula {
    pub fn parse(source: &str) -> Result<Self, FormulaError> {
        let ast = parser::parse(source)?;
        Ok(DiceFormula {
            source: source.to_string(),
            ast,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

/// What kind of die a `DieOutcome` came from.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DieSides {
    Numeric(u32),
    Fate,
    Coin,
}

/// One individual die's full history within a resolution (FR-013) — the
/// unit the presentation/animation layer renders.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DieOutcome {
    pub sides: DieSides,
    /// Every value this die produced, in order — index 0 is the
    /// original roll, subsequent entries are rerolls/explosions of
    /// *this* die.
    pub rolls: Vec<i64>,
    /// Whether this die's final value contributed to the aggregated
    /// result (`false` for a die dropped by a keep/drop modifier —
    /// dropped dice are represented, never hidden).
    pub kept: bool,
    pub final_value: i64,
}

/// A summed total (most formulas) or a success/failure count
/// (dice-pool-style formulas) — the shape is decided by the formula's
/// own notation, never a caller's expectation.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ResolutionKind {
    Total(f64),
    SuccessCount(i64),
}

/// The result of one `resolve()` call.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RollResolution {
    /// The original formula source that was resolved (audit clarity).
    /// Placeholder names are not rewritten into this string — the
    /// substituted values are recorded separately by the caller
    /// (`world_roll_records.bindings`, data-model.md).
    pub formula: String,
    /// Every individual die actually rolled, including every
    /// reroll/explosion (FR-013).
    pub dice: Vec<DieOutcome>,
    pub kind: ResolutionKind,
}
