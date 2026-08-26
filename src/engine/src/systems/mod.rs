//! Bevy ECS systems for game logic and synchronization.
//!
//! F2: System Registration
//! - core: GameSystem trait and SystemRegistry for extensible game system loading
//! - builtin: Built-in game systems (BasicSystem, future: DnD5e, Pathfinder, etc)

pub mod background;
pub mod builtin;
pub mod core;
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

pub use builtin::BasicSystem;
pub use core::{DerivedStats, GameSystem, SkillDefinition, SystemRegistry};
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
