//! Phase 4.9.B.2: Session Lifecycle Management
//!
//! Handles player presence tracking, session updates, and cleanup:
//! 1. Record player in players_online on WebSocket connect
//! 2. Touch last_seen on every mutation (via middleware)
//! 3. Clean up stale sessions (idle > 30 min)

use crate::models::{NewPlayersOnline, PlayersOnline};
use crate::schema::players_online;
use crate::state::DbPool;
use chrono::{Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

const IDLE_THRESHOLD_SECS: i32 = 30 * 60; // 30 minutes

/// Record a player's connection to a world/scene
///
/// Called when WebSocket connects. If player is already in players_online
/// for this world, updates last_seen and scene_id.
pub async fn connect_player(
    pool: DbPool,
    player_id: Uuid,
    world_id: Uuid,
    scene_id: Option<Uuid>,
) -> Result<PlayersOnline, String> {
    let mut conn = pool.get().map_err(|e| format!("Pool error: {}", e))?;

    let now = Utc::now().naive_utc();

    // Try to upsert: if player already connected to this world, update; otherwise insert
    let result = diesel::insert_into(players_online::table)
        .values(&NewPlayersOnline {
            player_id,
            world_id,
            scene_id,
            connected_at: now,
            last_seen: now,
            idle_duration_secs: 0,
            created_at: now,
            updated_at: now,
        })
        .on_conflict((players_online::player_id, players_online::world_id))
        .do_update()
        .set((
            players_online::last_seen.eq(now),
            players_online::scene_id.eq(scene_id),
            players_online::idle_duration_secs.eq(0),
            players_online::updated_at.eq(now),
        ))
        .get_result::<PlayersOnline>(&mut conn)
        .map_err(|e| format!("Failed to upsert player: {}", e))?;

    eprintln!(
        "🎮 Player {} connected to world {} (scene: {:?})",
        player_id, world_id, scene_id
    );

    Ok(result)
}

/// Disconnect a player from a world
///
/// Called when WebSocket closes. Removes the player from players_online.
pub async fn disconnect_player(
    pool: DbPool,
    player_id: Uuid,
    world_id: Uuid,
) -> Result<(), String> {
    let mut conn = pool.get().map_err(|e| format!("Pool error: {}", e))?;

    diesel::delete(
        players_online::table.filter(
            players_online::player_id
                .eq(player_id)
                .and(players_online::world_id.eq(world_id)),
        ),
    )
    .execute(&mut conn)
    .map_err(|e| format!("Failed to delete player: {}", e))?;

    eprintln!(
        "🔌 Player {} disconnected from world {}",
        player_id, world_id
    );

    Ok(())
}

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
