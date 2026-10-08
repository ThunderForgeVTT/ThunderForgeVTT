//! Rolls and how they resolved (spec 014).

use async_graphql::SimpleObject;

use crate::models::RollRecord;
use thunderforge_canvas_core::system_contribution::{RollOutcome, Verdict};
use thunderforge_dice::{ChainStep, DieSides, ResolutionKind, RollResolution};

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DieSidesKind {
    Numeric,
    Fate,
    Coin,
}

/// Why a value in a die's chain after the first was rolled (spec 083).
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DieStep {
    Reroll,
    Explode,
}

impl From<ChainStep> for DieStep {
    fn from(step: ChainStep) -> Self {
        match step {
            ChainStep::Reroll => DieStep::Reroll,
            ChainStep::Explode => DieStep::Explode,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDieOutcome {
    pub sides_kind: DieSidesKind,
    /// Set iff `sides_kind == NUMERIC` (e.g. 20 for a d20).
    pub numeric_sides: Option<i32>,
    /// Full chain: original roll + every reroll/explosion of this die.
    pub rolls: Vec<i32>,
    /// Why each value after the first was rolled: steps[i] explains
    /// rolls[i + 1]. Empty for rolls stored before spec 083.
    pub steps: Vec<DieStep>,
    pub kept: bool,
    pub final_value: i32,
}

impl From<&thunderforge_dice::DieOutcome> for GraphQLDieOutcome {
    fn from(outcome: &thunderforge_dice::DieOutcome) -> Self {
        let (sides_kind, numeric_sides) = match outcome.sides {
            DieSides::Numeric(n) => (DieSidesKind::Numeric, Some(n as i32)),
            DieSides::Fate => (DieSidesKind::Fate, None),
            DieSides::Coin => (DieSidesKind::Coin, None),
        };
        GraphQLDieOutcome {
            sides_kind,
            numeric_sides,
            rolls: outcome.rolls.iter().map(|v| *v as i32).collect(),
            steps: outcome.steps.iter().map(|s| DieStep::from(*s)).collect(),
            kept: outcome.kept,
            final_value: outcome.final_value as i32,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RollResultKind {
    Total,
    SuccessCount,
}

/// How a roll came out, from the host's closed list (spec 067 FR-032).
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RollVerdict {
    Success,
    Failure,
    Tie,
    CriticalSuccess,
    CriticalFailure,
}

impl From<Verdict> for RollVerdict {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Success => RollVerdict::Success,
            Verdict::Failure => RollVerdict::Failure,
            Verdict::Tie => RollVerdict::Tie,
            Verdict::CriticalSuccess => RollVerdict::CriticalSuccess,
            Verdict::CriticalFailure => RollVerdict::CriticalFailure,
        }
    }
}

/// A roll as its system's adjudicator judged it.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLRollOutcome {
    pub verdict: RollVerdict,
    /// The verdict in the system's own words.
    pub label: String,
}

impl From<&RollOutcome> for GraphQLRollOutcome {
    fn from(outcome: &RollOutcome) -> Self {
        GraphQLRollOutcome {
            verdict: outcome.verdict.into(),
            label: outcome.label.clone(),
        }
    }
}

/// The outcome stored with a roll, when one was and it still reads.
pub fn stored_outcome(row: &RollRecord) -> Option<GraphQLRollOutcome> {
    let stored = row.outcome.clone()?;
    serde_json::from_value::<RollOutcome>(stored)
        .ok()
        .map(|outcome| GraphQLRollOutcome::from(&outcome))
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLRollResolution {
    /// The resolved formula (original source; see `RollResolution::formula`
    /// doc comment — placeholder substitutions are recorded separately).
    pub formula: String,
    pub dice: Vec<GraphQLDieOutcome>,
    pub result_kind: RollResultKind,
    /// The total, or the success count, per `result_kind`.
    pub result_value: f64,
    /// How the world's system judged the roll. Null when nothing judged it:
    /// the system has no adjudicator, or there was nothing to judge against.
    pub outcome: Option<GraphQLRollOutcome>,
}

impl From<&RollResolution> for GraphQLRollResolution {
    fn from(resolution: &RollResolution) -> Self {
        let (result_kind, result_value) = match resolution.kind {
            ResolutionKind::Total(v) => (RollResultKind::Total, v),
            ResolutionKind::SuccessCount(n) => (RollResultKind::SuccessCount, n as f64),
        };
        GraphQLRollResolution {
            formula: resolution.formula.clone(),
            dice: resolution
                .dice
                .iter()
                .map(GraphQLDieOutcome::from)
                .collect(),
            result_kind,
            result_value,
            outcome: None,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLRollRecord {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub triggered_by: uuid::Uuid,
    pub resolution: GraphQLRollResolution,
    pub created_at: String,
}

impl From<RollRecord> for GraphQLRollRecord {
    fn from(row: RollRecord) -> Self {
        // `detail` is only ever written by `rollDice` immediately after a
        // successful `thunderforge_dice::resolve()` (data-model.md), so a
        // deserialization failure here would indicate a persisted-shape
        // bug, not caller input — falling back to an empty resolution
        // rather than panicking on a history read.
        let resolution: RollResolution =
            serde_json::from_value(row.detail.clone()).unwrap_or(RollResolution {
                formula: row.formula.clone(),
                dice: Vec::new(),
                kind: ResolutionKind::Total(row.result_value),
            });
        GraphQLRollRecord {
            id: row.id,
            world_id: row.world_id,
            triggered_by: row.triggered_by,
            resolution: GraphQLRollResolution {
                outcome: stored_outcome(&row),
                ..GraphQLRollResolution::from(&resolution)
            },
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
