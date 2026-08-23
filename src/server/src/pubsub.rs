//! PostgreSQL NOTIFY/LISTEN backplane for real-time event distribution.
//!
//! This module manages the pub/sub infrastructure for world events.
//! Events are broadcast via a Tokio broadcast channel for fan-out to all WebSocket subscriptions.
//!
//! Phase 4.3 scaffolding: the actual PostgreSQL LISTEN task (Phase 4.4) was
//! never wired up, so nothing constructs `PubSubBackplane` yet — `AppState`
//! creates its own broadcast channel directly instead (see `state.rs`).
//! Kept rather than deleted since it documents the intended shape for when
//! that task lands.

use crate::models::WorldEvent;
use tokio::sync::broadcast;

/// Manages the pub/sub backplane for world events.
#[allow(dead_code)]
pub struct PubSubBackplane {
    /// Broadcast channel sender for world events
    pub broadcast_tx: broadcast::Sender<WorldEvent>,
}

#[allow(dead_code)]
impl PubSubBackplane {
    /// Create the backplane's broadcast channel (sender + receiver pair).
    /// Named `channel`, not `new`, since it doesn't return `Self` — nothing
    /// yet constructs a `PubSubBackplane` itself (see module docs above).
    pub fn channel() -> (
        broadcast::Sender<WorldEvent>,
        broadcast::Receiver<WorldEvent>,
    ) {
        broadcast::channel::<WorldEvent>(1000)
    }

    /// Initialize the backplane and return the receiver.
    /// Phase 4.3: Actual PostgreSQL LISTEN task deferred to Phase 4.4
    pub fn spawn_listener() -> broadcast::Receiver<WorldEvent> {
        let (_tx, rx) = Self::channel();
        eprintln!("PubSub: Backplane initialized (Phase 4.3 - LISTEN task deferred to Phase 4.4)");
        rx
    }
}

#[allow(dead_code)]
impl Default for PubSubBackplane {
    fn default() -> Self {
        let (broadcast_tx, _) = Self::channel();
        Self { broadcast_tx }
    }
}
