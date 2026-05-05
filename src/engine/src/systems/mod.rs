//! Bevy ECS systems for game logic and synchronization.
//! 
//! F2: System Registration
//! - core: GameSystem trait and SystemRegistry for extensible game system loading
//! - builtin: Built-in game systems (BasicSystem, future: DnD5e, Pathfinder, etc)

pub mod core;
pub mod builtin;
pub mod sync;
pub mod optimistic;
pub mod token_loader;
pub mod token_sync;
pub mod selection;

pub use core::{GameSystem, SkillDefinition, DerivedStats, SystemRegistry};
pub use builtin::BasicSystem;
pub use sync::{process_server_responses, handle_mutation_errors};
pub use optimistic::{
    process_mutation_results,
    mark_mutation_pending,
    PendingMutation,
};
pub use token_loader::{TokenCache, load_test_tokens};
pub use token_sync::{TokenJson, ColorJson, sync_tokens_from_rxdb, set_pending_tokens};
pub use selection::{handle_token_selection, render_selection_feedback, handle_keyboard_token_movement};

// Stub for Phase 4.7 compatibility
#[derive(bevy::prelude::Event, Clone, Debug)]
pub struct MutationRejected {
    pub entity: bevy::prelude::Entity,
    pub mutation_id: u64,
    pub error: Option<String>,
}
