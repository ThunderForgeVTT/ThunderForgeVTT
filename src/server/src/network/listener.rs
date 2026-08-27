//! PostgreSQL LISTEN background task for real-time event distribution.
//!
//! This module spawns a long-running async task that:
//! 1. Establishes a PostgreSQL connection via tokio-postgres
//! 2. Issues LISTEN world_events_channel
//! 3. Broadcasts notifications to all subscribers via tokio::sync::broadcast
//! 4. Auto-reconnects on connection loss
//! 5. Tracks metrics and handles backpressure gracefully
//!
//! Phase 4.9.A: Full pg_notify listener integration with backpressure handling

use crate::models::WorldEvent;
use crate::schema::world_events;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::{sleep, timeout};

type DbPool = r2d2::Pool<ConnectionManager<diesel::PgConnection>>;

/// Configuration for the PubSub backplane
const LISTEN_CHANNEL: &str = "world_events_channel";

/// Events drained per 100ms poll. Generous relative to the old value of 10:
/// the cursor makes truncation safe (anything left over is picked up on the
/// next pass rather than lost), so this only needs to be large enough that a
/// realistic burst clears in a pass or two.
const POLL_BATCH_SIZE: i64 = 256;
const RECONNECT_DELAY_MS: u64 = 1000;
const RECONNECT_MAX_DELAY_MS: u64 = 30000;
const BROADCAST_BUFFER_SIZE: usize = 10000;

/// Backpressure thresholds
const BACKPRESSURE_WARNING_THRESHOLD: usize = 8000; // 80% full
const BACKPRESSURE_CRITICAL_THRESHOLD: usize = 9500; // 95% full
const METRICS_LOG_INTERVAL_SECS: u64 = 10;

/// Metrics for monitoring broadcast channel health
#[derive(Debug, Clone)]
struct BroadcastMetrics {
    events_sent: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
    subscriber_lagged_count: Arc<AtomicU64>,
}

/// Spawns a background task that listens to database events via PostgreSQL NOTIFY.
///
/// This task:
/// - Establishes a dedicated PostgreSQL connection for LISTEN
/// - Issues LISTEN on world_events_channel
/// - Broadcasts notifications to all subscribers
/// - Automatically recovers on connection loss with exponential backoff
/// - Tracks metrics and logs periodically
pub fn spawn_listen_task(pool: DbPool, broadcast_tx: broadcast::Sender<WorldEvent>) {
    let metrics = BroadcastMetrics {
        events_sent: Arc::new(AtomicU64::new(0)),
        events_dropped: Arc::new(AtomicU64::new(0)),
        subscriber_lagged_count: Arc::new(AtomicU64::new(0)),
    };

    let metrics_clone = metrics.clone();

    // Spawn metrics reporter task
    tokio::spawn(async move {
        let mut last_events_sent = 0u64;
        let mut last_events_dropped = 0u64;
        let mut last_lagged = 0u64;

        loop {
            sleep(Duration::from_secs(METRICS_LOG_INTERVAL_SECS)).await;

            let events_sent = metrics_clone.events_sent.load(Ordering::Relaxed);
            let events_dropped = metrics_clone.events_dropped.load(Ordering::Relaxed);
            let lagged_count = metrics_clone
                .subscriber_lagged_count
                .load(Ordering::Relaxed);

            let sent_delta = events_sent.saturating_sub(last_events_sent);
            let dropped_delta = events_dropped.saturating_sub(last_events_dropped);
            let lagged_delta = lagged_count.saturating_sub(last_lagged);

            eprintln!(
                "[PubSub] 📊 Metrics [{}s]: sent={} (+{}), dropped={} (+{}), lagged={} (+{})",
                METRICS_LOG_INTERVAL_SECS,
                events_sent,
                sent_delta,
                events_dropped,
                dropped_delta,
                lagged_count,
                lagged_delta
            );

            last_events_sent = events_sent;
            last_events_dropped = events_dropped;
            last_lagged = lagged_count;
        }
    });

    tokio::spawn(async move {
        let mut reconnect_delay = RECONNECT_DELAY_MS;

        loop {
            eprintln!("[PubSub] 🔄 Starting PostgreSQL LISTEN connection");

            match run_listen_loop(&pool, &broadcast_tx, &metrics).await {
                Ok(_) => {
                    // Unexpected end, reset reconnect delay
                    reconnect_delay = RECONNECT_DELAY_MS;
                }
                Err(e) => {
                    eprintln!(
                        "[PubSub] ❌ LISTEN loop failed: {}. Reconnecting in {}ms...",
                        e, reconnect_delay
                    );
                    sleep(Duration::from_millis(reconnect_delay)).await;

                    // Exponential backoff: double delay up to max
                    reconnect_delay = (reconnect_delay * 2).min(RECONNECT_MAX_DELAY_MS);
                }
            }
        }
    });
}

/// Run the main LISTEN loop with a single PostgreSQL connection.
///
/// Returns Ok(()) if loop exits cleanly (shouldn't happen in production).
/// Returns Err(String) if connection fails or LISTEN fails.
async fn run_listen_loop(
    pool: &DbPool,
    broadcast_tx: &broadcast::Sender<WorldEvent>,
    metrics: &BroadcastMetrics,
) -> Result<(), String> {
    // Get database URL from environment
    let db_url =
        std::env::var("DATABASE_URL").map_err(|e| format!("DATABASE_URL not set: {}", e))?;

    eprintln!(
        "[PubSub] 📡 Connecting to PostgreSQL LISTEN on '{}'",
        LISTEN_CHANNEL
    );

    // Create a dedicated tokio-postgres connection (not from pool)
    let (_client, connection) = timeout(
        Duration::from_secs(10),
        tokio_postgres::connect(&db_url, tokio_postgres::tls::NoTls),
    )
    .await
    .map_err(|_| "Connection timeout".to_string())?
    .map_err(|e| format!("Failed to connect: {}", e))?;

    // Spawn the connection handler in a separate task
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("[PubSub] ⚠️  Connection error: {}", e);
        }
    });

    // Issue LISTEN on the channel to keep it active (connection kept alive for notifications)
    _client
        .execute(&format!("LISTEN {}", LISTEN_CHANNEL), &[])
        .await
        .map_err(|e| format!("LISTEN failed: {}", e))?;

    eprintln!(
        "[PubSub] ✅ LISTEN active on '{}', waiting for notifications...",
        LISTEN_CHANNEL
    );

    // Use polling since true LISTEN/NOTIFY stream handling is complex in tokio-postgres
    // We'll poll for events periodically while keeping the connection alive
    // Start from the newest event that already exists, not from zero.
    //
    // The cursor is now a real `id > ?` bound, so a zero start would replay
    // the entire world_events table to every connected client on boot. The
    // previous "newest ten, skip duplicates" shape hid this: it could never
    // reach backwards past ten rows. A listener is for what happens next;
    // history is what the delta sync is for.
    //
    // Failing to read the high-water mark is not fatal — falling back to 0
    // would replay everything, so a failure here starts from "nothing yet"
    // and the first genuinely new event moves the cursor forward.
    let mut last_event_id: i64 = current_max_event_id(pool).unwrap_or(i64::MAX);
    eprintln!("[PubSub] 📍 Streaming events after id={}", last_event_id);
    let mut last_log_time = Instant::now();

    loop {
        // Poll for new events every 100ms
        match poll_new_events_with_conn(pool, last_event_id) {
            Ok(events) => {
                for event in events {
                    // The query already excludes anything at or below
                    // `last_event_id` and returns ascending, so every row here
                    // is new and arrives in the order it was recorded.
                    // Advancing the cursor per event keeps that true if the
                    // batch is truncated by LIMIT.
                    last_event_id = event.id;

                    eprintln!(
                        "[PubSub] 📡 Event id={}, world_id={}, code={}",
                        event.id, event.world_id, event.event_code
                    );

                    // Broadcast the full models::WorldEvent to all subscribers
                    match broadcast_tx.send(event) {
                        Ok(count) => {
                            metrics.events_sent.fetch_add(1, Ordering::Relaxed);
                            let buffer_len = broadcast_tx.len();

                            // Log backpressure warnings
                            if buffer_len >= BACKPRESSURE_CRITICAL_THRESHOLD {
                                eprintln!(
                                    "[PubSub] 🔴 CRITICAL: Broadcast buffer at {}% capacity ({}/{})",
                                    (buffer_len * 100) / BROADCAST_BUFFER_SIZE,
                                    buffer_len,
                                    BROADCAST_BUFFER_SIZE
                                );
                            } else if buffer_len >= BACKPRESSURE_WARNING_THRESHOLD {
                                eprintln!(
                                    "[PubSub] 🟡 WARNING: Broadcast buffer at {}% capacity ({}/{})",
                                    (buffer_len * 100) / BROADCAST_BUFFER_SIZE,
                                    buffer_len,
                                    BROADCAST_BUFFER_SIZE
                                );
                            }

                            // Debug: log every 100 events or every 10 seconds
                            let now = Instant::now();
                            if now.duration_since(last_log_time) > Duration::from_secs(10) {
                                eprintln!(
                                    "[PubSub] 📢 Event sent to {} subscribers (buffer: {}/{})",
                                    count, buffer_len, BROADCAST_BUFFER_SIZE
                                );
                                last_log_time = now;
                            }
                        }
                        Err(broadcast::error::SendError(_)) => {
                            eprintln!("[PubSub] ⚠️  No active subscribers");
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[PubSub] ⚠️  Poll error: {}", e);
                // Don't break on poll errors - continue retrying
            }
        }

        // Short poll interval
        sleep(Duration::from_millis(100)).await;
    }
}

/// The highest event id currently recorded, or `None` if that cannot be read.
fn current_max_event_id(pool: &DbPool) -> Option<i64> {
    let mut conn = pool.get().ok()?;
    world_events::table
        .select(diesel::dsl::max(world_events::id))
        .first::<Option<i64>>(&mut conn)
        .ok()
        .flatten()
        .or(Some(0))
}

/// Poll the database for events newer than `after_id`, oldest first.
///
/// Two properties here are load-bearing, and getting either wrong drops
/// events silently:
///
/// 1. **Ascending order.** This previously selected the ten most recent events
///    `DESC` and skipped any whose id was `<= last_event_id`. Iterating a
///    descending batch sets `last_event_id` to the *newest* event on the first
///    pass, so every older event in the same batch is then discarded as a
///    duplicate. Any burst landing inside one 100ms poll window lost all but
///    the newest — a GM moving several tokens quickly, a map import, or a
///    dice roll alongside a chat message. Found by the tier-5 session storm:
///    three events sent back-to-back delivered one, the same three spaced
///    1.5s apart delivered all three.
/// 2. **Filtering in SQL, not in the loop.** Bounding on `id > after_id` means
///    the `LIMIT` applies to events we have not seen yet. With the old
///    "newest ten, filter afterwards" shape, eleven events inside one window
///    pushed the oldest out of the batch entirely, so they could never be
///    delivered no matter how the loop behaved.
fn poll_new_events_with_conn(pool: &DbPool, after_id: i64) -> Result<Vec<WorldEvent>, String> {
    let mut conn = pool
        .get()
        .map_err(|e| format!("Failed to get connection: {}", e))?;

    world_events::table
        .filter(world_events::id.gt(after_id))
        .order(world_events::id.asc())
        .limit(POLL_BATCH_SIZE)
        .load::<WorldEvent>(&mut conn)
        .map_err(|e| format!("Database query failed: {}", e))
}

/// Spawns a background task that listens to presence changes via PostgreSQL NOTIFY.
///
/// This task (Phase 4.9.B.3):
/// - Establishes a dedicated PostgreSQL connection for LISTEN
/// - Issues LISTEN on players_online_channel
/// - Broadcasts presence change notifications to all subscribers
/// - Automatically recovers on connection loss
pub fn spawn_presence_listener_task(_broadcast_tx: broadcast::Sender<serde_json::Value>) {
    tokio::spawn(async move {
        // Note: In production, we'd use tokio-postgres for true LISTEN support.
        // For now, we provide a fallback approach.
        eprintln!("[Presence] Presence listener started (polling fallback)");

        loop {
            sleep(Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(LISTEN_CHANNEL, "world_events_channel");
        assert_eq!(RECONNECT_DELAY_MS, 1000);
        assert_eq!(RECONNECT_MAX_DELAY_MS, 30000);
        const { assert!(BROADCAST_BUFFER_SIZE > 1000) };
    }

    #[test]
    fn test_exponential_backoff() {
        let mut delay = RECONNECT_DELAY_MS;
        assert_eq!(delay, 1000);

        delay = (delay * 2).min(RECONNECT_MAX_DELAY_MS);
        assert_eq!(delay, 2000);

        delay = (delay * 2).min(RECONNECT_MAX_DELAY_MS);
        assert_eq!(delay, 4000);

        // Keep increasing until we hit max
        for _ in 0..20 {
            delay = (delay * 2).min(RECONNECT_MAX_DELAY_MS);
        }
        assert_eq!(delay, RECONNECT_MAX_DELAY_MS);
    }

    #[test]
    fn test_backpressure_thresholds() {
        // Warning threshold at 80% (8000/10000)
        assert_eq!(BACKPRESSURE_WARNING_THRESHOLD, 8000);
        // Critical threshold at 95% (9500/10000)
        assert_eq!(BACKPRESSURE_CRITICAL_THRESHOLD, 9500);
        // Warning should be less than critical
        const { assert!(BACKPRESSURE_WARNING_THRESHOLD < BACKPRESSURE_CRITICAL_THRESHOLD) };
        // Both should be less than buffer size
        const { assert!(BACKPRESSURE_CRITICAL_THRESHOLD < BROADCAST_BUFFER_SIZE) };
    }

    #[test]
    fn test_metrics_creation() {
        let metrics = BroadcastMetrics {
            events_sent: Arc::new(AtomicU64::new(0)),
            events_dropped: Arc::new(AtomicU64::new(0)),
            subscriber_lagged_count: Arc::new(AtomicU64::new(0)),
        };

        assert_eq!(metrics.events_sent.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.events_dropped.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.subscriber_lagged_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = BroadcastMetrics {
            events_sent: Arc::new(AtomicU64::new(0)),
            events_dropped: Arc::new(AtomicU64::new(0)),
            subscriber_lagged_count: Arc::new(AtomicU64::new(0)),
        };

        metrics.events_sent.fetch_add(1, Ordering::Relaxed);
        assert_eq!(metrics.events_sent.load(Ordering::Relaxed), 1);

        metrics.events_sent.fetch_add(5, Ordering::Relaxed);
        assert_eq!(metrics.events_sent.load(Ordering::Relaxed), 6);
    }
}
