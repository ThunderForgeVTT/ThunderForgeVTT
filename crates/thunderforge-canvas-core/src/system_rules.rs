//! The one contract a game system implements to say what a character has.
//!
//! # Why this is here and not in the engine
//!
//! It was in the engine, and it was also in a pack, and the two had drifted.
//! `src/engine/src/systems/core.rs` declared a `GameSystem` trait;
//! `packs/systems/dnd5e/engine/src/plugin.rs` declared a `GameSystemTrait`
//! carrying the comment "should match the one in
//! `src/engine/src/systems/core.rs` — Re-defined here to avoid cross-package
//! dependency". By the time both were read together they shared exactly one
//! method name, and nothing depended on either.
//!
//! The duplication had a stated cause, and this crate is the cure for it:
//! `src/engine/Cargo.toml` and `src/server/Cargo.toml` both already depend on
//! `thunderforge_canvas_core`, and it is the only crate both of them have. It
//! is also where this codebase has twice put rules of exactly this kind, for
//! the same reason each time — its tests execute natively, and the engine
//! crate's compile under wasm32 and never run.
//!
//! # Why there is no `armor_class` here
//!
//! The trait this replaces returned `DerivedStats { effective_health,
//! armor_class, initiative, proficiency_bonus }`. That is one ruleset's
//! character sheet compiled into a contract. Blades in the Dark's resources
//! are stress, trauma and coin; Fate Core declares no abilities at all;
//! Genie has Wish Points. `armor_class` means nothing to any of them.
//!
//! This codebase has made that mistake twice and corrected it twice — see
//! [`crate::attributes`] on the fixed `TokenAbilities` struct that stored six
//! `None`s for a Genie character, and [`crate::resource_display`] on the same
//! correction for resources. A third fixed struct would be a regression with
//! two precedents against it. So the contract carries `identifier -> value`
//! pairs and names no system's concepts in its own types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::attributes::AttributeDeclaration;

/// Whether a value was read from stored data or computed from it.
///
/// # Why a surface needs to know
///
/// So it can tell a player which numbers they may edit. A 5e Strength score
/// is typed in; its modifier is not. A text box over a computed number invites
/// the two to disagree, and the stored one is the one that goes stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub enum Origin {
    /// Read from the actor's stored slot, against the system's manifest.
    Stored,
    /// Computed by the system's own rules, and never written down.
    Derived,
}

/// What a declared value can be.
///
/// Deliberately small. Every variant here is something at least one shipping
/// manifest already needs: scores and modifiers are integers, a Fate ladder
/// rung is text, a proficiency is a boolean, a condition set is a list. There
/// is no variant for a nested object, because a value a surface cannot render
/// without knowing what it means is a value this contract should not carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub enum DeclaredValueKind {
    Integer(i32),
    Number(f64),
    Text(String),
    Boolean(bool),
    List(Vec<String>),
    /// A pool: a current value and, when the system gives one, a maximum.
    ///
    /// # Why this is a variant and not two values
    ///
    /// A bar is a proportion, and a proportion needs both halves *together*.
    /// Publishing a resource as one rendered string — "4 / 7" — forced the
    /// only consumer that draws bars to parse it back apart, which is exactly
    /// the branching-on-what-a-value-means this contract exists to prevent: a
    /// system writing "4 of 7" would have silently lost its bar. Found by
    /// building the renderer against the format, and recorded as spec 032's
    /// T019a before it was fixed.
    ///
    /// `max` is absent for a counter rather than zero. A pool with no maximum
    /// is not a pool that is empty — Blades in the Dark's coin counts up with
    /// nothing to be a proportion of — and this is the same distinction
    /// [`crate::resource_display::ResourceEntry`] already draws.
    Fraction {
        current: i32,
        max: Option<i32>,
    },
    /// A bounded run of marks, and how many are filled.
    ///
    /// Fate Core's stress is eight of these in one track; 5e's death saves are
    /// two separate runs of three meaning opposite things, which is why a
    /// track carries no notion of rows — two tracks is what two tracks are.
    ///
    /// Distinct from [`Self::Fraction`], which it superficially resembles. A
    /// pool is a quantity with a maximum and the numbers are the point; a
    /// track is a set of marks and the *count* is the point. Drawing one as
    /// the other gives a bar where a player expects boxes to tick.
    Track {
        filled: u32,
        of: u32,
    },
    /// An ordered set of named states, of which one is current.
    ///
    /// Cypher's damage track — hale, impaired, debilitated, dead — has no
    /// marks to count at all, which is why this is a separate kind rather than
    /// a track with labels. A state set is a position on a ladder.
    ///
    /// `current` absent means none of them, which is a real answer: an
    /// uninjured character is at no position on a damage track.
    State {
        current: Option<String>,
        /// In the system's own order, worst last.
        options: Vec<String>,
    },
}

impl DeclaredValueKind {
    /// The integer inside, when there is one.
    ///
    /// A whole-numbered float counts, because a system storing `14.0` means
    /// fourteen. A fractional one does not: rounding it would invent precision
    /// the sheet does not have, which is the rule [`crate::attributes`]
    /// already applies when reading stored scores.
    pub fn as_integer(&self) -> Option<i32> {
        match self {
            Self::Integer(value) => Some(*value),
            Self::Number(value) if value.fract() == 0.0 => i32::try_from(*value as i64).ok(),
            // A pool's integer is its current value; the maximum is a
            // separate fact and a caller asking for "the number" means this
            // one.
            Self::Fraction { current, .. } => Some(*current),
            // A track's integer is how many marks are filled. A state set has
            // no number at all — asking for one is a category error, and
            // returning its index would invent an arithmetic the system never
            // declared.
            Self::Track { filled, .. } => i32::try_from(*filled).ok(),
            _ => None,
        }
    }
}

/// One value a system publishes about an actor.
///
/// The unit everything downstream carries and nothing downstream interprets.
/// A sheet lays these out, the canvas draws bars from some of them, and
/// neither knows what any of them mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct DeclaredValue {
    /// The system's own identifier — `strength`, `strengthMod`, `wishPoints`.
    pub id: String,
    /// What a person is shown.
    pub label: String,
    /// Short form for tight layouts, where the system offers one.
    pub abbreviation: Option<String>,
    pub value: DeclaredValueKind,
    /// The group this belongs to, when it is part of one (FR-033).
    ///
    /// A Fate consequence is a severity *and* the aspect written into it; a
    /// Cypher stat is a current value, a pool and an edge. Publishing those as
    /// unrelated identifiers loses the fact that they are one thing, which is
    /// what a sheet shows and a player reads.
    ///
    /// # Why a field rather than nesting
    ///
    /// Because `DeclaredValue` is flat and everything downstream relies on it:
    /// the resolver, the wire type, the layout renderer and every test all
    /// walk a list of values with an id. Nesting would change all of them to
    /// gain one relationship. A group id names the relationship and leaves the
    /// list a list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The group's own name, when its system gave it one (T019g).
    ///
    /// Without this a group was named after whichever member happened to be
    /// declared first, which is a guess that was right by luck: Cypher's
    /// `might` group leads with `might`, so it read "Might". Reorder the
    /// manifest and the same group reads "Might Edge". A Fate consequence
    /// group wants to read "Mild Consequence" whatever its parts are called.
    ///
    /// Repeated on every member of the group, and that repetition cannot
    /// drift: the manifest declares a group **once**, in its own `groups`
    /// block, and the server stamps the answer onto each value. Nothing
    /// hand-writes it twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_label: Option<String>,
    /// This is the member to show when there is room for one (T019g).
    ///
    /// A group is several values and a compact layout has space for one of
    /// them. Which one is a fact about the ruleset — a Cypher stat shows its
    /// current value, not its edge — and nothing in the format could say it,
    /// so the renderer took the first and hoped.
    ///
    /// False for a value in no group, and false for every member when the
    /// system named no headline; a renderer with no headline falls back to
    /// the first member, which is where this started.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub headline: bool,
    pub origin: Origin,
}

/// Everything already known about one actor, as a lookup.
///
/// Handed to [`SystemRules::derive`] so a rule can read the scores it needs
/// without being handed the database.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeclaredValues {
    by_id: BTreeMap<String, DeclaredValue>,
}

impl DeclaredValues {
    pub fn new(values: impl IntoIterator<Item = DeclaredValue>) -> Self {
        Self {
            by_id: values.into_iter().map(|v| (v.id.clone(), v)).collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&DeclaredValue> {
        self.by_id.get(id)
    }

    /// The integer stored under `id`, when there is one.
    ///
    /// Returns `None` for a value that is absent as well as one that is not a
    /// number, and the caller must treat those the same way: a declaration the
    /// actor stores nothing for is **omitted, not defaulted**. A zero is a
    /// statement — a real and crippling score in every system shipping here —
    /// whereas an unfilled sheet is the absence of one.
    pub fn integer(&self, id: &str) -> Option<i32> {
        self.by_id.get(id)?.value.as_integer()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &DeclaredValue> {
        self.by_id.values()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// What a game system contributes: the values it computes from the ones it stores.
///
/// A system's *stored* values need no code — its manifest already declares
/// them and [`crate::attributes`] already reads them. This trait is only for
/// the half a manifest cannot express: the numbers that exist because
/// something else does.
///
/// # `derive` must be pure
///
/// No database, no network, no clock, no randomness.
///
/// A derived value is recomputed every time an actor is read, on every
/// viewer's behalf, and is **never written down** — because a derived value
/// that is also stored is two values that can disagree, and the stored one is
/// the one that goes stale. If `derive` were impure, the same character would
/// render differently to two people at the same table and neither would be
/// wrong.
///
/// Purity is also what lets a rule be tested without a database, which is the
/// whole reason this contract lives in a natively-tested crate.
pub trait SystemRules: Send + Sync {
    /// The system this implements, matching its manifest `id`.
    fn id(&self) -> &str;

    /// The identifiers [`Self::derive`] may return, declared up front.
    ///
    /// Stated separately from `derive` because an interface pack has to be
    /// validated against a system *without running it* (spec 032 FR-026): a
    /// pack laying out `strengthMod` is checked against this list, not against
    /// the output of a function nobody has any actor to call it on.
    fn derived_declarations(&self) -> Vec<AttributeDeclaration>;

    /// Values this system computes, given everything already read.
    ///
    /// Returns only what it adds. A rule that cannot compute a value — the
    /// score it depends on is absent — omits it rather than returning a zero,
    /// for the same reason stored values are omitted rather than defaulted.
    fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue>;
}

/// Merge a system's stored and derived values into the one set everything reads.
///
/// # Why `visible` and `context` are two arguments
///
/// What a rule may *read* is not what a sheet *shows*.
///
/// Genie's by-level Wish Points rule needs the character's `level`, which
/// lives in the actor's trait slot and is not one of the three attributes
/// Genie declares. Handing `derive` only the displayed attributes would make
/// that rule uncomputable; adding `level` to the displayed set to feed it
/// would put a field on the sheet to satisfy a function, which is the tail
/// wagging the dog.
///
/// So `context` is everything legible about the actor, and `visible` is the
/// subset a surface presents. `context` should contain `visible`; nothing
/// breaks if it does not, the rule simply sees less.
///
/// # Why undeclared derivations are dropped
///
/// A `derive` returning an identifier absent from `derived_declarations` is a
/// bug in the pack, and this treats it as one: the value is dropped rather
/// than rendered. Rendering it would put a number on a sheet that no interface
/// pack could ever have been validated against — the pack's layout was checked
/// against the declarations, so a value outside them has no declared place to
/// appear and no label anyone approved.
///
/// Dropped silently at this layer and reported by the test in this module,
/// because the failure is a build-time mistake in a bundled pack, not
/// something a player at a table can act on.
pub fn resolve(
    rules: Option<&dyn SystemRules>,
    visible: Vec<DeclaredValue>,
    context: &DeclaredValues,
) -> Vec<DeclaredValue> {
    // Deduplicated by identifier, **in the order the system declared them**.
    //
    // This used to funnel `visible` through `DeclaredValues`, which is a
    // `BTreeMap` keyed by id — so it deduplicated correctly and alphabetised
    // everything on the way past. Genie declares might, cunning, spirit and a
    // sheet showed cunning, might, spirit; 5e declares walk, fly, swim, climb
    // and a sheet showed climb first. Every layer above this one is careful to
    // say that the system's order is the system's own and a pack never
    // reorders a set, and the order had already been lost before any of them
    // saw it.
    let mut seen = std::collections::BTreeSet::new();
    let mut out: Vec<DeclaredValue> = visible
        .into_iter()
        // First declaration wins, matching `indexById` on the other side of
        // the wire: a value listed twice is one value, and the earlier is the
        // one the system reached for first.
        .filter(|value| seen.insert(value.id.clone()))
        .collect();

    let Some(rules) = rules else {
        return out;
    };

    let permitted: BTreeMap<String, AttributeDeclaration> = rules
        .derived_declarations()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    let mut derived: Vec<DeclaredValue> = rules
        .derive(context)
        .into_iter()
        // A derived value must be declared, and must not shadow anything the
        // actor stores: if a system both stores and computes one identifier,
        // the stored value is the one a player typed in and the computed one
        // is the disagreement this contract exists to prevent. Checked against
        // `context` rather than `visible`, so a stored field that happens not
        // to be on the sheet still wins.
        .filter(|value| permitted.contains_key(&value.id) && !context.contains(&value.id))
        .collect();
    derived.sort_by_key(|value| {
        permitted
            .get(&value.id)
            .map(|d| d.order)
            .unwrap_or(usize::MAX)
    });

    out.append(&mut derived);
    out
}

#[cfg(test)]
#[path = "system_rules_tests.rs"]
mod tests;
