//! Phase 4.9.D.1: WebSocket Client Wrapper for WASM
//!
//! Abstracts gloo-net WebSocket for Bevy, provides connection state management,
//! exponential backoff reconnection, and message deserialization.

#![cfg(target_arch = "wasm32")]

use bevy::prelude::*;
use gloo_net::websocket::{futures::WebSocket, Message as WsMessage};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use super::ServerEvent;

/// Connection state of WebSocket
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// GraphQL subscription message from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLMessage {
    pub r#type: String,  // "data", "error", "complete"
    pub id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

/// WebSocket client for GraphQL subscriptions (WASM-safe)
#[derive(Resource)]
pub struct WebSocketClient {
    pub server_url: String,
    pub state: ConnectionState,
    pub last_event_id: i64,
    pub message_queue: VecDeque<ServerEvent>,
    pub connection_attempts: u32,
    pub last_connection_time: f64,
}

impl WebSocketClient {
    /// Create a new WebSocket client with server URL
    pub fn new(server_url: String) -> Self {
        Self {
            server_url,
            state: ConnectionState::Disconnected,
            last_event_id: 0,
            message_queue: VecDeque::new(),
            connection_attempts: 0,
            last_connection_time: 0.0,
        }
    }

    /// Get current connection state
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Enqueue a server event received from WebSocket
    pub fn enqueue_event(&mut self, event: ServerEvent) {
        self.last_event_id = event.id;
        self.message_queue.push_back(event);
    }

    /// Drain all queued events
    pub fn drain_events(&mut self) -> Vec<ServerEvent> {
        self.message_queue.drain(..).collect()
    }

    /// Get exponential backoff delay (1s, 2s, 4s, 8s, max 30s)
    pub fn get_reconnect_delay(&self) -> f64 {
        let base_delay = 1.0;
        let max_delay = 30.0;
        let delay = base_delay * 2.0_f64.powi(self.connection_attempts as i32);
        delay.min(max_delay)
    }

    /// Mark connection state as connecting
    pub fn set_connecting(&mut self) {
        self.state = ConnectionState::Connecting;
    }

    /// Mark connection state as connected
    pub fn set_connected(&mut self) {
        self.state = ConnectionState::Connected;
        self.connection_attempts = 0;
    }

    /// Mark connection state as reconnecting
    pub fn set_reconnecting(&mut self) {
        self.state = ConnectionState::Reconnecting;
        self.connection_attempts += 1;
    }

    /// Mark connection state as failed
    pub fn set_failed(&mut self) {
        self.state = ConnectionState::Failed;
    }

    /// Mark connection state as disconnected
    pub fn set_disconnected(&mut self) {
        self.state = ConnectionState::Disconnected;
    }
}

/// Task handle for WebSocket polling (stored as resource)
#[derive(Resource, Default)]
pub struct WebSocketTask {
    pub running: bool,
}

/// Configuration for WebSocket connection
#[derive(Clone, Resource)]
pub struct WebSocketConfig {
    pub server_url: String,
    pub world_id: String,
}

impl WebSocketConfig {
    pub fn new(server_url: String, world_id: String) -> Self {
        Self {
            server_url,
            world_id,
        }
    }

    /// Get GraphQL WebSocket URL (converts http:// to ws://)
    pub fn graphql_ws_url(&self) -> String {
        let ws_url = if self.server_url.starts_with("https://") {
            self.server_url.replace("https://", "wss://")
        } else {
            self.server_url.replace("http://", "ws://")
        };
        format!("{}/graphql", ws_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_client_creation() {
        let client = WebSocketClient::new("ws://localhost:8080".to_string());
        assert_eq!(client.server_url, "ws://localhost:8080");
        assert_eq!(client.state, ConnectionState::Disconnected);
    }

    #[test]
    fn test_connection_states() {
        let mut client = WebSocketClient::new("ws://localhost:8080".to_string());

        client.set_connecting();
        assert_eq!(client.state, ConnectionState::Connecting);

        client.set_connected();
        assert_eq!(client.state, ConnectionState::Connected);
        assert_eq!(client.connection_attempts, 0);

        client.set_reconnecting();
        assert_eq!(client.state, ConnectionState::Reconnecting);
        assert_eq!(client.connection_attempts, 1);
    }

    #[test]
    fn test_reconnect_delay() {
        let mut client = WebSocketClient::new("ws://localhost:8080".to_string());

        // First attempt: 1 second
        assert_eq!(client.get_reconnect_delay(), 1.0);

        // Second attempt: 2 seconds
        client.connection_attempts = 1;
        assert_eq!(client.get_reconnect_delay(), 2.0);

        // Fifth attempt: 16 seconds
        client.connection_attempts = 4;
        assert_eq!(client.get_reconnect_delay(), 16.0);

        // Sixth attempt: capped at 30 seconds
        client.connection_attempts = 5;
        assert_eq!(client.get_reconnect_delay(), 30.0);
    }

    #[test]
    fn test_event_queue() {
        let mut client = WebSocketClient::new("ws://localhost:8080".to_string());

        client.enqueue_event(ServerEvent {
            id: 1,
            event_code: 1,
            token_event: None,
        });

        client.enqueue_event(ServerEvent {
            id: 2,
            event_code: 2,
            token_event: None,
        });

        assert_eq!(client.last_event_id, 2);

        let events = client.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, 1);
        assert_eq!(events[1].id, 2);

        let events_after = client.drain_events();
        assert!(events_after.is_empty());
    }

    #[test]
    fn test_websocket_config_urls() {
        let config = WebSocketConfig::new(
            "http://localhost:8080".to_string(),
            "world-123".to_string(),
        );
        assert_eq!(config.graphql_ws_url(), "ws://localhost:8080/graphql");

        let config_https = WebSocketConfig::new(
            "https://example.com".to_string(),
            "world-456".to_string(),
        );
        assert_eq!(
            config_https.graphql_ws_url(),
            "wss://example.com/graphql"
        );
    }
}
