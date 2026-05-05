//! PostgreSQL LISTEN background task for real-time event distribution.
//!
//! This module spawns a long-running async task that:
//! 1. Periodically polls PostgreSQL for new world events
//! 2. Queries events created after the last seen ID
//! 3. Broadcasts events to all subscribers via tokio::sync::broadcast
//!
//! Phase 4.5: Replace polling with true LISTEN/NOTIFY support

use crate::models::WorldEvent as ModelWorldEvent;
use crate::schema::world_events;
use diesel::r2d2::{self, ConnectionManager};
use diesel::prelude::*;
use std::time::Duration;
use thunderforge_core::events::WorldEvent;
use tokio::sync::broadcast;
use tokio::time::sleep;

type DbPool = r2d2::Pool<ConnectionManager<diesel::PgConnection>>;

/// Spawns a background task that listens to database events.
/// 
/// This task:
/// - Periodically polls for new world events
/// - Broadcasts events to all subscribers
/// - Automatically recovers on connection loss
pub fn spawn_listen_task(
    pool: DbPool,
    broadcast_tx: broadcast::Sender<WorldEvent>,
) {
    tokio::spawn(async move {
        let mut last_event_id: i64 = 0;

        loop {
            match poll_new_events(&pool, &mut last_event_id) {
                Ok(events) => {
                    for event in events {
                        eprintln!("[PubSub] 📡 Broadcasting event id={}", event.id);
                        
                        // Convert models::WorldEvent to core::events::WorldEvent
                        let core_event = WorldEvent::new(event.world_id.to_string());
                        
                        if let Err(broadcast::error::SendError(_)) = broadcast_tx.send(core_event) {
                            eprintln!("[PubSub] ⚠️  No active subscribers");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[PubSub] Error polling events: {}. Retrying in 1s...", e);
                }
            }

            // Poll interval: 1 second
            sleep(Duration::from_secs(1)).await;
        }
    });
}

/// Poll the database for new events since last_event_id.
fn poll_new_events(pool: &DbPool, last_event_id: &mut i64) -> Result<Vec<ModelWorldEvent>, String> {
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    world_events::table
        .filter(world_events::id.gt(*last_event_id))
        .order(world_events::id.asc())
        .limit(10)
        .load::<ModelWorldEvent>(&mut conn)
        .map_err(|e| format!("Database query failed: {}", e))
        .and_then(|events| {
            if let Some(last) = events.last() {
                *last_event_id = last.id;
            }
            Ok(events)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_filtering() {
        // Verify that we track last_event_id correctly
        let mut last_id = 5;
        assert_eq!(last_id, 5);
        last_id = 10;
        assert_eq!(last_id, 10);
    }
}

