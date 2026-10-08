//! A roll, thrown on the board (spec 083).
//!
//! The server has already decided every value. This module decides only how
//! the decided values are shown: which solid each die is ([`shapes`]), how it
//! comes to rest with the server's face toward the viewer ([`landing`]), the
//! seeded path it takes there ([`tumble`]), the line of arithmetic under it
//! ([`readout`]), and which throw plays when several arrive at once
//! ([`queue`]).
//!
//! It lives here rather than in the engine because the engine cannot compile
//! for the host, so a `#[test]` there never runs (research R12). The engine's
//! `plugins/dice/` draws what this module computes.

pub mod landing;
pub mod queue;
pub mod readout;
pub mod shapes;
pub mod tumble;

use thunderforge_dice::{
    Breakdown, ChainStep, DieSides, PlaceholderBindings, RollResolution, breakdown,
};

/// How long each part of a throw takes, in milliseconds (research R9). The
/// engine exports these as `dice_timings()`, and the web's roll panel waits
/// on them, so there is one owner (FR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// The first tumble, from the bottom edge to rest.
    pub tumble_ms: u32,
    /// Each reroll or explosion after the first value lands.
    pub step_ms: u32,
    /// How long the landed throw and its readout hold.
    pub hold_ms: u32,
    /// The fade out.
    pub fade_ms: u32,
    /// The fade in, under reduced motion, in place of the tumble.
    pub reduced_ms: u32,
}

pub const TIMINGS: Timings = Timings {
    tumble_ms: 1200,
    step_ms: 500,
    hold_ms: 2500,
    fade_ms: 400,
    reduced_ms: 150,
};

/// At most this many dice are drawn; the rest are counted in a chip.
/// Explosions count toward it.
pub const MAX_DRAWN: usize = 20;

/// A roll as the engine was handed it, in the dice crate's own types.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowSpec {
    pub roll_id: String,
    pub roller: String,
    pub label: Option<String>,
    /// The numbers the server put in for the formula's placeholders.
    pub bindings: PlaceholderBindings,
    /// The formula, every die, and the server's result.
    pub resolution: RollResolution,
}

/// The solid a die is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    D4,
    D6,
    D8,
    D10,
    D12,
    D20,
    /// Two ten-sided dice, tens and units.
    D100,
    /// A cube labelled 1, 2, 3 twice.
    D3,
    /// A cube with `+`, blank and `-` twice.
    Fate,
    /// A disc with `H` and `T`.
    Coin,
    /// A disc with `1` and `2`.
    D2,
    /// Any other size: a disc showing the landed value.
    Disc(u32),
}

impl ShapeKind {
    pub fn of(sides: DieSides) -> Self {
        match sides {
            DieSides::Fate => Self::Fate,
            DieSides::Coin => Self::Coin,
            DieSides::Numeric(4) => Self::D4,
            DieSides::Numeric(6) => Self::D6,
            DieSides::Numeric(8) => Self::D8,
            DieSides::Numeric(10) => Self::D10,
            DieSides::Numeric(12) => Self::D12,
            DieSides::Numeric(20) => Self::D20,
            DieSides::Numeric(100) => Self::D100,
            DieSides::Numeric(3) => Self::D3,
            DieSides::Numeric(2) => Self::D2,
            DieSides::Numeric(n) => Self::Disc(n),
        }
    }
}

/// One value a drawn die comes to show, in turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub value: i64,
    /// Struck through once the next segment starts: a reroll, or the value a
    /// clamp replaced.
    pub struck: bool,
    /// Whether the die tumbles to this value. A clamp swaps the number in
    /// place.
    pub tumble: bool,
    /// When it starts, from the throw's start.
    pub starts_ms: u32,
    /// When it shows, from the throw's start.
    pub lands_ms: u32,
}

/// One die as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowDie {
    pub shape: ShapeKind,
    /// The index of its source `DieOutcome` in the resolution.
    pub outcome: usize,
    pub segments: Vec<Segment>,
    /// For a die added by an explosion, the drawn index of the die whose
    /// chain it continues.
    pub explosion_of: Option<usize>,
    /// `false` when keep/drop dropped it: it is dimmed.
    pub kept: bool,
    /// For a success-count roll, whether it succeeded: `false` is dimmed.
    pub succeeded: Option<bool>,
}

impl ThrowDie {
    /// The value it rests on.
    pub fn face(&self) -> i64 {
        self.segments.last().map_or(0, |s| s.value)
    }

    /// When it enters the board.
    pub fn enters_ms(&self) -> u32 {
        self.segments.first().map_or(0, |s| s.starts_ms)
    }

    /// When it shows its final value.
    pub fn lands_ms(&self) -> u32 {
        self.segments.last().map_or(0, |s| s.lands_ms)
    }

    /// The values a reroll struck through, in order.
    pub fn rerolled(&self) -> Vec<i64> {
        self.segments
            .iter()
            .zip(self.segments.iter().skip(1))
            .filter(|(_, next)| next.tumble)
            .map(|(s, _)| s.value)
            .collect()
    }

    /// The value a clamp struck through.
    pub fn clamped(&self) -> Option<i64> {
        let n = self.segments.len();
        (n >= 2 && !self.segments[n - 1].tumble).then(|| self.segments[n - 2].value)
    }
}

/// Expands a roll into the dice drawn for it, and how many past
/// [`MAX_DRAWN`] are left undrawn (data-model.md, "Expanding a
/// `DieOutcome`").
pub fn expand(spec: &ThrowSpec) -> (Vec<ThrowDie>, usize) {
    let marks = match breakdown(&spec.resolution.formula, &spec.resolution, &spec.bindings) {
        Some(Breakdown::Successes(marks)) => Some(marks),
        _ => None,
    };
    let t = TIMINGS;
    let mut drawn: Vec<ThrowDie> = Vec::new();
    let mut total = 0usize;

    for (outcome, die) in spec.resolution.dice.iter().enumerate() {
        let shape = ShapeKind::of(die.sides);
        let succeeded = marks.as_ref().and_then(|m| m.get(outcome).copied());
        let Some(&first) = die.rolls.first() else {
            continue;
        };
        let make = |value: i64, starts_ms: u32, lands_ms: u32| Segment {
            value,
            struck: false,
            tumble: true,
            starts_ms,
            lands_ms,
        };
        let mut chain = vec![ThrowDie {
            shape,
            outcome,
            segments: vec![make(first, 0, t.tumble_ms)],
            explosion_of: None,
            kept: die.kept,
            succeeded,
        }];
        let head = drawn.len();
        for (i, &value) in die.rolls.iter().enumerate().skip(1) {
            let step = die.steps.get(i - 1).copied().unwrap_or(ChainStep::Reroll);
            let starts = t.tumble_ms + (i as u32 - 1) * t.step_ms;
            let segment = make(value, starts, starts + t.step_ms);
            match step {
                ChainStep::Reroll => {
                    let current = chain.last_mut().expect("a chain has a die");
                    if let Some(last) = current.segments.last_mut() {
                        last.struck = true;
                    }
                    current.segments.push(segment);
                }
                ChainStep::Explode => chain.push(ThrowDie {
                    shape,
                    outcome,
                    segments: vec![segment],
                    explosion_of: Some(head),
                    kept: die.kept,
                    succeeded,
                }),
            }
        }
        let last = *die.rolls.last().expect("rolls is not empty");
        if die.final_value != last {
            let current = chain.last_mut().expect("a chain has a die");
            let at = current.lands_ms();
            if let Some(prev) = current.segments.last_mut() {
                prev.struck = true;
            }
            current.segments.push(Segment {
                value: die.final_value,
                struck: false,
                tumble: false,
                starts_ms: at,
                lands_ms: at,
            });
        }
        total += chain.len();
        for d in chain {
            if drawn.len() < MAX_DRAWN {
                drawn.push(d);
            }
        }
    }
    let hidden = total - drawn.len();
    (drawn, hidden)
}

/// When the throw's last drawn die shows its final value.
pub fn lands_ms(dice: &[ThrowDie]) -> u32 {
    dice.iter().map(ThrowDie::lands_ms).max().unwrap_or(0)
}

#[cfg(test)]
#[path = "expand_tests.rs"]
mod expand_tests;
