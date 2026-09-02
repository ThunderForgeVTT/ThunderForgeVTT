//! Bevy ECS systems for game logic and synchronization.
//!
//! # Where the game-system contract went
//!
//! This module used to carry `core::GameSystem` — a trait with one stub
//! implementation, `BasicSystem`, registered into a `SystemRegistry` that a
//! startup plugin inserted as a resource **nothing ever read**. Its
//! `DerivedStats` return type had fixed fields for armour class, initiative
//! and proficiency bonus, which is one ruleset's character sheet compiled
//! into a renderer: it had nowhere to put Blades in the Dark's stress and
//! trauma, and nothing to say to Fate Core, which declares no abilities at
//! all.
//!
//! The contract every system implements is now
//! `thunderforge_canvas_core::system_rules::SystemRules`, stated once, in the
//! only crate both the engine and the server already depend on. It carries
//! declared `identifier -> value` pairs and names no system's concepts. See
//! ADR-060.

// # These five were files, not modules
//
// `conflict_visualization`, `event_dispatcher`, `mutation_sender`, `presence`
// and `token_sync_d2` were never declared anywhere in the crate. They carried
// `#![cfg(target_arch = "wasm32")]`, which made them look browser-only, but
// nothing declared them on *any* target — so they compiled nowhere, and the
// ~45 tests in `tests_f1_unit.rs` and `tests_f2_f4_integration.rs` that
// exercise them were unreachable text. Declared here so they are part of the
// crate, and so their tests can run (spec 032 T083).
pub mod background;
pub mod conflict_visualization;
pub mod event_dispatcher;
pub mod lighting;
pub mod mutation_sender;
pub mod optimistic;
pub mod presence;
pub mod selection;
pub mod shape;
pub mod sync;
pub mod token;
pub mod token_grid;
pub mod token_loader;
pub mod token_move;
pub mod token_sync_d2;
pub mod wall;

#[cfg(test)]
mod tests_f1_unit;
#[cfg(test)]
mod tests_f2_f4_integration;

pub use optimistic::{PendingMutation, mark_mutation_pending, process_mutation_results};
pub use sync::{handle_mutation_errors, process_server_responses};
pub use token_loader::{TokenCache, load_test_tokens};

// Stub for Phase 4.7 compatibility
#[derive(bevy::prelude::Event, Clone, Debug)]
pub struct MutationRejected {
    pub entity: bevy::prelude::Entity,
    pub mutation_id: u64,
    pub error: Option<String>,
}
