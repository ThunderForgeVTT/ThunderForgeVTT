//! How a game system pack announces itself, without anything having to list it.
//!
//! # The problem this replaces
//!
//! `crates/thunderforge-server/src/systems.rs` carried seven `register_*_system` functions,
//! each naming a system id as a string literal and wiring five validator
//! function pointers by hand, all called from one `GAME_SYSTEMS` initialiser
//! that already had a `// In future phases: register_coc7e_system(...)`
//! comment waiting to be the eighth. Adding a system meant editing shared
//! server code that had to know the system's name and the shape of its data.
//!
//! That is the thing spec 032's SC-004 measures, and the thing that makes the
//! eighth pack cost as much as the first.
//!
//! # What is discovered, and what cannot be
//!
//! A pack submits its contribution here and the server collects them. Nothing
//! in shared code names a system, wires a validator, or matches on an id.
//!
//! One thing is **not** discovered, and the honest version of this decision
//! says so: a statically linked Rust crate that nothing references is never
//! linked at all, and its submissions vanish with it. Measured, not assumed —
//! a binary depending on a submitting crate without naming any symbol from it
//! collected an empty set, in debug and release alike; adding `use pack as _;`
//! collected everything.
//!
//! So `crates/thunderforge-server/src/system_packs.rs` holds one `use <pack> as _;` line per
//! bundled pack, and `Cargo.toml` holds one dependency. Those two lines are
//! build-graph facts: they say a crate exists and should be linked, and they
//! say nothing about what it contains. They cannot drift out of step with a
//! system's data the way a validator list can, because they carry no
//! information to drift.
//!
//! `scripts/check-system-registry.mjs` is what keeps it that way — it fails
//! the build if a system identifier reappears in shared server code.

use crate::content_entry::{Entry as ContentEntry, SourceLine as ContentSourceLine};
use crate::roll_facets::RollFacets;
use crate::system_rules::SystemRules;

/// Validates one of an actor's stored data slots for one system.
pub type ValidatorFn = fn(&serde_json::Value) -> Result<(), String>;

/// Builds a system's rules from its own manifest.
///
/// A constructor rather than a value, because rules are built from the pack's
/// `system.json` — the manifest stays the authority on tables like Genie's
/// by-level Wish Points ladder, instead of those numbers being copied into
/// Rust where they would need keeping in step by hand.
/// Refines one entry read out of a document. See
/// [`SystemContribution::refine_content`].
pub type ContentRefineFn = fn(&mut ContentEntry, &[ContentSourceLine]);

/// Checks one world setting beyond what its declaration can say.
///
/// Receives the setting's key and the value a Game Master submitted, after
/// the manifest's own declaration has already allowed it. A refusal is shown
/// to that Game Master, so it is written for them.
pub type SettingValidatorFn = fn(&str, &serde_json::Value) -> Result<(), String>;

pub type RulesFn = fn(&serde_json::Value) -> Box<dyn SystemRules>;

/// How a roll came out, in the host's words.
///
/// A closed list, because the host draws and stores these and a pack's own
/// word for one would be a word nothing else could read. What the pack calls
/// the result is [`RollOutcome::label`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Success,
    Failure,
    Tie,
    CriticalSuccess,
    CriticalFailure,
}

/// What was rolled, as an adjudicator sees it.
#[derive(Debug, Clone, Copy)]
pub struct RollFacts<'a> {
    /// The check's id: a manifest `checks` entry, or a pack's own name for a
    /// roll its own mutation makes.
    pub check: &'a str,
    /// The final value of every die that counted, in the order rolled.
    pub dice: &'a [i64],
    /// What the formula came to.
    pub total: f64,
}

/// A roll, judged. Stored with the roll and shown to the table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RollOutcome {
    pub verdict: Verdict,
    /// The verdict in the system's own words.
    pub label: String,
}

/// Judges a roll (spec 067 FR-032).
///
/// Pure, as `derive` is: the roll and a context value in, a verdict out, and
/// no database. For a manifest check the context is the world's effective
/// system settings, keyed by setting id. A pack whose roll needs more than
/// that — a target, a modifier — gathers it in its own mutation and passes
/// its own context to this same function.
///
/// `None` is "not judged": there was nothing to judge the roll against. It
/// is not a failure, and the host stores no outcome for it.
pub type AdjudicatorFn = fn(&RollFacts<'_>, &serde_json::Value) -> Option<RollOutcome>;

/// Everything one game system pack contributes.
///
/// Every field beyond `id` is optional because the systems genuinely differ:
/// Genie has no spellcasting and therefore no `spell_data`, Fate Core declares
/// no abilities at all, and a pack that computes nothing has no `rules`.
/// Absence here is a fact about the ruleset, not an omission to be filled in.
pub struct SystemContribution {
    /// Matches the pack's manifest `id`.
    pub id: &'static str,
    pub ability_data: Option<ValidatorFn>,
    pub resource_data: Option<ValidatorFn>,
    pub proficiency_data: Option<ValidatorFn>,
    pub trait_data: Option<ValidatorFn>,
    pub spell_data: Option<ValidatorFn>,
    /// The system's derived values, when it has any.
    pub rules: Option<RulesFn>,
    /// Refines a generically-read entry with what only this system knows
    /// (spec 049 FR-016, ADR-096).
    ///
    /// A content pattern says which labelled fields to read, and that covers
    /// most of a book. It cannot express a reading of free prose — a 5e
    /// attack's reach in feet is not a labelled field, and spec 045
    /// established that reach is per attack rather than per creature size, so
    /// it matters and cannot be dropped.
    ///
    /// Rather than pretend such a thing into the declaration or keep a second
    /// reader beside the generic one, a pack contributes this. It receives
    /// what the shared reader produced and the lines it came from, and may
    /// fill in [`ContentEntry::extras`].
    pub refine_content: Option<ContentRefineFn>,
    /// A further check on a world setting (spec 067 FR-007).
    ///
    /// A manifest declares a setting's type, bounds and options, and for most
    /// settings that is the whole rule. This is for the rest: a value that is
    /// well-typed and still not one this ruleset can be played with.
    pub world_setting: Option<SettingValidatorFn>,
    /// Judges this system's rolls. A system without one has rolls that are
    /// rolled and recorded and never judged, which is every system before
    /// spec 067.
    pub adjudicate: Option<AdjudicatorFn>,
    /// Shapes this system's rolls and spends its rerolls (spec 084). A system
    /// without one rolls every formula as written and offers no reroll.
    pub roll_facets: Option<&'static RollFacets>,
}

impl SystemContribution {
    /// A contribution that validates nothing and derives nothing.
    ///
    /// Exists so a pack can fill in only the slots it has, with `..` doing the
    /// rest, rather than writing five `None`s to say five true things.
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            ability_data: None,
            resource_data: None,
            proficiency_data: None,
            trait_data: None,
            spell_data: None,
            rules: None,
            refine_content: None,
            world_setting: None,
            adjudicate: None,
            roll_facets: None,
        }
    }
}

inventory::collect!(SystemContribution);

/// Every contribution linked into this binary.
pub fn contributions() -> impl Iterator<Item = &'static SystemContribution> {
    inventory::iter::<SystemContribution>.into_iter()
}

/// The one contributed by `id`, if this build has it.
pub fn contribution_for(id: &str) -> Option<&'static SystemContribution> {
    contributions().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collection works at all in this crate's own build.
    ///
    /// Deliberately thin: what matters is that a *pack* crate's submission
    /// arrives, and only a binary linking one can show that. That is
    /// `crates/thunderforge-server/src/system_packs.rs`'s test.
    #[test]
    fn a_contribution_defaults_to_contributing_nothing_but_its_name() {
        let bare = SystemContribution::new("bare");
        assert_eq!(bare.id, "bare");
        assert!(bare.ability_data.is_none());
        assert!(bare.rules.is_none());
        assert!(bare.roll_facets.is_none());
    }
}
