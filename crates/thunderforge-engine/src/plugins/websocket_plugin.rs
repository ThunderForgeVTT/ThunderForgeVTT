//! Phase 4.9.D.1: WebSocket Plugin
//!
//! Bevy plugin that initializes and manages WebSocket connection,
//! handles subscriptions, and integrates with the game loop.

#![cfg(target_arch = "wasm32")]

use bevy::prelude::*;

use crate::network::websocket_client::{WebSocketClient, WebSocketConfig, ConnectionState};

/// Plugin to set up WebSocket connection and polling
pub struct WebSocketPlugin {
    pub config: WebSocketConfig,
}

impl Plugin for WebSocketPlugin {
    fn build(&self, app: &mut App) {
        let client = WebSocketClient::new(self.config.graphql_ws_url());
        
        app
            .insert_resource(client)
            .insert_resource(self.config.clone())
            .add_systems(Startup, setup_websocket_connection)
            .add_systems(Update, poll_websocket_messages);
    }
}

impl WebSocketPlugin {
    pub fn new(server_url: String, world_id: String) -> Self {
        Self {
            config: WebSocketConfig::new(server_url, world_id),
        }
    }
}

/// Startup system: Initialize WebSocket connection
fn setup_websocket_connection(
    mut client: ResMut<WebSocketClient>,
    config: Res<WebSocketConfig>,
) {
    client.set_connecting();
    eprintln!(
        "[Phase4.9.D🔌] Connecting WebSocket to: {}",
        client.server_url
    );
    eprintln!(
        "[Phase4.9.D📡] Subscribing to world: {}",
        config.world_id
    );

    // Phase 4.9.D.2: Actual async WebSocket connection
    // For now, we just initialize the connection state
    // The real connection will be established asynchronously in next phase
}

/// Update system: Poll for incoming messages
fn poll_websocket_messages(mut client: ResMut<WebSocketClient>) {
    // Phase 4.9.D.2: Poll messages from WebSocket
    // For now, mark that this is where polling would happen
    
    match client.state {
        ConnectionState::Connecting => {
            // Simulate connection success for testing
            client.set_connected();
            eprintln!("[Phase4.9.D✅] WebSocket connected (simulated)");
        }
        ConnectionState::Connected => {
            // Poll for messages (will be implemented in D.2)
            eprintln!("[Phase4.9.D📡] Polling for messages...");
        }
        ConnectionState::Reconnecting => {
            let delay = client.get_reconnect_delay();
            eprintln!("[Phase4.9.D🔄] Reconnecting... (delay: {}s)", delay);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_plugin_creation() {
        let plugin = WebSocketPlugin::new(
            "http://localhost:8080".to_string(),
            "world-123".to_string(),
        );
        assert_eq!(plugin.config.server_url, "http://localhost:8080");
        assert_eq!(plugin.config.world_id, "world-123");
        assert_eq!(
            plugin.config.graphql_ws_url(),
            "ws://localhost:8080/graphql"
        );
    }
}

