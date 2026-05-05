//! Async mutation execution with HTTP transport - Phase 4.6
//!
//! Handles GraphQL mutations via gloo-net.
//! Uses spawn_local + mpsc channels to bridge async HTTP to ECS.

#![cfg(target_arch = "wasm32")]

use bevy::prelude::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use wasm_bindgen_futures::spawn_local;

use crate::sync_test::{CircularFlowTracer, FlowStage};

/// Mutation result from server
#[derive(Debug, Clone)]
pub struct MutationResponse {
    /// Unique mutation ID (for correlation)
    pub mutation_id: u64,
    /// Was the mutation successful
    pub success: bool,
    /// Server-provided error message
    pub error: Option<String>,
}

/// Resource for tracking active mutations
#[derive(Resource)]
pub struct MutationTracker {
    /// Atomic counter for unique mutation IDs
    next_id: Arc<AtomicU64>,
}

impl Default for MutationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MutationTracker {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Generate next mutation ID (thread-safe atomic)
    pub fn next_mutation_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// Execute moveToken mutation asynchronously
///
/// Phase 4.6: Spawns async HTTP task
/// Phase 4.6.1: Will wire result back to ECS via commands.trigger()
pub fn execute_move_token_mutation(
    tracker: &MutationTracker,
    server_url: &str,
    world_id: String,
    token_id: String,
    x: f32,
    y: f32,
    z: f32,
) -> u64 {
    let mutation_id = tracker.next_mutation_id();
    let server_url = format!("http://{}/graphql", server_url.replace("ws://", ""));

    eprintln!(
        "[Mutation📤] Queuing mutation id={}: move token {} to ({}, {}, {})",
        mutation_id, token_id, x, y, z
    );

    // Spawn async HTTP task
    spawn_local(async move {
        if let Err(e) = execute_move_mutation_async(
            mutation_id,
            server_url,
            world_id,
            token_id,
            x,
            y,
            z,
        )
        .await
        {
            eprintln!("[Mutation❌] Async error: {}", e);
        }
    });

    mutation_id
}

/// Async HTTP POST task for moveToken mutation
async fn execute_move_mutation_async(
    mutation_id: u64,
    server_url: String,
    world_id: String,
    token_id: String,
    x: f32,
    y: f32,
    z: f32,
) -> Result<(), String> {
    eprintln!("[Mutation🌐] HTTP POST to {}", server_url);

    let mutation_query = r#"
        mutation moveToken($input: MoveTokenInput!) {
            moveToken(input: $input) {
                id
                success
                error
            }
        }
    "#;

    let variables = json!({
        "input": {
            "worldId": world_id,
            "tokenId": token_id,
            "x": x,
            "y": y,
            "z": z
        }
    });

    let body = json!({
        "query": mutation_query,
        "variables": variables
    });

    eprintln!("[Mutation📝] Body: {}", body);

    // Phase 4.6.1: Implement actual HTTP via gloo-net
    eprintln!("[Mutation✅] Mutation {} queued (HTTP execution deferred to Phase 4.6.1)", mutation_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutation_id_generation() {
        let tracker = MutationTracker::new();
        let id1 = tracker.next_mutation_id();
        let id2 = tracker.next_mutation_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_mutation_tracker_creation() {
        let tracker = MutationTracker::new();
        let id = tracker.next_mutation_id();
        assert_eq!(id, 1);
    }
}
