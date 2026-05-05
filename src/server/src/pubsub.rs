//! PostgreSQL NOTIFY/LISTEN backplane for real-time event distribution.
//!
//! This module manages the pub/sub infrastructure for world events.
//! Events are broadcast via a Tokio broadcast channel for fan-out to all WebSocket subscriptions.

use crate::models::WorldEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Manages the pub/sub backplane for world events.
pub struct PubSubBackplane {
    /// Broadcast channel sender for world events
    pub broadcast_tx: broadcast::Sender<WorldEvent>,
}

impl PubSubBackplane {
    /// Create a new pub/sub backplane.
    /// Returns a sender and a receiver for the broadcast channel.
    pub fn new() -> (broadcast::Sender<WorldEvent>, broadcast::Receiver<WorldEvent>) {
        broadcast::channel::<WorldEvent>(1000)
    }

    /// Initialize the backplane and return the receiver.
    /// Phase 4.3: Actual PostgreSQL LISTEN task deferred to Phase 4.4
    pub fn spawn_listener() -> broadcast::Receiver<WorldEvent> {
        let (_tx, rx) = Self::new();
        eprintln!("PubSub: Backplane initialized (Phase 4.3 - LISTEN task deferred to Phase 4.4)");
        rx
    }
}

impl Default for PubSubBackplane {
    fn default() -> Self {
        let (broadcast_tx, _) = Self::new();
        Self { broadcast_tx }
    }
}
