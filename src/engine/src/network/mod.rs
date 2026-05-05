//! GraphQL Network Synchronization Layer
//!
//! Implements WASM-safe communication with the backend GraphQL server.
//! Handles WebSocket subscriptions and mutation calls for real-time sync.

#![cfg(target_arch = "wasm32")]

pub mod client;
pub mod websocket;
pub mod mutations;

use bevy::prelude::*;
use bevy::ecs::event::Event;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Represents a GraphQL mutation result from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult {
    /// ID of the mutation (for correlation)
    pub id: Option<i64>,
    /// Was the mutation successful
    pub success: bool,
    /// Event code (1 = created, 2 = moved, 3 = updated, -1 = rejected)
    pub event_code: i32,
    /// Error message if rejected
    pub error: Option<String>,
}

/// Represents a server event from worldEventCreated subscription
#[derive(Debug, Clone, Serialize, Deserialize, Event)]
pub struct ServerEvent {
    /// Event ID
    pub id: i64,
    /// Event code (1 = TokenCreated, 2 = TokenMoved, etc.)
    pub event_code: i32,
    /// Token data (JSON payload)
    pub token_event: Option<serde_json::Value>,
}

/// Resource for managing GraphQL network state
#[derive(Resource, Clone)]
pub struct GraphQLClient {
    /// Server endpoint (e.g., "http://localhost:8080")
    pub endpoint: String,

    /// Last mutation ID (for correlation)
    pub last_mutation_id: Arc<std::sync::atomic::AtomicI64>,
}

impl GraphQLClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            last_mutation_id: Arc::new(std::sync::atomic::AtomicI64::new(0)),
        }
    }

    /// Get the next mutation ID
    fn next_mutation_id(&self) -> i64 {
        let atomic = self.last_mutation_id.as_ref();
        atomic.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Fire a moveToken mutation to the server
    pub async fn move_token(
        &self,
        world_id: String,
        token_id: String,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<MutationResult, String> {
        let mutation_id = self.next_mutation_id();

        let mutation = r#"
            mutation moveToken($input: MoveTokenInput!) {
                moveToken(input: $input) {
                    id
                    success
                    eventCode
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

        self.execute_mutation(mutation, variables, mutation_id).await
    }

    /// Execute a generic GraphQL mutation
    async fn execute_mutation(
        &self,
        _query: &str,
        _variables: serde_json::Value,
        mutation_id: i64,
    ) -> Result<MutationResult, String> {
        eprintln!("[GraphQL] Executing mutation id={}", mutation_id);

        // Phase 4.3: Implement actual GraphQL mutation via fetch API
        // For now, return a successful mutation result
        // This will be replaced with actual network communication
        Ok(MutationResult {
            id: Some(mutation_id),
            success: true,
            event_code: 2, // Moved
            error: None,
        })
    }
}

/// Resource for managing the WebSocket subscription
#[derive(Resource)]
pub struct WorldEventSubscription {
    /// Last event ID received
    pub last_event_id: i64,

    /// Queue of incoming server events
    pub event_queue: Arc<std::sync::Mutex<Vec<ServerEvent>>>,
}

impl WorldEventSubscription {
    pub fn new() -> Self {
        Self {
            last_event_id: 0,
            event_queue: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Add an event to the queue
    pub fn enqueue_event(&self, event: ServerEvent) {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.push(event);
        }
    }

    /// Get and clear the event queue
    pub fn drain_events(&self) -> Vec<ServerEvent> {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }
}

/// JavaScript callback for WebSocket events
/// This is called from JavaScript when server sends events
#[wasm_bindgen]
pub fn on_world_event_received(event_json: &str) {
    // This would need access to the Bevy World to update the subscription resource
    // For now, we enqueue it to a static queue
    if let Ok(event) = serde_json::from_str::<ServerEvent>(event_json) {
        WORLD_EVENT_QUEUE.lock().ok().map(|mut q| q.push(event));
    }
}

/// Static queue for world events (filled from JavaScript)
static WORLD_EVENT_QUEUE: std::sync::Mutex<Vec<ServerEvent>> =
    std::sync::Mutex::new(Vec::new());

/// System to process incoming world events from the subscription
pub fn process_server_events(subscription: ResMut<WorldEventSubscription>) {
    // Drain events from static queue
    if let Ok(mut queue) = WORLD_EVENT_QUEUE.lock() {
        for event in queue.drain(..) {
            subscription.enqueue_event(event);
        }
    }
}

/// System to log network events (for debugging)
pub fn log_network_events(subscription: Res<WorldEventSubscription>) {
    let events = subscription.drain_events();
    for event in events {
        eprintln!(
            "[Network] Server event: id={}, code={}",
            event.id, event.event_code
        );
        subscription.enqueue_event(event); // Re-add for other systems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphql_client_creation() {
        let client = GraphQLClient::new("http://localhost:8080".to_string());
        assert_eq!(client.endpoint, "http://localhost:8080");
    }

    #[test]
    fn test_mutation_id_increment() {
        let client = GraphQLClient::new("http://localhost:8080".to_string());
        assert_eq!(client.next_mutation_id(), 0);
        assert_eq!(client.next_mutation_id(), 1);
        assert_eq!(client.next_mutation_id(), 2);
    }

    #[test]
    fn test_world_event_subscription() {
        let subscription = WorldEventSubscription::new();
        assert_eq!(subscription.last_event_id, 0);

        subscription.enqueue_event(ServerEvent {
            id: 1,
            event_code: 2,
            token_event: Some(json!({})),
        });

        let events = subscription.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, 1);
    }
}
