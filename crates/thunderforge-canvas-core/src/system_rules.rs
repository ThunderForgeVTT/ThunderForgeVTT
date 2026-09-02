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
/// # Why the filter exists
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
pub fn resolve(rules: Option<&dyn SystemRules>, stored: Vec<DeclaredValue>) -> Vec<DeclaredValue> {
    let stored = DeclaredValues::new(stored);
    let Some(rules) = rules else {
        return stored.iter().cloned().collect();
    };

    let permitted: BTreeMap<String, AttributeDeclaration> = rules
        .derived_declarations()
        .into_iter()
        .map(|d| (d.id.clone(), d))
        .collect();

    let mut out: Vec<DeclaredValue> = stored.iter().cloned().collect();
    let mut derived: Vec<DeclaredValue> = rules
        .derive(&stored)
        .into_iter()
        // A derived value must be declared, and must not shadow a stored one:
        // if a system both stores and computes the same identifier, the stored
        // value is the one a player typed in and the computed one is the
        // disagreement this contract exists to prevent.
        .filter(|value| permitted.contains_key(&value.id) && !stored.contains(&value.id))
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
