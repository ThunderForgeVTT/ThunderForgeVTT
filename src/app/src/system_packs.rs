//! The one place that says which system packs are linked into this build.
//!
//! # Why this file has to exist, and why it is only imports
//!
//! Spec 032's FR-029 says a system pack's contribution must be *discovered*
//! rather than listed, and `thunderforge_canvas_core::system_contribution`
//! delivers that: each pack submits what it contributes, and nothing collects
//! them by name.
//!
//! There is one thing that cannot be discovered, and pretending otherwise
//! would have produced a feature that silently registered nothing. A
//! statically linked Rust crate that nothing references is never linked at
//! all, and its submissions go with it. That was measured rather than assumed:
//! a binary depending on a submitting crate without naming a single symbol
//! from it collected an **empty** set, in debug and release alike; adding one
//! `use pack as _;` collected everything.
//!
//! So the `use` lines below are load-bearing, and deleting one silently
//! removes a game system from the product. They are also the *only* thing
//! shared server code says about these packs. Compare what they replaced:
//! seven `register_*_system` functions, each naming a system id as a string
//! and wiring five validator function pointers by hand, plus a `GAME_SYSTEMS`
//! initialiser with a `// In future phases: register_coc7e_system(...)`
//! comment already waiting to be the eighth.
//!
//! A line here carries no information about the system it names — not its
//! data shapes, not its validators, not its rules — so unlike a validator
//! list it has nothing that can drift out of step with the pack. Adding a
//! system is now one line here and one dependency in `Cargo.toml`, both of
//! them build-graph facts.
//!
//! `scripts/check-system-registry.mjs` fails the build if a system identifier
//! reappears anywhere else in shared server code.

// Linked for their `inventory::submit!` blocks alone. Nothing here calls into
// them, and nothing should: the moment shared code names a pack's function,
// this file stops being a build-graph fact and starts being a registry again.
use blades_server as _;
use cypher_server as _;
use dnd5e_server as _;
use fate_server as _;
use genie_server as _;
use pathfinder2e_server as _;
use yze_server as _;

#[cfg(test)]
mod tests {
    use thunderforge_canvas_core::system_contribution::{contribution_for, contributions};

    /// Every bundled pack arrives, and arrives *through discovery* — this
    /// module names no system id, and neither does the collector.
    ///
    /// The list here is the assertion, not the mechanism. If a `use` line
    /// above is deleted, this fails with the missing system named, which is
    /// the failure mode the old hand-written registry could not produce: a
    /// forgotten `register_*_system` call simply meant a system quietly did
    /// not exist.
    #[test]
    fn every_bundled_pack_is_discovered() {
        for id in [
            "blades_in_the_dark",
            "cypher_system",
            "dnd5e",
            "fate_core",
            "genie",
            "pathfinder2e",
            "year_zero_engine",
        ] {
            assert!(
                contribution_for(id).is_some(),
                "{id} contributed nothing — is its `use` line in system_packs.rs still there?"
            );
        }
    }

    /// Two packs claiming one identity must be caught, not resolved by
    /// whichever the linker happened to emit first.
    #[test]
    fn no_two_packs_claim_the_same_identity() {
        let mut seen: Vec<&str> = contributions().map(|c| c.id).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "two packs contributed the same id");
    }

    /// Genie is the system that actually derives something today, and this is
    /// the end of the wire that proves a pack's rules reach the server.
    #[test]
    fn genie_contributes_rules_and_the_others_do_not_yet() {
        let genie = contribution_for("genie").expect("genie is bundled");
        assert!(
            genie.rules.is_some(),
            "genie derives its by-level Wish Points"
        );
        assert!(
            contribution_for("fate_core").is_some_and(|c| c.rules.is_none()),
            "a pack that computes nothing declares no rules, which is a fact \
             about the ruleset rather than an omission"
        );
    }

    /// A pack's world-creation hook reaches the registry, in the binary where
    /// the linkage is the real one.
    ///
    /// This cannot be asserted in `thunderforge-server`'s own tests:
    /// `inventory` collects into one compiled crate instance, and `cargo test`
    /// builds that library a second time under `cfg(test)` while the packs
    /// were built against the first. Here there is one of everything.
    ///
    /// It matters because a hook that is never collected looks exactly like a
    /// system that contributes nothing — world creation succeeds, quietly
    /// missing whatever the pack meant to do.
    #[test]
    fn a_pack_contributes_a_world_creation_hook() {
        let hooks: Vec<&str> = thunderforge_server::world_hooks::hooks()
            .map(|hook| hook.system_id)
            .collect();

        assert!(
            !hooks.is_empty(),
            "no world-creation hook was collected; if the `use <pack> as _;` \
             lines above are gone, every pack's hooks vanish with them"
        );
    }

    /// Every contributed hook belongs to a pack this build actually has.
    ///
    /// A hook for a system with no manifest on disk would run against worlds
    /// nobody can create, which is dead code that looks live.
    #[test]
    fn every_hook_belongs_to_a_bundled_system() {
        let installed: Vec<String> = std::fs::read_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/systems"),
        )
        .expect("packs/systems must exist")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

        for hook in thunderforge_server::world_hooks::hooks() {
            assert!(
                installed.iter().any(|id| id == hook.system_id),
                "a hook claims {:?}, which is not a pack in packs/systems",
                hook.system_id
            );
        }
    }
}
