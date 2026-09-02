//! Phase 4.9.D.3: Player Presence System
//!
//! Handles player presence indicators:
//! - Broadcasting local player camera position
//! - Receiving remote player positions via subscriptions
//! - Rendering presence indicators (cursors, names) on canvas
//!
//! Architecture:
//! 1. Local: Camera system broadcasts position every 500ms
//! 2. Network: Broadcast via GraphQL mutation updatePlayerPresence
//! 3. Remote: Subscriptions receive playerPresenceUpdated events
//! 4. Render: Presence components display cursors/names

use bevy::prelude::*;

/// Component: Represents a remote player's presence
#[derive(Component, Clone, Debug)]
pub struct PlayerPresence {
    pub player_id: String,
    pub player_name: String,
    pub world_id: String,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub last_seen: f64,
    pub color: Color,
}

impl PlayerPresence {
    pub fn new(player_id: String, player_name: String, world_id: String) -> Self {
        Self {
            player_id,
            player_name,
            world_id,
            camera_x: 0.0,
            camera_y: 0.0,
            camera_zoom: 1.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            last_seen: 0.0,
            color: Color::srgb(0.2, 0.8, 1.0), // Cyan
        }
    }

    /// Check if presence is stale (> 10 seconds)
    pub fn is_stale(&self, current_time: f64) -> bool {
        current_time - self.last_seen > 10.0
    }
}

/// Component: Marks a sprite as presence cursor indicator
#[derive(Component, Clone, Debug)]
pub struct PresenceCursor {
    pub player_id: String,
}

/// Component: Marks a text as presence name label
#[derive(Component, Clone, Debug)]
pub struct PresenceLabel {
    pub player_id: String,
}

/// Resource: Tracks all connected player presences
#[derive(Resource, Default)]
pub struct PresenceRegistry {
    pub players: std::collections::HashMap<String, PlayerPresence>,
}

impl PresenceRegistry {
    pub fn add_or_update(&mut self, presence: PlayerPresence) {
        eprintln!(
            "[Phase4.9.D3👥] Presence updated: player={}, pos=({:.1}, {:.1})",
            presence.player_id, presence.camera_x, presence.camera_y
        );
        self.players.insert(presence.player_id.clone(), presence);
    }

    pub fn remove(&mut self, player_id: &str) {
        if let Some(presence) = self.players.remove(player_id) {
            eprintln!(
                "[Phase4.9.D3👥] Presence removed: player={}",
                presence.player_id
            );
        }
    }

    pub fn get(&self, player_id: &str) -> Option<&PlayerPresence> {
        self.players.get(player_id)
    }

    pub fn get_all(&self) -> Vec<PlayerPresence> {
        self.players.values().cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.players.len()
    }
}

/// Resource: Tracks local player state for broadcasting
#[derive(Resource)]
pub struct LocalPlayerPresence {
    pub player_id: String,
    pub player_name: String,
    pub world_id: String,
    pub last_broadcast: f64,
    pub broadcast_interval: f64, // seconds
}

impl LocalPlayerPresence {
    pub fn new(player_id: String, player_name: String, world_id: String) -> Self {
        Self {
            player_id,
            player_name,
            world_id,
            last_broadcast: 0.0,
            broadcast_interval: 0.5, // 500ms
        }
    }

    pub fn should_broadcast(&self, current_time: f64) -> bool {
        current_time - self.last_broadcast >= self.broadcast_interval
    }

    pub fn mark_broadcast(&mut self, current_time: f64) {
        self.last_broadcast = current_time;
    }
}

/// System: Broadcast local player camera position periodically
pub fn broadcast_player_presence(mut local_player: ResMut<LocalPlayerPresence>, time: Res<Time>) {
    let current_time = time.elapsed_secs() as f64;

    if !local_player.should_broadcast(current_time) {
        return;
    }

    // Simulate broadcasting camera position
    eprintln!(
        "[Phase4.9.D3📡] Broadcasting presence: player={}, time={:.2}",
        local_player.player_id, current_time
    );

    // In production, this would:
    // 1. Get camera position from camera_query
    // 2. Get mouse position from input
    // 3. Send HTTP mutation updatePlayerPresence
    // 4. Server broadcasts to all connected clients via pg_notify

    local_player.mark_broadcast(current_time);
}

/// System: Update presence indicators based on registry
pub fn update_presence_indicators(presence_registry: Res<PresenceRegistry>, time: Res<Time>) {
    let current_time = time.elapsed_secs() as f64;

    // Log updates
    for presence in presence_registry.get_all() {
        if presence.is_stale(current_time) {
            eprintln!(
                "[Phase4.9.D3👥] Presence stale, should remove: player={}",
                presence.player_id
            );
        }
    }

    // In production, this would:
    // 1. Query for PresenceCursor components and update their transforms
    // 2. Query for PresenceLabel components and update their text
    // 3. Despawn stale presence entities after 10 seconds
}

/// System: Receive and process playerPresenceUpdated subscription events
pub fn process_presence_updates(
    presence_registry: ResMut<PresenceRegistry>,
    _world_event_queue: Res<crate::systems::event_dispatcher::WorldEventQueue>,
) {
    // In production, subscribe to worldEventCreated and filter for presence_updated events
    // For now, this is a stub that receives events from the server

    // Example: Each worldEventCreated with event_code=3 (presence update) contains:
    // {
    //   "player_id": "player-1",
    //   "player_name": "Alice",
    //   "camera_x": 100.5,
    //   "camera_y": 200.5,
    //   "camera_zoom": 1.5,
    //   "cursor_x": 150.0,
    //   "cursor_y": 250.0
    // }

    eprintln!(
        "[Phase4.9.D3📡] Presence updates processed. Current players: {}",
        presence_registry.count()
    );
}

/// System: Log presence changes for debugging
pub fn log_presence_events(presence_registry: Res<PresenceRegistry>) {
    if presence_registry.count() > 0 {
        eprintln!(
            "[Phase4.9.D3👥] Connected players: {}",
            presence_registry.count()
        );

        for presence in presence_registry.get_all() {
            eprintln!(
                "  - {}: camera=({:.1}, {:.1}), cursor=({:.1}, {:.1})",
                presence.player_name,
                presence.camera_x,
                presence.camera_y,
                presence.cursor_x,
                presence.cursor_y
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_presence_creation() {
        let presence = PlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );

        assert_eq!(presence.player_id, "player-1");
        assert_eq!(presence.player_name, "Alice");
        assert!(!presence.is_stale(5.0)); // 5 seconds < 10 second stale threshold
    }

    #[test]
    fn test_presence_stale_detection() {
        let mut presence = PlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );

        presence.last_seen = 0.0;
        assert!(presence.is_stale(11.0)); // > 10 seconds
        assert!(!presence.is_stale(9.0)); // < 10 seconds
    }

    #[test]
    fn test_presence_registry_operations() {
        let mut registry = PresenceRegistry::default();

        let presence1 = PlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );

        registry.add_or_update(presence1.clone());
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.get("player-1").unwrap().player_name, "Alice");

        registry.remove("player-1");
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_local_player_presence_broadcast() {
        let mut local_player = LocalPlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );

        assert!(local_player.should_broadcast(1.0));

        local_player.mark_broadcast(1.0);
        assert!(!local_player.should_broadcast(1.1)); // Only 0.1s passed, need 0.5s

        local_player.mark_broadcast(1.0);
        assert!(local_player.should_broadcast(1.6)); // 0.6s passed, > 0.5s interval
    }

    #[test]
    fn test_presence_cursor_component() {
        let cursor = PresenceCursor {
            player_id: "player-1".to_string(),
        };

        assert_eq!(cursor.player_id, "player-1");
    }

    #[test]
    fn test_presence_label_component() {
        let label = PresenceLabel {
            player_id: "player-2".to_string(),
        };

        assert_eq!(label.player_id, "player-2");
    }
}
