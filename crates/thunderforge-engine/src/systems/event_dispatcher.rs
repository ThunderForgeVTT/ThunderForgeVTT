//! Phase 4.9.D.1: Event Dispatcher System
//!
//! Converts incoming GraphQL ServerEvent messages into an event queue,
//! enabling systems to react to server updates in a decoupled way.

use bevy::prelude::*;
use serde_json::Value;
use std::collections::VecDeque;

/// Bevy event representing a world event from server
#[derive(Clone, Debug)]
pub struct WorldEventReceived {
    /// Event ID from server
    pub event_id: i64,
    /// Event code (1=normal, 2=conflict, -1=rejected, etc.)
    pub event_code: i32,
    /// Full token event payload (JSONB from server)
    pub token_event: Option<Value>,
    /// Token ID (extracted from payload)
    pub token_id: Option<String>,
    /// User who caused the event (from payload)
    pub created_by: Option<String>,
    /// Optional custom event type
    pub event_type: Option<String>,
}

/// Resource: Queue of world events dispatched to ECS
#[derive(Resource, Default)]
pub struct WorldEventQueue {
    events: VecDeque<WorldEventReceived>,
}

impl WorldEventQueue {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Push an event to the queue
    pub fn push(&mut self, event: WorldEventReceived) {
        self.events.push_back(event);
    }

    /// Drain all queued events
    pub fn drain(&mut self) -> Vec<WorldEventReceived> {
        self.events.drain(..).collect()
    }

    /// Get the next event without removing it
    pub fn peek(&self) -> Option<&WorldEventReceived> {
        self.events.front()
    }
}

/// Extract token ID from JSONB payload
pub fn extract_token_id(payload: &Value) -> Option<String> {
    payload
        .get("token_id")
        .or_else(|| payload.get("tokenId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract user ID from JSONB payload
pub fn extract_user_id(payload: &Value) -> Option<String> {
    payload
        .get("created_by")
        .or_else(|| payload.get("createdBy"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract event type from JSONB payload
pub fn extract_event_type(payload: &Value) -> Option<String> {
    payload
        .get("event_type")
        .or_else(|| payload.get("eventType"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// `dispatch_world_events` was here.
//
// It took `ResMut<WebSocketClient>` from `crate::network::websocket_client` —
// a module `network/mod.rs` does not declare. `network/websocket_client.rs`
// and `network/transport.rs` are files on disk that are not part of the crate
// on any target, exactly as this module and four of its neighbours were
// (spec 032 T083). So the function could not compile against the crate as it
// exists, and nothing called it. Removed rather than resurrected: reviving it
// means deciding what the WebSocket transport is, which is a decision, not a
// test repair.
//
// The queue it fed, and everything that reads that queue, is intact.

/// System: Log all world events (for debugging)
pub fn log_world_events(mut queue: ResMut<WorldEventQueue>) {
    let events = queue.drain();

    for event in events {
        let event_name = match event.event_code {
            1 => "NORMAL",
            2 => "CONFLICT_LWW",
            -1 => "REJECTED",
            -2 => "PERMISSION_DENIED",
            _ => "UNKNOWN",
        };

        eprintln!(
            "[Phase4.9.D🔔] {} (code={}): token={:?}, user={:?}",
            event_name, event.event_code, event.token_id, event.created_by
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_token_id() {
        let payload = serde_json::json!({
            "token_id": "token-123"
        });
        assert_eq!(extract_token_id(&payload), Some("token-123".to_string()));

        let payload_alt = serde_json::json!({
            "tokenId": "token-456"
        });
        assert_eq!(
            extract_token_id(&payload_alt),
            Some("token-456".to_string())
        );

        let payload_empty = serde_json::json!({});
        assert_eq!(extract_token_id(&payload_empty), None);
    }

    #[test]
    fn test_extract_user_id() {
        let payload = serde_json::json!({
            "created_by": "user-123"
        });
        assert_eq!(extract_user_id(&payload), Some("user-123".to_string()));

        let payload_alt = serde_json::json!({
            "createdBy": "user-456"
        });
        assert_eq!(extract_user_id(&payload_alt), Some("user-456".to_string()));

        let payload_empty = serde_json::json!({});
        assert_eq!(extract_user_id(&payload_empty), None);
    }

    #[test]
    fn test_world_event_queue() {
        let mut queue = WorldEventQueue::new();

        queue.push(WorldEventReceived {
            event_id: 1,
            event_code: 1,
            token_event: None,
            token_id: Some("token-123".to_string()),
            created_by: Some("user-123".to_string()),
            event_type: None,
        });

        queue.push(WorldEventReceived {
            event_id: 2,
            event_code: 2,
            token_event: None,
            token_id: Some("token-456".to_string()),
            created_by: Some("user-456".to_string()),
            event_type: None,
        });

        let events = queue.drain();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[1].event_id, 2);
    }

    #[test]
    fn test_world_event_received_creation() {
        let event = WorldEventReceived {
            event_id: 1,
            event_code: 1,
            token_event: Some(serde_json::json!({
                "token_id": "token-123",
                "x": 100,
                "y": 200
            })),
            token_id: Some("token-123".to_string()),
            created_by: Some("user-123".to_string()),
            event_type: None,
        };

        assert_eq!(event.event_id, 1);
        assert_eq!(event.event_code, 1);
        assert_eq!(event.token_id, Some("token-123".to_string()));
        assert_eq!(event.created_by, Some("user-123".to_string()));
    }
}
