//! Axum WebSocket handler for GraphQL subscriptions.
//!
//! This module provides the HTTP upgrade handler that converts incoming WebSocket
//! connections into GraphQL subscription channels. Connected clients can subscribe
//! to worldEventCreated and receive real-time event updates from the broadcast channel.
//!
//! Phase 4.9.B.2: Enhanced with session lifecycle tracking (connect/disconnect)

use axum::{
    Extension,
    extract::{Path, State, ws::WebSocketUpgrade},
    response::IntoResponse,
};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{auth_middleware::AuthenticatedUser, models::WorldEvent, session, state::AppState};

/// Handles HTTP upgrade requests for WebSocket connections.
///
/// Accepts world_id as a path parameter: `/ws/{world_id}`
///
/// This function is used as an Axum handler:
/// ```ignore
/// app.route("/events/{world_id}", get(websocket_handler))
/// ```
///
/// When a client connects, it:
/// 1. Records the player in players_online (Phase 4.9.B.2)
/// 2. Subscribes to the broadcast channel
/// 3. Sends real-time world events to the client
/// 4. Removes the player on disconnect
pub async fn websocket_handler(
    State(app_state): State<AppState>,
    Path(world_id): Path<Uuid>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    eprintln!(
        "[WS] New WebSocket connection attempt: player={}, world={}",
        auth_user.user_id, world_id
    );

    let player_id = auth_user.user_id;
    let broadcast_rx = app_state.world_event_sender.subscribe();

    ws.on_upgrade(move |socket| handle_socket(socket, broadcast_rx, app_state, player_id, world_id))
}

/// Handles the WebSocket connection lifecycle.
///
/// Phase 4.9.B.2:
/// 1. Record player in players_online on connect
/// 2. Subscribe to broadcast and send events
/// 3. Remove player on disconnect
async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    mut broadcast_rx: broadcast::Receiver<WorldEvent>,
    app_state: AppState,
    player_id: Uuid,
    world_id: Uuid,
) {
    // 1. Record player connection (Phase 4.9.B.2)
    if let Err(e) = session::connect_player(
        app_state.db_pool.clone(),
        player_id,
        world_id,
        None, // Scene will be updated by client mutations
    )
    .await
    {
        eprintln!("[WS] ❌ Failed to record player connection: {}", e);
        return;
    }

    eprintln!("[WS] ✅ WebSocket connection established");

    // Send a welcome message
    if let Err(e) = socket
        .send(axum::extract::ws::Message::Text(
            r#"{"type":"connection_ack"}"#.into(),
        ))
        .await
    {
        eprintln!("[WS] ❌ Failed to send ACK: {}", e);
        session::disconnect_player(app_state.db_pool.clone(), player_id, world_id)
            .await
            .ok();
        return;
    }

    // Main event loop: forward broadcast events to WebSocket
    loop {
        match broadcast_rx.recv().await {
            Ok(event) => {
                // Filter by world_id
                if event.world_id != world_id {
                    continue;
                }

                // Convert event to JSON payload
                let payload = serde_json::json!({
                    "type": "data",
                    "id": 1,
                    "payload": {
                        "data": {
                            "worldEventCreated": {
                                "id": event.id,
                                "worldId": event.world_id,
                                "eventCode": event.event_code,
                                "createdBy": event.created_by,
                            }
                        }
                    }
                });

                let message = match serde_json::to_string(&payload) {
                    Ok(json) => {
                        eprintln!("[WS] 📤 Sending event to player {}", player_id);
                        axum::extract::ws::Message::Text(json.into())
                    }
                    Err(e) => {
                        eprintln!("[WS] ⚠️  Failed to serialize event: {}", e);
                        continue;
                    }
                };

                // Send to client
                if let Err(e) = socket.send(message).await {
                    eprintln!(
                        "[WS] ❌ Failed to send event to {}: {}. Closing connection.",
                        player_id, e
                    );
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

    // 3. Remove player on disconnect (Phase 4.9.B.2)
    if let Err(e) = session::disconnect_player(app_state.db_pool.clone(), player_id, world_id).await
    {
        eprintln!("[WS] ❌ Failed to record player disconnect: {}", e);
    }

    eprintln!(
        "[WS] 🔌 WebSocket connection closed for player {}",
        player_id
    );
}

#[cfg(test)]
mod tests {

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
