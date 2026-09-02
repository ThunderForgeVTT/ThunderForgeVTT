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

pub mod background;
pub mod lighting;
pub mod optimistic;
pub mod selection;
pub mod shape;
pub mod sync;
pub mod token;
pub mod token_grid;
pub mod token_loader;
pub mod token_move;
pub mod wall;

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
