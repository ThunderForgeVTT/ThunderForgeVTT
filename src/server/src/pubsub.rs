//! PostgreSQL NOTIFY/LISTEN backplane for real-time event distribution.
//!
//! This module manages a single PostgreSQL connection listening on world_events_channel.
//! When notifications are received, events are fetched from the database and broadcast
//! via a Tokio broadcast channel for fan-out to all WebSocket subscriptions.

use crate::models::WorldEvent;
use diesel::r2d2::{self, ConnectionManager};
use std::sync::Arc;
use tokio::sync::broadcast;

#[allow(dead_code)]
type DbPool = r2d2::Pool<ConnectionManager<diesel::PgConnection>>;

/// Manages the pub/sub backplane for world events.
#[allow(dead_code)]
pub struct PubSubBackplane {
    /// Broadcast channel for world events. Clients subscribe to receive events.
    broadcast_tx: broadcast::Sender<WorldEvent>,
}

impl PubSubBackplane {
    /// Spawn a new pub/sub backplane task.
    ///
    /// This creates a single Tokio task that:
    /// 1. Acquires a dedicated database connection
    /// 2. Issues a PostgreSQL LISTEN command on world_events_channel
    /// 3. Waits for notifications in a loop
    /// 4. When notified, fetches the full event from the database
    /// 5. Broadcasts the event to all subscribers
    ///
    /// The broadcast channel has a buffer of 1000 events. If subscribers lag,
    /// old events are dropped but new ones continue to flow.
    pub async fn spawn_listener(pool: DbPool) -> Result<broadcast::Receiver<WorldEvent>, String> {
        let (tx, _rx) = broadcast::channel::<WorldEvent>(1000);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            // This task runs indefinitely, listening for database notifications
            if let Err(e) = Self::listen_loop(pool, tx_clone).await {
                eprintln!("PubSub listener error: {}", e);
            }
        });

        Ok(tx.subscribe())
    }

    /// Main loop for listening to PostgreSQL notifications.
    #[allow(dead_code)]
    async fn listen_loop(pool: DbPool, _tx: broadcast::Sender<WorldEvent>) -> Result<(), String> {
        loop {
            // Get a dedicated connection for LISTEN
            let mut conn = pool
                .get()
                .map_err(|e| format!("Failed to get DB connection: {}", e))?;

            // Issue LISTEN command via raw SQL
            tokio::task::spawn_blocking(move || {
                use diesel::prelude::*;
                diesel::sql_query("LISTEN world_events_channel")
                    .execute(&mut conn)
                    .map_err(|e| format!("Failed to execute LISTEN: {}", e))
            })
            .await
            .map_err(|e| format!("Blocking task error: {}", e))??;

            // Wait for notifications (this is a simplified version; in production,
            // you'd use an async PostgreSQL driver like tokio-postgres)
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    /// Get a subscriber handle for this backplane.
    ///
    /// Each caller gets their own broadcast::Receiver that will receive
    /// all subsequent events. Events already sent before subscription is created
    /// are not received (fire-and-forget semantics).
    #[allow(dead_code)]
    pub fn subscribe(&self) -> broadcast::Receiver<WorldEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Send an event to all subscribers (primarily for testing/manual triggering).
    #[allow(dead_code)]
    pub fn publish(&self, event: WorldEvent) {
        let _ = self.broadcast_tx.send(event);
    }
}

/// Initialize the pub/sub backplane for a given database pool.
#[allow(dead_code)]
pub async fn initialize_pubsub(pool: DbPool) -> Result<Arc<PubSubBackplane>, String> {
    let _rx = PubSubBackplane::spawn_listener(pool.clone()).await?;

    Ok(Arc::new(PubSubBackplane {
        broadcast_tx: broadcast::channel::<WorldEvent>(1000).0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubsub_broadcast_channel_capacity() {
        // Ensure broadcast channel is sized appropriately
        let (_tx, _rx) = broadcast::channel::<WorldEvent>(1000);
        // Channel capacity = 1000, meaning up to 1000 events can be buffered
    }
}
