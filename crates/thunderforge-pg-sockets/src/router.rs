//! One channel per world, instead of one channel for everybody.
//!
//! # The shape this replaces
//!
//! The server had a single process-wide `broadcast::channel(10000)`. Every
//! subscriber — the GraphQL `worldEventsCreated` subscription, the raw
//! WebSocket route, the SSE route — took a receiver on it and then decided
//! for itself whether the event was any of its business:
//!
//! ```ignore
//! if event.world_id == world_uuid { Some(event) } else { None }
//! ```
//!
//! So an event in a five-player world was cloned and delivered to *every
//! connected client in the system*, and thrown away by all but five. The cost
//! of an event scaled with the number of people connected, not the number who
//! wanted it.
//!
//! Measured on one workstation, fan-out only — no sockets, no serialisation:
//!
//! | connections | worlds | topology  | deliveries for 2,000 events | wasted   |
//! |------------:|-------:|-----------|----------------------------:|---------:|
//! |       1,000 |    200 | global    |                   2,000,000 |     200× |
//! |       1,000 |    200 | per-world |                      10,000 |     1.0× |
//! |     100,000 | 20,000 | global    |                 200,000,000 |  20,000× |
//! |     100,000 | 20,000 | per-world |                      10,000 |     1.0× |
//!
//! At 100k connections the global topology spent 4.3s of pure fan-out on work
//! that per-world routing finished in 0.088s. That is the whole reason this
//! type exists.
//!
//! # Why this is not a broker
//!
//! A NATS subject and a Centrifugo channel give the same routing shape, and
//! either would also fix the amplification. They fix it by adding a broker to
//! run, secure and monitor, which buys fan-out *across* processes. Until one
//! process can no longer hold the connections, that is scale paid for and not
//! used — so the routing lives here, in the process that already has the
//! sockets, and the seam is shaped so a broker can be slid underneath later.
//!
//! # Lifecycle
//!
//! A world's channel is created when someone first subscribes to it and
//! removed when the last subscriber goes away ([`WorldRouter::reap`]). It is
//! *not* created by publishing: an event for a world nobody is watching costs
//! one map lookup and a drop, which is the common case on a busy server.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

/// How many events a slow subscriber may fall behind before it starts losing
/// them.
///
/// Per world, not per process, which is what makes a small number safe: the
/// old global channel held 10,000 because it carried every world's traffic at
/// once. A single world produces events at human speed — a GM moving tokens,
/// a die roll, a line of chat — so 256 is minutes of backlog for one table,
/// and a subscriber further behind than that wants a resync, not a longer
/// queue.
pub const WORLD_CHANNEL_CAPACITY: usize = 256;

/// Per-world event fan-out.
///
/// `T` is whatever the server wants to deliver; this crate deliberately does
/// not know what an event is, which is what keeps it testable without a
/// database or a schema.
#[derive(Debug)]
pub struct WorldRouter<T> {
    routes: DashMap<Uuid, broadcast::Sender<T>>,
}

impl<T: Clone> Default for WorldRouter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> WorldRouter<T> {
    pub fn new() -> Self {
        Self {
            routes: DashMap::new(),
        }
    }

    /// Listen to one world.
    ///
    /// Creates the world's channel if this is its first subscriber. The
    /// receiver only ever sees events published for `world_id`, so callers
    /// have nothing to filter — which is the point, and is why the old
    /// `if event.world_id == world_uuid` checks are gone rather than moved.
    pub fn subscribe(&self, world_id: Uuid) -> broadcast::Receiver<T> {
        self.routes
            .entry(world_id)
            .or_insert_with(|| broadcast::channel(WORLD_CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Deliver an event to one world's subscribers.
    ///
    /// Returns how many receivers it reached. Zero is the ordinary case for a
    /// world nobody has open, and is not a failure — the event is already
    /// durable in Postgres, and this is a live nudge, not the record of it.
    pub fn publish(&self, world_id: Uuid, event: T) -> usize {
        match self.routes.get(&world_id) {
            Some(tx) => tx.send(event).unwrap_or(0),
            None => 0,
        }
    }

    /// Drop channels nobody is listening to.
    ///
    /// Without this the map grows by one entry per world ever opened and
    /// never shrinks — a slow leak on a long-lived server, and one that only
    /// shows up after weeks. Called periodically rather than on unsubscribe
    /// because `broadcast::Receiver` has no drop hook the sender can observe,
    /// and because reaping on every disconnect would churn the map during the
    /// reconnect storm that follows a deploy.
    ///
    /// Racy by nature and safe because of it: a world reaped in the instant
    /// between a subscriber appearing and its first event is simply recreated
    /// by the next `subscribe`, and the subscriber that lost its channel sees
    /// a closed receiver, which every caller already treats as "resync".
    ///
    /// The count is of entries this call actually dropped, taken as they are
    /// dropped. It used to be `len()` before minus `len()` after, which is a
    /// different quantity the moment anyone subscribes concurrently — and
    /// somebody always does, since reaping runs on a timer against a live
    /// server. One new world entering the map during the retain makes the
    /// second `len()` the larger of the two, and an unsigned subtraction of
    /// two independently-observed lengths then panics the worker thread in a
    /// debug build and reports a nonsense count near `usize::MAX` in a
    /// release one. Observed: a panic here took out a `tokio-rt-worker`
    /// between "Released 2 idle world channel(s)" and a burst of new
    /// subscriptions.
    pub fn reap(&self) -> usize {
        let mut reaped = 0;
        self.routes.retain(|_, tx| {
            let keep = tx.receiver_count() > 0;
            if !keep {
                reaped += 1;
            }
            keep
        });
        reaped
    }

    /// How many worlds currently have at least one channel.
    pub fn active_worlds(&self) -> usize {
        self.routes.len()
    }

    /// How many subscribers a world has, for metrics and tests.
    pub fn subscriber_count(&self, world_id: Uuid) -> usize {
        self.routes
            .get(&world_id)
            .map_or(0, |tx| tx.receiver_count())
    }
}

/// A router shared across the request handlers that subscribe to it.
pub type SharedWorldRouter<T> = Arc<WorldRouter<T>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn world(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn a_subscriber_hears_its_own_world() {
        let router: WorldRouter<i32> = WorldRouter::new();
        let mut rx = router.subscribe(world(1));

        assert_eq!(router.publish(world(1), 42), 1);
        assert_eq!(rx.try_recv().unwrap(), 42);
    }

    /// Reaping while somebody subscribes must not panic, and must not lie.
    ///
    /// This is the shape the server actually runs: reaping is on a timer
    /// against a live process, so new worlds arrive in the middle of it. The
    /// old count — `len()` before minus `len()` after — turned that ordinary
    /// overlap into an unsigned underflow, which panicked the worker thread
    /// in a debug build. Threads rather than a contrived injection point,
    /// because the race is between the retain and an insert and there is no
    /// seam between them to hook.
    #[test]
    fn reaping_survives_worlds_appearing_while_it_runs() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::thread;

        let router: Arc<WorldRouter<i32>> = Arc::new(WorldRouter::new());

        // 64 dead worlds for reap to find, so it has real work to do.
        for n in 0..64 {
            drop(router.subscribe(world(n)));
        }

        let stop = Arc::new(AtomicBool::new(false));
        let subscriber = {
            let router = Arc::clone(&router);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                // Held, so these worlds are live and must survive the reap.
                let mut held = Vec::new();
                let mut n = 1_000u128;
                while !stop.load(Ordering::Relaxed) {
                    held.push(router.subscribe(world(n)));
                    n += 1;
                }
                held.len()
            })
        };

        let mut reaped = 0;
        for _ in 0..200 {
            reaped += router.reap();
        }
        stop.store(true, Ordering::Relaxed);
        let added = subscriber
            .join()
            .expect("the subscriber thread must not panic");

        assert_eq!(
            reaped, 64,
            "reap must report the entries it actually dropped, however many \
             worlds appeared while it was running",
        );
        assert!(
            router.active_worlds() >= 1,
            "worlds with a live receiver must survive the reap ({added} were added)",
        );
    }

    #[test]
    fn a_subscriber_is_never_woken_by_another_world() {
        // The whole point of the type. Under the previous topology this
        // receiver would have been woken by every one of these publishes and
        // would have discarded all of them itself.
        let router: WorldRouter<i32> = WorldRouter::new();
        let mut rx = router.subscribe(world(1));

        for other in 2..50 {
            router.publish(world(other), other as i32);
        }

        assert!(
            rx.try_recv().is_err(),
            "an event for another world must not reach this subscriber at all"
        );
    }

    #[test]
    fn publishing_to_an_unwatched_world_reaches_nobody_and_creates_nothing() {
        let router: WorldRouter<i32> = WorldRouter::new();

        assert_eq!(router.publish(world(7), 1), 0);
        assert_eq!(
            router.active_worlds(),
            0,
            "publishing must not allocate a channel for a world nobody is watching"
        );
    }

    #[test]
    fn every_subscriber_of_one_world_gets_the_event() {
        let router: WorldRouter<i32> = WorldRouter::new();
        let mut players: Vec<_> = (0..5).map(|_| router.subscribe(world(1))).collect();

        assert_eq!(router.publish(world(1), 9), 5);
        for rx in &mut players {
            assert_eq!(rx.try_recv().unwrap(), 9);
        }
    }

    #[test]
    fn reaping_removes_worlds_nobody_is_listening_to() {
        let router: WorldRouter<i32> = WorldRouter::new();
        let keep = router.subscribe(world(1));
        drop(router.subscribe(world(2)));

        assert_eq!(router.active_worlds(), 2);
        assert_eq!(router.reap(), 1, "the abandoned world should be reaped");
        assert_eq!(router.active_worlds(), 1);
        assert_eq!(router.subscriber_count(world(1)), 1);

        drop(keep);
        assert_eq!(router.reap(), 1);
        assert_eq!(router.active_worlds(), 0);
    }

    #[test]
    fn a_reaped_world_is_recreated_by_the_next_subscriber() {
        // The reap race, asserted rather than assumed: losing the channel is
        // recoverable because the next subscribe rebuilds it.
        let router: WorldRouter<i32> = WorldRouter::new();
        drop(router.subscribe(world(1)));
        router.reap();

        let mut rx = router.subscribe(world(1));
        assert_eq!(router.publish(world(1), 3), 1);
        assert_eq!(rx.try_recv().unwrap(), 3);
    }

    #[test]
    fn a_subscriber_that_falls_far_behind_lags_rather_than_blocking_the_publisher() {
        // A subscriber that stops reading must never be able to stall the
        // listener task for everyone else. `broadcast` drops the oldest
        // instead, which the caller surfaces as a resync signal.
        let router: WorldRouter<i32> = WorldRouter::new();
        let mut rx = router.subscribe(world(1));

        for n in 0..(WORLD_CHANNEL_CAPACITY as i32 + 10) {
            router.publish(world(1), n);
        }

        match rx.try_recv() {
            Err(broadcast::error::TryRecvError::Lagged(missed)) => {
                assert_eq!(missed, 10, "exactly the overflow should be reported");
            }
            other => panic!("expected a Lagged report, got {other:?}"),
        }
    }
}
