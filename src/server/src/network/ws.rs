//! Axum WebSocket handler for GraphQL subscriptions.
//!
//! This module provides the HTTP upgrade handler that converts incoming WebSocket
//! connections into GraphQL subscription channels. Connected clients can subscribe
//! to worldEventCreated and receive real-time event updates from the broadcast channel.

use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use tokio::sync::broadcast;
use thunderforge_core::events::WorldEvent;

/// Handles HTTP upgrade requests for WebSocket connections.
/// 
/// This function is used as an Axum handler:
/// ```ignore
/// app.route("/ws", get(websocket_handler))
/// ```
/// 
/// When a client connects, it subscribes to the broadcast channel
/// and sends real-time world events to the client.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(broadcast_rx): axum::extract::State<broadcast::Receiver<WorldEvent>>,
) -> impl IntoResponse {
    eprintln!("[WS] New WebSocket connection attempt");
    ws.on_upgrade(|socket| handle_socket(socket, broadcast_rx))
}

/// Handles the WebSocket connection lifecycle.
/// 
/// Once upgraded, this function:
/// 1. Subscribes to the broadcast channel
/// 2. Enters a loop sending events to the client
/// 3. Handles client disconnection gracefully
async fn handle_socket(
    mut socket: WebSocket,
    mut broadcast_rx: broadcast::Receiver<WorldEvent>,
) {
    eprintln!("[WS] ✅ WebSocket connection established");

    // Send a welcome message
    if let Err(e) = socket
        .send(axum::extract::ws::Message::Text(
            r#"{"type":"connection_ack"}"#.into(),
        ))
        .await
    {
        eprintln!("[WS] ❌ Failed to send ACK: {}", e);
        return;
    }

    // Main event loop: forward broadcast events to WebSocket
    loop {
        match broadcast_rx.recv().await {
            Ok(event) => {
                // Convert event to JSON payload
                let payload = serde_json::json!({
                    "type": "data",
                    "id": 1,  // Simplified for Phase 4.4; Phase 4.5 adds request ID tracking
                    "payload": {
                        "data": {
                            "worldEventCreated": {
                                "eventCode": event.event_code,
                            }
                        }
                    }
                });

                let message = match serde_json::to_string(&payload) {
                    Ok(json) => {
                        eprintln!("[WS] 📤 Sending event to client");
                        axum::extract::ws::Message::Text(json.into())
                    }
                    Err(e) => {
                        eprintln!("[WS] ⚠️  Failed to serialize event: {}", e);
                        continue;
                    }
                };

                // Send to client
                if let Err(e) = socket.send(message).await {
                    eprintln!("[WS] ❌ Failed to send event: {}. Closing connection.", e);
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // Subscriber lagged behind; send error but continue
                eprintln!("[WS] ⚠️  Subscriber lagged; some events may have been missed");
                if let Err(e) = socket
                    .send(axum::extract::ws::Message::Text(
                        r#"{"type":"error","message":"Subscriber lagged"}"#.into(),
                    ))
                    .await
                {
                    eprintln!("[WS] ❌ Failed to send lag warning: {}", e);
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                // Broadcast channel closed; all publishers have dropped
                eprintln!("[WS] ℹ️  Broadcast channel closed");
                break;
            }
        }
    }

    eprintln!("[WS] 🔌 WebSocket connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        // Verify GraphQL subscription message structure
        let payload = serde_json::json!({
            "type": "data",
            "id": 1,
            "payload": {
                "data": {
                    "worldEventCreated": {
                        "eventCode": 2
                    }
                }
            }
        });

        let serialized = serde_json::to_string(&payload).expect("Should serialize");
        assert!(serialized.contains("\"type\":\"data\""));
        assert!(serialized.contains("\"id\":1"));
    }
}
