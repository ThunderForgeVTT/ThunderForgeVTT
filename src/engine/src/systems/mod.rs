//! Bevy ECS systems for game logic and synchronization.

pub mod sync;
pub mod optimistic;

pub use sync::{process_server_responses, handle_mutation_errors};
pub use optimistic::{
    process_mutation_results,
    mark_mutation_pending,
    PendingMutation,
};


