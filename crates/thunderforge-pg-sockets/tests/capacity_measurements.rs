//! Measurements behind `docs/research/capacity-analysis.md`.
//!
//! These are not assertions about correctness; they are the instruments that
//! produced the numbers in that document. They are `#[ignore]`d because a
//! timing or RSS measurement is not a pass/fail property and has no business
//! failing CI on a noisy build agent. Run them deliberately:
//!
//! ```text
//! cargo test -p thunderforge_pg_sockets --release --test capacity_measurements \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Each test prints the figures it measured, so re-running it is how a number
//! in the document gets re-derived rather than re-copied.
//!
//! Printing *is* the output of a measurement, and the payload type exists to
//! be the right size rather than to be read, so the two lints that object to
//! exactly that are turned off for this file only.
#![allow(clippy::print_stdout, dead_code)]

use std::hint::black_box;
use std::time::Instant;

use chrono::NaiveDateTime;
use thunderforge_pg_sockets::{Relay, RowStamp, WORLD_CHANNEL_CAPACITY, WorldRouter};
use uuid::Uuid;

/// A payload the same shape and size as the server's `WorldEvent`.
///
/// The router is generic, so the only thing that makes its memory cost
/// concrete is the payload the server actually pushes through it. This
/// mirrors `src/server/src/models.rs:370` field for field; `token_event` is
/// the one heap-carrying field, and is filled with a realistic token-move
/// document rather than left empty.
#[derive(Clone, Debug)]
struct EventLike {
    id: i64,
    world_id: Uuid,
    event_code: i32,
    token_event: Option<serde_json::Value>,
    created_at: NaiveDateTime,
    schema_version: i32,
    updated_at: NaiveDateTime,
    created_by: Uuid,
    updated_by: Uuid,
}

fn sample_event(id: i64, world_id: Uuid) -> EventLike {
    let now = chrono::Utc::now().naive_utc();
    EventLike {
        id,
        world_id,
        event_code: 1,
        token_event: Some(serde_json::json!({
            "type": "token.moved",
            "tokenId": "9f1c1b2e-0f5a-4a1b-9d3e-2c7a1f0b4d55",
            "x": 128.5,
            "y": 64.25,
            "z": 0.0,
        })),
        created_at: now,
        schema_version: 1,
        updated_at: now,
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
    }
}

/// Resident set size in bytes, from `/proc/self/statm` (field 2, in pages).
fn rss_bytes() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").expect("/proc/self/statm");
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .expect("resident pages field")
        .parse()
        .expect("resident pages is a number");
    pages * 4096
}

fn world(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// What a world's channel and a subscriber on it actually cost in RSS.
///
/// Measured in two phases against the same process so the two costs can be
/// separated: phase A pays for `W` channels plus one receiver each, phase B
/// pays for nine more receivers per world and nothing else. RSS only grows,
/// so the deltas are attributable.
#[test]
#[ignore = "a measurement, not an assertion; see the module docs"]
fn per_world_and_per_subscriber_memory() {
    const WORLDS: usize = 1_000;
    const EXTRA_SUBSCRIBERS: usize = 9;

    // Warm the allocator and the page tables so the first phase is not paying
    // for process start-up.
    {
        let warm: WorldRouter<EventLike> = WorldRouter::new();
        let held: Vec<_> = (0..64).map(|n| warm.subscribe(world(n))).collect();
        black_box(&held);
    }

    let router: WorldRouter<EventLike> = WorldRouter::new();

    let base = rss_bytes();
    let mut first: Vec<_> = Vec::with_capacity(WORLDS);
    for n in 0..WORLDS {
        first.push(router.subscribe(world(n as u128 + 1_000)));
    }
    let after_a = rss_bytes();

    let mut rest: Vec<_> = Vec::with_capacity(WORLDS * EXTRA_SUBSCRIBERS);
    for n in 0..WORLDS {
        for _ in 0..EXTRA_SUBSCRIBERS {
            rest.push(router.subscribe(world(n as u128 + 1_000)));
        }
    }
    let after_b = rss_bytes();

    black_box(&first);
    black_box(&rest);

    let delta_a = after_a - base;
    let delta_b = after_b - after_a;
    let per_subscriber = delta_b as f64 / (WORLDS * EXTRA_SUBSCRIBERS) as f64;
    let per_world = (delta_a as f64 / WORLDS as f64) - per_subscriber;
    let total_subs = WORLDS * (1 + EXTRA_SUBSCRIBERS);

    println!("--- per-world / per-subscriber memory ---");
    println!(
        "payload size_of::<EventLike>()   = {}",
        size_of::<EventLike>()
    );
    println!("channel capacity                 = {WORLD_CHANNEL_CAPACITY}");
    println!("worlds                           = {WORLDS}");
    println!("subscribers                      = {total_subs}");
    println!("phase A delta (worlds + 1 sub)   = {delta_a} bytes");
    println!("phase B delta ({EXTRA_SUBSCRIBERS} more subs each) = {delta_b} bytes");
    println!("=> per subscriber                = {per_subscriber:.0} bytes");
    println!("=> per world channel             = {per_world:.0} bytes");
    println!(
        "=> total for {total_subs} subscribers over {WORLDS} worlds = {} bytes ({:.1} MiB)",
        after_b - base,
        (after_b - base) as f64 / (1024.0 * 1024.0)
    );
}

/// What one `publish` costs as a world's subscriber count grows.
///
/// The interesting quantity is per-subscriber, because that is the slope the
/// per-world topology was built to flatten (`router.rs` module docs).
#[test]
#[ignore = "a measurement, not an assertion; see the module docs"]
fn fan_out_cost_by_subscriber_count() {
    println!("--- fan-out cost of WorldRouter::publish ---");
    println!(
        "payload size_of::<EventLike>() = {}",
        size_of::<EventLike>()
    );

    for subscribers in [1usize, 10, 100, 1_000] {
        let router: WorldRouter<EventLike> = WorldRouter::new();
        let id = world(1);
        let mut held: Vec<_> = (0..subscribers).map(|_| router.subscribe(id)).collect();

        let iterations = 20_000usize;
        // Warm-up.
        for n in 0..2_000 {
            router.publish(id, sample_event(n, id));
        }
        for rx in &mut held {
            while rx.try_recv().is_ok() {}
        }

        let mut prepared: Vec<EventLike> = (0..iterations as i64)
            .map(|n| sample_event(n, id))
            .collect();

        let start = Instant::now();
        for event in prepared.drain(..) {
            black_box(router.publish(id, event));
        }
        let elapsed = start.elapsed();

        let per_publish = elapsed.as_nanos() as f64 / iterations as f64;
        let per_delivery = per_publish / subscribers as f64;
        println!(
            "{subscribers:>5} subscribers: {per_publish:>10.1} ns/publish  \
             {per_delivery:>8.1} ns/subscriber  \
             => {:>12.0} events/s sustained on one core",
            1_000_000_000.0 / per_publish
        );

        black_box(&held);
    }
}

/// What one poll's worth of rows costs the relay to absorb.
///
/// This is the per-poll CPU on the delivery loop's own path, separate from the
/// database round-trip: dedupe insert, cursor settle, prune.
#[test]
#[ignore = "a measurement, not an assertion; see the module docs"]
fn relay_absorb_cost_for_a_full_poll_batch() {
    const BATCH: i64 = 256; // POLL_BATCH_SIZE, src/server/src/network/listener.rs:60

    let now = chrono::Utc::now().naive_utc();
    // Settled rows: older than COMMIT_GRACE, so the cursor passes all of them,
    // which is the ordinary steady-state shape.
    let settled = now - chrono::Duration::seconds(60);

    let polls = 2_000i64;
    let mut relay = Relay::new(0);

    let start = Instant::now();
    for poll in 0..polls {
        let rows: Vec<RowStamp> = (0..BATCH)
            .map(|n| RowStamp {
                id: poll * BATCH + n + 1,
                created_at: settled,
            })
            .collect();
        black_box(relay.absorb(rows, now));
    }
    let elapsed = start.elapsed();

    let per_poll = elapsed.as_nanos() as f64 / polls as f64;
    println!("--- Relay::absorb, {BATCH} rows per poll ---");
    println!("{per_poll:.0} ns per full poll batch");
    println!("{:.1} ns per row", per_poll / BATCH as f64);
    println!(
        "one poll every 100ms => {:.4}% of one core spent in absorb",
        (per_poll / 100_000_000.0) * 100.0
    );
}

/// The same fan-out, but end to end on a runtime with receivers awaiting.
///
/// `fan_out_cost_by_subscriber_count` measures only the publisher's side, and
/// on its own it is misleading: `tokio::sync::broadcast` writes the value into
/// the ring **once** and each receiver clones it out on `recv`, so the
/// publisher's cost is flat in subscriber count and the real per-delivery work
/// — the clone, the waker, the task hop — lands on the consumer.
///
/// The publisher paces itself here, in rounds, waiting for every consumer to
/// drain each round before starting the next. Without that pacing the
/// publisher laps the 256-slot ring, consumers take `Lagged` and skip most of
/// the events, and the resulting "ns per delivery" is a measurement of
/// deliveries that never happened. The test asserts zero lag so that failure
/// mode cannot come back quietly.
#[test]
#[ignore = "a measurement, not an assertion; see the module docs"]
fn end_to_end_delivery_cost_by_subscriber_count() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::broadcast::error::RecvError;

    println!("--- end-to-end delivery (publisher + N awaiting receivers, paced) ---");
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!("runtime worker threads = {workers}");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("runtime");

    // Half the ring per round, so a round can never overrun the buffer even if
    // one consumer is a whole round behind.
    const ROUND: usize = WORLD_CHANNEL_CAPACITY / 2;
    const ROUNDS: usize = 200;

    for subscribers in [1usize, 10, 100, 1_000] {
        let (elapsed, lagged) = rt.block_on(async move {
            let router: Arc<WorldRouter<EventLike>> = Arc::new(WorldRouter::new());
            let id = world(1);
            let consumed = Arc::new(AtomicUsize::new(0));
            let lagged = Arc::new(AtomicUsize::new(0));
            let target = ROUND * ROUNDS;

            let mut tasks = Vec::with_capacity(subscribers);
            for _ in 0..subscribers {
                let mut rx = router.subscribe(id);
                let consumed = Arc::clone(&consumed);
                let lagged = Arc::clone(&lagged);
                tasks.push(tokio::spawn(async move {
                    let mut seen = 0usize;
                    while seen < target {
                        match rx.recv().await {
                            Ok(event) => {
                                black_box(event);
                                seen += 1;
                                consumed.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(RecvError::Lagged(n)) => {
                                lagged.fetch_add(n as usize, Ordering::Relaxed);
                                seen += n as usize;
                                consumed.fetch_add(n as usize, Ordering::Relaxed);
                            }
                            Err(RecvError::Closed) => break,
                        }
                    }
                }));
            }

            tokio::task::yield_now().await;

            let start = Instant::now();
            for round in 0..ROUNDS {
                for n in 0..ROUND as i64 {
                    router.publish(id, sample_event(n, id));
                }
                let due = (round + 1) * ROUND * subscribers;
                while consumed.load(Ordering::Relaxed) < due {
                    tokio::task::yield_now().await;
                }
            }
            let elapsed = start.elapsed();
            for task in tasks {
                let _ = task.await;
            }
            (elapsed, lagged.load(Ordering::Relaxed))
        });

        assert_eq!(
            lagged, 0,
            "the pacing must keep every consumer inside the ring"
        );

        let events = ROUND * ROUNDS;
        let deliveries = events * subscribers;
        let per_delivery = elapsed.as_nanos() as f64 / deliveries as f64;
        println!(
            "{subscribers:>5} subscribers x {events:>6} events = {deliveries:>9} deliveries in \
             {:>8.1} ms => {per_delivery:>7.1} ns/delivery ({:>12.0} deliveries/s, \
             {:>10.0} events/s)",
            elapsed.as_secs_f64() * 1_000.0,
            deliveries as f64 / elapsed.as_secs_f64(),
            events as f64 / elapsed.as_secs_f64(),
        );
    }
}

/// Worst-case per-world memory: a channel whose 256-slot ring is full.
///
/// The empty-channel figure understates the steady state of a busy table,
/// because each held slot also owns its payload's heap (`token_event` is a
/// `serde_json::Value`). This is the number that matters for "how many worlds
/// fit in RAM" when every one of them is mid-combat.
#[test]
#[ignore = "a measurement, not an assertion; see the module docs"]
fn per_world_memory_with_a_full_event_ring() {
    const WORLDS: usize = 500;

    {
        let warm: WorldRouter<EventLike> = WorldRouter::new();
        let held: Vec<_> = (0..64).map(|n| warm.subscribe(world(n))).collect();
        black_box(&held);
    }

    let router: WorldRouter<EventLike> = WorldRouter::new();
    let mut held = Vec::with_capacity(WORLDS);

    let base = rss_bytes();
    for n in 0..WORLDS {
        let id = world(n as u128 + 5_000);
        held.push(router.subscribe(id));
    }
    let empty = rss_bytes();

    for n in 0..WORLDS {
        let id = world(n as u128 + 5_000);
        for e in 0..WORLD_CHANNEL_CAPACITY as i64 {
            router.publish(id, sample_event(e, id));
        }
    }
    let full = rss_bytes();

    black_box(&held);

    println!("--- per-world memory, empty ring vs full ring ---");
    println!("worlds                      = {WORLDS}");
    println!("ring capacity               = {WORLD_CHANNEL_CAPACITY} events");
    println!(
        "empty channel               = {:.0} bytes/world",
        (empty - base) as f64 / WORLDS as f64
    );
    println!(
        "full ring (256 payloads)    = {:.0} bytes/world",
        (full - base) as f64 / WORLDS as f64
    );
    println!(
        "=> heap held by the payloads = {:.0} bytes/world ({:.0} bytes/event)",
        (full - empty) as f64 / WORLDS as f64,
        (full - empty) as f64 / (WORLDS * WORLD_CHANNEL_CAPACITY) as f64
    );
}
