//! Phase 4.9.B.2: Session Lifecycle Management
//!
//! What remains of the `players_online` bookkeeping:
//! 1. Touch last_seen on token mutations
//! 2. Clean up stale sessions (idle > 30 min)
//!
//! Rows were inserted only by the `/api/events/{world_id}` socket, which
//! nothing connected to and which was removed in spec 051 (research R7). Live
//! presence is held in memory (`AppState::presence`); nothing inserts into
//! this table now, so both functions below find nothing new to act on. They stay while
//! they have callers, and go with the table when it is dropped.

use crate::schema::players_online;
use crate::state::DbPool;
use chrono::{Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

const IDLE_THRESHOLD_SECS: i32 = 30 * 60; // 30 minutes

/// Touch last_seen for a player in a world
///
/// Called on every mutation to update activity timestamp.
/// Used by cleanup task to identify idle players.
pub async fn touch_last_seen(pool: DbPool, player_id: Uuid, world_id: Uuid) -> Result<(), String> {
    let mut conn = pool.get().map_err(|e| format!("Pool error: {}", e))?;

    let now = Utc::now().naive_utc();

    diesel::update(
        players_online::table.filter(
            players_online::player_id
                .eq(player_id)
                .and(players_online::world_id.eq(world_id)),
        ),
    )
    .set((
        players_online::last_seen.eq(now),
        players_online::idle_duration_secs.eq(0),
        players_online::updated_at.eq(now),
    ))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to touch last_seen: {}", e))?;

    Ok(())
}

/// Spawn the session cleanup task
///
/// Runs periodically to:
/// 1. Calculate idle_duration_secs for all players
/// 2. Delete sessions idle for > 30 minutes
///
/// This prevents players_online from growing unbounded if clients crash
/// without sending disconnect messages.
pub fn spawn_session_cleanup_task(pool: DbPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60)); // Run every minute

        loop {
            interval.tick().await;

            if let Err(e) = cleanup_idle_sessions(pool.clone()).await {
                eprintln!("⚠️  Session cleanup error: {}", e);
            }
        }
    });
}

/// Clean up idle sessions
async fn cleanup_idle_sessions(pool: DbPool) -> Result<(), String> {
    let mut conn = pool.get().map_err(|e| format!("Pool error: {}", e))?;

    let now = Utc::now().naive_utc();
    let threshold = now - Duration::seconds(IDLE_THRESHOLD_SECS as i64);

    // Delete sessions idle for > threshold
    let deleted =
        diesel::delete(players_online::table.filter(players_online::last_seen.lt(threshold)))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to delete idle sessions: {}", e))?;

    if deleted > 0 {
        eprintln!("🧹 Cleaned up {} idle sessions", deleted);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_threshold_constant() {
        // Verify idle threshold is 30 minutes
        assert_eq!(IDLE_THRESHOLD_SECS, 30 * 60);
        assert_eq!(IDLE_THRESHOLD_SECS, 1800);
    }

    #[test]
    fn test_idle_threshold_seconds() {
        let thirty_min_secs = IDLE_THRESHOLD_SECS as i64;
        let thirty_min_duration = Duration::seconds(thirty_min_secs);
        assert_eq!(thirty_min_duration.num_minutes(), 30);
    }
}
