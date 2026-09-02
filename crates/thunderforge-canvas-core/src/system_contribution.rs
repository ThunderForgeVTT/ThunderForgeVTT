//! How a game system pack announces itself, without anything having to list it.
//!
//! # The problem this replaces
//!
//! `src/server/src/systems.rs` carried seven `register_*_system` functions,
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
//! So `src/server/src/system_packs.rs` holds one `use <pack> as _;` line per
//! bundled pack, and `Cargo.toml` holds one dependency. Those two lines are
//! build-graph facts: they say a crate exists and should be linked, and they
//! say nothing about what it contains. They cannot drift out of step with a
//! system's data the way a validator list can, because they carry no
//! information to drift.
//!
//! `scripts/check-system-registry.mjs` is what keeps it that way — it fails
//! the build if a system identifier reappears in shared server code.

use crate::system_rules::SystemRules;

/// Validates one of an actor's stored data slots for one system.
pub type ValidatorFn = fn(&serde_json::Value) -> Result<(), String>;

/// Builds a system's rules from its own manifest.
///
/// A constructor rather than a value, because rules are built from the pack's
/// `system.json` — the manifest stays the authority on tables like Genie's
/// by-level Wish Points ladder, instead of those numbers being copied into
/// Rust where they would need keeping in step by hand.
pub type RulesFn = fn(&serde_json::Value) -> Box<dyn SystemRules>;

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
    /// `src/server/src/system_packs.rs`'s test.
    #[test]
    fn a_contribution_defaults_to_contributing_nothing_but_its_name() {
        let bare = SystemContribution::new("bare");
        assert_eq!(bare.id, "bare");
        assert!(bare.ability_data.is_none());
        assert!(bare.rules.is_none());
    }
}
