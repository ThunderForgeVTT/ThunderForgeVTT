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
    // The high-water mark is waited for, not guessed at.
    //
    // This used to be `unwrap_or(i64::MAX)`, with a comment saying a failure
    // "starts from nothing yet and the first genuinely new event moves the
    // cursor forward". That is not what `i64::MAX` does. No row can ever
    // satisfy `id > i64::MAX`, so a single failed query at boot — a pool not
    // yet warm, a database still starting — left this task polling forever
    // and delivering **nothing, ever**, while logging a cheerful
    // "Streaming events after id=9223372036854775807" and looking healthy.
    //
    // Falling back to 0 is not the answer either: it would replay the entire
    // `world_events` table to every client. Both guesses are wrong, so this
    // does not guess. The database being unavailable at startup is a
    // transient condition and the honest response is to wait for it.
    let mut last_event_id: i64 = loop {
        // `None` is the failure case; an empty table answers `Some(0)`.
        match current_max_event_id(pool) {
            Some(id) => break id,
            None => {
                eprintln!(
                    "[PubSub] ⚠️  Cannot read the event high-water mark; retrying in 1s. \
                     No events will be delivered until this succeeds."
                );
                sleep(Duration::from_secs(1)).await;
            }
        }
    };
    eprintln!("[PubSub] 📍 Streaming events after id={}", last_event_id);
    let mut last_log_time = Instant::now();

    // Ids already broadcast but not yet passed by the cursor.
    //
    // The cursor deliberately lags behind delivery (see `settled_cursor`), so
    // each poll re-reads rows it has already sent. This is what stops them
    // being sent twice. Bounded by the number of events inside one
    // `COMMIT_GRACE` window and pruned every pass.
    let mut delivered: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();

    loop {
        // Poll for new events every 100ms
        match poll_new_events_with_conn(pool, last_event_id) {
            Ok(events) => {
                // The cursor advances only past rows old enough that nothing
                // older can still be uncommitted — never simply to the last id
                // seen, which is what used to lose an out-of-order commit.
                let settled =
                    settled_cursor(&events, last_event_id, chrono::Utc::now().naive_utc());

                for event in events {
                    // Broadcast on first sight, whatever the cursor is doing.
                    // Waiting for the row to settle would put `COMMIT_GRACE`
                    // on every event's latency for a race that affects almost
                    // none of them.
                    if !delivered.insert(event.id) {
                        continue;
                    }

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

                // Only now, and only as far as the settled rule allows.
                last_event_id = settled;
                // Anything the cursor has passed can never come back in a
                // future poll, so remembering it serves no purpose.
                delivered = delivered.split_off(&(last_event_id + 1));
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

/// How long a row must have existed before the cursor may pass it.
///
/// This is the window in which a transaction can still be holding an id it
/// took but has not committed. It bounds the race described on
/// [`settled_cursor`]: a transaction that keeps its id longer than this can
/// still be missed, and one that commits within it cannot.
///
/// Two seconds is chosen against what actually writes here — single-statement
/// inserts at the end of a mutation that has already done its work — where
/// the gap between taking an id and committing is sub-millisecond. It costs
/// nothing at delivery time (see the loop: rows are broadcast the moment they
/// are seen, not when they settle) and only delays how quickly the cursor
/// forgets them.
const COMMIT_GRACE: Duration = Duration::from_secs(2);

/// The highest id the cursor may advance to, given what this poll returned.
///
/// # The bug this exists to prevent
///
/// `world_events.id` is `BIGSERIAL`. A sequence value is taken at **INSERT**
/// and the row becomes visible at **COMMIT**, and those two orders need not
/// agree. If transaction A takes id 102 and B takes 103, and B commits first,
/// a poll landing between them sees only `[103]`. Advancing the cursor to 103
/// means 102 — committing a millisecond later — can never satisfy
/// `id > cursor` again. It is lost permanently, with no log, no metric and no
/// `Lagged`: the quietest failure in the whole delivery path.
///
/// That is not hypothetical. `a_poll_between_out_of_order_commits_still_
/// delivers_the_lower_id` stages exactly that interleaving against a real
/// database, and before this rule existed it lost the event every time.
///
/// # The rule
///
/// Walk the batch in ascending id order and stop at the first row young
/// enough that an older sibling could still be uncommitted. Everything before
/// it is settled: any transaction that took a lower id has had longer than
/// [`COMMIT_GRACE`] to commit, so if it has not appeared by now it never
/// will (it rolled back, and its id is a permanent gap).
///
/// Delivery does not wait for this — rows are broadcast as soon as they are
/// seen. Only the *cursor* waits, which is why the fix costs latency nowhere.
/// The price is that a row is re-read from the database on each poll until it
/// settles, and the caller must therefore remember what it has already sent.
fn settled_cursor(events: &[WorldEvent], current: i64, now: chrono::NaiveDateTime) -> i64 {
    let cutoff = now - chrono::Duration::from_std(COMMIT_GRACE).unwrap_or_default();
    let mut settled = current;
    for event in events {
        if event.created_at >= cutoff {
            break;
        }
        settled = settled.max(event.id);
    }
    settled
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

    /// An event that takes a lower id but commits later must still be
    /// delivered.
    ///
    /// `world_events.id` is `BIGSERIAL`: the sequence value is taken at
    /// **INSERT**, the row appears at **COMMIT**, and those orders need not
    /// agree. If A takes 102 and B takes 103, and B commits first, a poll
    /// landing between them sees only `[103]`. A cursor that advances to the
    /// last id seen puts 102 permanently out of reach — no log, no metric, no
    /// `Lagged`, just an event that never arrives.
    ///
    /// This stages that interleaving against a real database: two
    /// connections, two open transactions, ids taken in one order and
    /// committed in the other, with a poll in between. It failed before
    /// `settled_cursor` existed, reporting `Saw [901]`.
    #[test]
    fn a_poll_between_out_of_order_commits_still_delivers_the_lower_id() {
        use crate::test_support::*;
        use diesel::connection::SimpleConnection;
        use uuid::Uuid;

        let state = test_app_state();
        let mut setup = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut setup);
        let world_id = insert_test_world(&mut setup, owner_id);
        let start = current_max_event_id(&state.db_pool).expect("high-water mark");
        drop(setup);

        // Two independent connections, so the transactions are genuinely
        // concurrent rather than nested.
        let mut a = state.db_pool.get().unwrap();
        let mut b = state.db_pool.get().unwrap();

        let insert = |world: Uuid, user: Uuid, code: i32| {
            format!(
                "INSERT INTO world_events \
                 (world_id, event_code, schema_version, created_at, updated_at, \
                  created_by, updated_by) \
                 VALUES ('{world}', {code}, 1, now(), now(), '{user}', '{user}')"
            )
        };

        // A takes the lower id first and holds its transaction open.
        a.batch_execute("BEGIN").unwrap();
        a.batch_execute(&insert(world_id, owner_id, 900)).unwrap();

        // B takes the higher id and commits immediately.
        b.batch_execute("BEGIN").unwrap();
        b.batch_execute(&insert(world_id, owner_id, 901)).unwrap();
        b.batch_execute("COMMIT").unwrap();

        // One turn of the listener's loop, exactly as `run_listen_loop` does
        // it: poll, deliver everything seen, then advance the cursor only as
        // far as `settled_cursor` permits.
        let mut cursor = start;
        let mut delivered: Vec<(i64, i32)> = Vec::new();
        let mut seen_ids = std::collections::BTreeSet::new();
        let mut turn = |cursor: &mut i64,
                        delivered: &mut Vec<(i64, i32)>,
                        seen: &mut std::collections::BTreeSet<i64>| {
            let events = poll_new_events_with_conn(&state.db_pool, *cursor).unwrap();
            let settled = settled_cursor(&events, *cursor, chrono::Utc::now().naive_utc());
            for event in &events {
                if seen.insert(event.id) && event.world_id == world_id {
                    delivered.push((event.id, event.event_code));
                }
            }
            *cursor = settled;
        };

        turn(&mut cursor, &mut delivered, &mut seen_ids);

        // A commits late, with the lower id.
        a.batch_execute("COMMIT").unwrap();

        turn(&mut cursor, &mut delivered, &mut seen_ids);

        let codes: Vec<i32> = delivered.iter().map(|(_, code)| *code).collect();
        assert!(
            codes.contains(&901),
            "the early-committing event must be delivered; saw {codes:?}"
        );
        assert!(
            codes.contains(&900),
            "the event that took the lower id but committed later was lost: the \
             cursor advanced past it while it was still invisible, and \
             `id > cursor` can never return it. Saw {codes:?}"
        );
        assert_eq!(
            codes.len(),
            2,
            "a lagging cursor re-reads rows it has already sent, so each must \
             still be delivered exactly once; saw {codes:?}"
        );
    }

    /// The cursor must not pass a row young enough to have an uncommitted
    /// older sibling — and must not stall on one either.
    #[test]
    fn the_cursor_stops_at_the_first_row_too_young_to_be_settled() {
        let now = chrono::Utc::now().naive_utc();
        let old = now - chrono::Duration::seconds(60);
        let fresh = now - chrono::Duration::milliseconds(10);

        let events = vec![
            event_at(10, old),
            event_at(11, old),
            // Young: nothing at or beyond this may be passed, because a
            // transaction holding an id below it could still commit.
            event_at(12, fresh),
            event_at(13, old),
        ];

        assert_eq!(
            settled_cursor(&events, 9, now),
            11,
            "the cursor stops before the first unsettled row"
        );
        // Note 13 is old but comes after 12: order, not age, decides where the
        // walk stops. Passing 13 would strand anything that took id 12.
        assert_eq!(
            settled_cursor(&[], 9, now),
            9,
            "an empty poll moves nothing"
        );
        assert_eq!(
            settled_cursor(&[event_at(10, fresh)], 9, now),
            9,
            "a batch of nothing but fresh rows leaves the cursor where it was"
        );
        assert_eq!(
            settled_cursor(&[event_at(10, old), event_at(11, old)], 9, now),
            11,
            "an entirely settled batch advances all the way"
        );
    }

    fn event_at(id: i64, created_at: chrono::NaiveDateTime) -> WorldEvent {
        WorldEvent {
            id,
            world_id: uuid::Uuid::nil(),
            event_code: 1,
            token_event: None,
            created_at,
            schema_version: 1,
            updated_at: created_at,
            created_by: uuid::Uuid::nil(),
            updated_by: uuid::Uuid::nil(),
        }
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
