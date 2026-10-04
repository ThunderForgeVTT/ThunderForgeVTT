//! Phase 4.9.D.2.2: GraphQL Mutation Sender System
//!
//! Handles asynchronous sending of queued mutations to the GraphQL server
//! and correlating responses back to Bevy entities for conflict handling.
//!
//! Flow:
//! 1. Local movement → handle_local_token_movement queues mutation
//! 2. mutation_sender_system reads queue and sends HTTP request
//! 3. Server validates, persists, and sends response
//! 4. handle_mutation_response correlates response with mutation_id
//! 5. On conflict (event_code=2), fire Trigger<ConflictDetected>
//! 6. sync_token_positions_from_server applies server position

use bevy::prelude::*;
use std::collections::HashMap;

use crate::systems::token_sync_d2::{GraphQLMutationQueue, PendingMutationInfo};

/// Event: Server rejected mutation (validation error, permission denied, etc)
#[derive(Event, Clone, Debug)]
pub struct MutationRejected {
    pub mutation_id: String,
    pub token_id: String,
    pub error_code: i32,
    pub error_message: String,
}

/// Event: Server confirmed mutation but detected conflict (LWW applied)
#[derive(Event, Clone, Debug)]
pub struct ConflictDetected {
    pub mutation_id: String,
    pub token_id: String,
    pub conflict_code: i32,
}

/// System: Periodically send pending mutations to server
pub fn send_pending_mutations(
    mutation_queue: Res<GraphQLMutationQueue>,
    mut sender_state: ResMut<MutationSenderState>,
) {
    let pending = mutation_queue.get_pending();

    for mutation_info in pending {
        // Skip if already sent (waiting for response)
        if sender_state
            .in_flight
            .contains_key(&mutation_info.mutation_id)
        {
            continue;
        }

        // Skip if recently sent (batching + backpressure)
        if sender_state
            .recently_sent
            .contains(&mutation_info.mutation_id)
        {
            continue;
        }

        eprintln!(
            "[Phase4.9.D2.2📤] Sending mutation: id={}, token={}, pos=({}, {})",
            mutation_info.mutation_id, mutation_info.token_id, mutation_info.x, mutation_info.y
        );

        // Mark as sent (add to in_flight)
        sender_state
            .in_flight
            .insert(mutation_info.mutation_id.clone(), mutation_info.clone());

        // Queue HTTP request (WASM doesn't support blocking HTTP, so we use a task)
        // For now, we'll simulate the response in the next frame
        // In production, this would use gloo_net::http::Request
    }

    // Clear recently_sent after some frames
    if sender_state.frames_since_clear > 5 {
        sender_state.recently_sent.clear();
        sender_state.frames_since_clear = 0;
    } else {
        sender_state.frames_since_clear += 1;
    }
}

/// System: Handle mutation responses from server
/// In a real implementation, this would receive responses via gloo_net
pub fn handle_mutation_responses(
    mut mutation_queue: ResMut<GraphQLMutationQueue>,
    mut sender_state: ResMut<MutationSenderState>,
) {
    // Check in_flight mutations for simulated responses
    let mut completed = Vec::new();

    // Collect completed mutation IDs first (avoid borrow issues)
    {
        for mutation_id in sender_state.in_flight.keys() {
            // Simulate server response delay (2 frames minimum)
            let frames_in_flight = sender_state.frames_since_clear;

            if frames_in_flight > 2 {
                // Simulate successful response (no conflict)
                // In production, this would receive from GraphQL subscription
                completed.push(mutation_id.clone());
            }
        }
    }

    // Process completed mutations
    for mutation_id in completed {
        if let Some(info) = sender_state.in_flight.remove(&mutation_id) {
            eprintln!(
                "[Phase4.9.D2.2✅] Mutation confirmed: id={}, token={}",
                mutation_id, info.token_id
            );

            // Mark as successfully sent
            sender_state.recently_sent.insert(mutation_id.clone());
            mutation_queue.mark_complete(&mutation_id);
        }
    }
}

/// Resource: Tracks state of mutation sender
#[derive(Resource)]
pub struct MutationSenderState {
    /// Mutations currently in flight (sent, awaiting response)
    pub in_flight: HashMap<String, PendingMutationInfo>,

    /// Recently sent mutations (within last N frames)
    pub recently_sent: std::collections::HashSet<String>,

    /// Frame counter for backpressure
    pub frames_since_clear: u32,

    /// GraphQL endpoint
    pub graphql_endpoint: String,
}

impl MutationSenderState {
    pub fn new(graphql_endpoint: String) -> Self {
        Self {
            in_flight: HashMap::new(),
            recently_sent: std::collections::HashSet::new(),
            frames_since_clear: 0,
            graphql_endpoint,
        }
    }
}

impl Default for MutationSenderState {
    fn default() -> Self {
        Self::new("http://localhost:4000/graphql".to_string())
    }
}

/// GraphQL mutation for upsertToken.
///
/// `pub` because nothing in the crate calls it: the sender that would have
/// posted this body is still a simulation (see `handle_mutation_response`
/// above). It is the module's statement of the wire format, and its tests
/// cover it, so it is exported rather than deleted or `#[allow]`ed.
///
/// Example payload:
/// ```json
/// {
///   "query": "mutation { upsertToken(input: { world_id: \"...\", scene_id: \"...\", token_id: \"...\", x: 100, y: 200 }) { id, x, y, event_code } }"
/// }
/// ```
pub fn build_upsert_token_mutation(
    world_id: &str,
    scene_id: &str,
    token_id: &str,
    x: i32,
    y: i32,
) -> String {
    format!(
        r#"mutation {{
          upsertToken(input: {{
            world_id: "{}"
            scene_id: "{}"
            token_id: "{}"
            x: {}
            y: {}
          }}) {{
            id
            x
            y
            event_code
            updated_at
          }}
        }}"#,
        world_id, scene_id, token_id, x, y
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutation_sender_state_creation() {
        let state = MutationSenderState::new("http://localhost:4000/graphql".to_string());
        assert_eq!(state.in_flight.len(), 0);
        assert_eq!(state.recently_sent.len(), 0);
        assert_eq!(state.frames_since_clear, 0);
    }

    #[test]
    fn test_mutation_sender_default() {
        let state = MutationSenderState::default();
        assert!(state.graphql_endpoint.contains("localhost"));
    }

    #[test]
    fn test_build_upsert_token_mutation() {
        let mutation = build_upsert_token_mutation("world-1", "scene-1", "token-1", 100, 200);

        assert!(mutation.contains("upsertToken"));
        assert!(mutation.contains("world-1"));
        assert!(mutation.contains("token-1"));
        assert!(mutation.contains("100"));
        assert!(mutation.contains("200"));
    }

    #[test]
    fn test_mutation_rejected_event() {
        let event = MutationRejected {
            mutation_id: "mut-1".to_string(),
            token_id: "token-1".to_string(),
            error_code: 400,
            error_message: "Invalid position".to_string(),
        };

        assert_eq!(event.mutation_id, "mut-1");
        assert_eq!(event.error_code, 400);
    }

    #[test]
    fn test_conflict_detected_event() {
        let event = ConflictDetected {
            mutation_id: "mut-1".to_string(),
            token_id: "token-1".to_string(),
            conflict_code: 2,
        };

        assert_eq!(event.conflict_code, 2);
    }
}
