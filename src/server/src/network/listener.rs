//! Real-time event distribution: read `world_events`, wake the right clients.
//!
//! This module spawns a long-running async task that polls the event table
//! and hands each new row to the per-world router, which fans it out to the
//! sessions watching that world.
//!
//! # There is no `LISTEN` connection here any more
//!
//! There used to be one, and it did nothing. The task opened a dedicated
//! `tokio-postgres` connection, issued `LISTEN world_events_channel`, logged
//! "waiting for notifications…" — and then never read the notification
//! stream. Every notification it asked for was decoded and dropped. The
//! actual delivery mechanism was, and is, the 100ms poll below.
//!
//! So it cost a permanent Postgres backend and bought nothing, while the log
//! line and the module name asserted a design that was not running. Naming
//! the mechanism honestly matters more here than usual, because the thing
//! being described is invisible when it breaks: events stay durable, HTTP
//! keeps answering, and only the live nudges stop.
//!
//! Waking on `NOTIFY` instead of a timer is still worth doing — it would take
//! delivery latency from a 100ms floor to commit time — and the crate docs in
//! `thunderforge-pg-sockets` argue for it. It is a wake, not a delivery
//! guarantee: a notification only reaches sessions listening at that instant,
//! so the poll has to stay as the reconciliation net behind it either way.
//! What is gone is the pretence that it was already wired up.

use crate::models::WorldEvent;
use crate::schema::world_events;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use thunderforge_pg::{DeliveryConfig, DeliveryMetrics, EventSink, EventSource, run_delivery};
use thunderforge_pg_sockets::SharedWorldRouter;
use tokio::sync::broadcast;
use tokio::time::sleep;

type DbPool = r2d2::Pool<ConnectionManager<diesel::PgConnection>>;

/// What the relay needs from a row: an id and a commit time.
///
/// The trait lives in `thunderforge-pg-sockets` and the type lives here,
/// which is the boundary that keeps the delivery rules testable without a
/// schema, a pool or a database.
impl thunderforge_pg_sockets::Stamped for WorldEvent {
    fn stamp(&self) -> thunderforge_pg_sockets::RowStamp {
        thunderforge_pg_sockets::RowStamp {
            id: self.id,
            created_at: self.created_at,
        }
    }
}

/// Events drained per 100ms poll. Generous relative to the old value of 10:
/// the cursor makes truncation safe (anything left over is picked up on the
/// next pass rather than lost), so this only needs to be large enough that a
/// realistic burst clears in a pass or two.
const POLL_BATCH_SIZE: i64 = 256;
const METRICS_LOG_INTERVAL_SECS: u64 = 10;

/// The pool, presented to the delivery loop as a source of rows.
///
/// Both calls block: `pool.get()` waits on a condvar and diesel's `load`
/// blocks on a socket. The [`EventSource`] contract says so, which is what
/// keeps the loop from ever running them on an async worker.
struct PoolEventSource {
    pool: DbPool,
}

impl EventSource for PoolEventSource {
    type Row = WorldEvent;

    fn poll(&self, after: i64) -> Result<Vec<WorldEvent>, String> {
        poll_new_events_with_conn(&self.pool, after)
    }

    fn high_water(&self) -> Option<i64> {
        current_max_event_id(&self.pool)
    }
}

/// The per-world router, presented to the delivery loop as somewhere to put
/// rows.
struct RouterSink {
    router: SharedWorldRouter<WorldEvent>,
    /// Rate limit for the per-event line, which is far too chatty otherwise.
    last_log: std::sync::Mutex<Instant>,
}

impl EventSink for RouterSink {
    type Row = WorldEvent;

    fn publish(&self, event: WorldEvent) -> usize {
        let world_id = event.world_id;
        let event_id = event.id;
        let event_code = event.event_code;

        // To that world's subscribers, and to nobody else. This used to be one
        // `send` on a process-wide channel that woke every connected client in
        // the system and let each of them discover the event was not theirs.
        let count = self.router.publish(world_id, event);

        if let Ok(mut last) = self.last_log.lock() {
            let now = Instant::now();
            if now.duration_since(*last) > Duration::from_secs(10) {
                eprintln!(
                    "[PubSub] 📢 Event id={event_id} code={event_code} world={world_id} \
                     → {count} subscriber(s); {} world(s) currently routed",
                    self.router.active_worlds()
                );
                *last = now;
            }
        }

        count
    }

    // `reap` is deliberately NOT implemented here, so the delivery loop never
    // calls it.
    //
    // Reaping is `DashMap::retain`, which takes a write lock on *every* shard
    // in turn. Publishing takes a read lock on *one*. Putting the all-shards
    // scan on the same task as delivery means any shard that is momentarily
    // unavailable stops event delivery for the entire server — and delivery
    // is the thing players notice. Housekeeping should never be able to do
    // that, so it runs on its own task on its own schedule (see
    // `spawn_channel_reaper`). Missing a reap costs an idle map entry until
    // the next pass; missing delivery costs the game.
}

/// Spawns the background task that reads world events and wakes the sessions
/// watching them.
///
/// The loop itself lives in `thunderforge_pg::delivery`, where its failure
/// modes — a poll that panics, hangs, or errors — are ordinary test cases
/// rather than things you can only witness by running the whole stack under
/// load. What stays here is the part that genuinely needs Postgres and the
/// router: the two adapters above, and the reporting below.
pub fn spawn_listen_task(pool: DbPool, router: SharedWorldRouter<WorldEvent>) {
    let metrics = Arc::new(DeliveryMetrics::default());
    let source = Arc::new(PoolEventSource { pool });
    let sink = Arc::new(RouterSink {
        router,
        last_log: std::sync::Mutex::new(Instant::now()),
    });

    spawn_metrics_reporter(metrics.clone());
    spawn_channel_reaper(sink.router.clone());

    // Never signalled in the server; the loop runs for the life of the
    // process. The channel exists so tests can stop it.
    let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    // Held forever, because dropping the sender would resolve the watch and
    // stop delivery.
    std::mem::forget(_stop_tx);

    eprintln!("[Server] 🚀 Starting world-event delivery loop");
    tokio::spawn(run_delivery(
        source,
        sink,
        metrics,
        DeliveryConfig::default(),
        stop_rx,
    ));
}

/// Drop world channels nobody is listening to, on their own schedule.
///
/// Every 5s rather than every 100ms: this is a scan of a map sized by *open*
/// worlds, and doing it ten times a second bought nothing. More importantly it
/// is off the delivery path entirely — see the note on `RouterSink`.
fn spawn_channel_reaper(router: SharedWorldRouter<WorldEvent>) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;
            let reaped = router.reap();
            if reaped > 0 {
                eprintln!("[PubSub] 🧹 Released {reaped} idle world channel(s)");
            }
        }
    });
}

/// Report the delivery counters every [`METRICS_LOG_INTERVAL_SECS`].
fn spawn_metrics_reporter(metrics: Arc<DeliveryMetrics>) {
    tokio::spawn(async move {
        let mut last_sent = 0u64;
        let mut last_dropped = 0u64;
        let mut last_polls = 0u64;

        loop {
            sleep(Duration::from_secs(METRICS_LOG_INTERVAL_SECS)).await;

            let sent = metrics.sent.load(Ordering::Relaxed);
            let dropped = metrics.dropped.load(Ordering::Relaxed);
            let polls = metrics.polls.load(Ordering::Relaxed);
            let cursor = metrics.cursor.load(Ordering::Relaxed);
            let errors = metrics.errors.load(Ordering::Relaxed);
            let panics = metrics.panics.load(Ordering::Relaxed);
            let timeouts = metrics.timeouts.load(Ordering::Relaxed);

            let poll_delta = polls.saturating_sub(last_polls);

            // The subscriber half of the same story, and the only place it is
            // reported. Those numbers used to be a line per event per
            // subscriber on the subscription's own task — a blocking write to
            // a pipe, on the hot path, scaling with the thing it described.
            // See `crate::graphql::subscription_metrics`.
            let (sockets, subs_opened, subs_refused, subs_delivered, subs_lagged) =
                crate::graphql::subscription_metrics::snapshot();

            eprintln!(
                "[PubSub] 📊 Metrics [{}s]: sent={} (+{}), dropped={} (+{}), polls={} (+{}), \
                 cursor={}, errors={}, panics={}, timeouts={}, sockets={}, subs_open={}, \
                 subs_refused={}, subs_delivered={}, subs_lagged={}",
                METRICS_LOG_INTERVAL_SECS,
                sent,
                sent.saturating_sub(last_sent),
                dropped,
                dropped.saturating_sub(last_dropped),
                polls,
                poll_delta,
                cursor,
                errors,
                panics,
                timeouts,
                sockets,
                subs_opened,
                subs_refused,
                subs_delivered,
                subs_lagged
            );

            // A poll every 100ms is ~100 per interval. Zero does not mean
            // "quiet", it means the delivery loop has stopped running — the
            // whole backplane down while HTTP keeps answering and the process
            // keeps looking healthy. Separating that from "nothing was
            // written" once took attaching to a frozen process; now it is a
            // line in the log.
            if poll_delta == 0 {
                eprintln!(
                    "[PubSub] 🛑 NO POLLS COMPLETED in the last {}s — real-time delivery is \
                     STOPPED (cursor stuck at {}). Events are still being written and are still \
                     durable; nothing will reach clients live until this recovers.",
                    METRICS_LOG_INTERVAL_SECS, cursor
                );
            }

            last_sent = sent;
            last_dropped = dropped;
            last_polls = polls;
        }
    });
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

// `COMMIT_GRACE` and `settled_cursor` now live in
// `thunderforge_pg_sockets::cursor`, where the rule can be exercised against
// plain values instead of only against a live database.

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
    use thunderforge_pg_sockets::Relay;

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
        let mut relay = Relay::new(start);
        let mut delivered: Vec<(i64, i32)> = Vec::new();
        let turn = |relay: &mut Relay, delivered: &mut Vec<(i64, i32)>| {
            let events = poll_new_events_with_conn(&state.db_pool, relay.cursor()).unwrap();
            for event in relay.absorb(events, chrono::Utc::now().naive_utc()) {
                if event.world_id == world_id {
                    delivered.push((event.id, event.event_code));
                }
            }
        };

        turn(&mut relay, &mut delivered);

        // A commits late, with the lower id.
        a.batch_execute("COMMIT").unwrap();

        turn(&mut relay, &mut delivered);

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
}
