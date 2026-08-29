//! Keep asking the database what is new, forever, and never die quietly.
//!
//! The loop itself is three lines of logic. Everything interesting here is
//! about what happens when the thing it calls misbehaves, because that is the
//! part that had no tests and the part that broke.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use thunderforge_pg_sockets::{Relay, Stamped};

/// Where rows come from.
///
/// Deliberately **blocking**, because the real implementation is diesel and
/// diesel blocks. Saying so in the type is what stops the loop from calling
/// it on an async worker thread by accident — which is exactly what the
/// server used to do.
pub trait EventSource: Send + Sync + 'static {
    type Row: Stamped + Send + 'static;

    /// Rows with `id > after`, ascending, bounded by some batch limit.
    fn poll(&self, after: i64) -> Result<Vec<Self::Row>, String>;

    /// The newest id that exists, or `None` if that cannot be read right now.
    fn high_water(&self) -> Option<i64>;
}

/// Where rows go.
pub trait EventSink: Send + Sync + 'static {
    type Row;

    /// Deliver one row; return how many subscribers it reached.
    fn publish(&self, row: Self::Row) -> usize;

    /// Drop any fan-out state nobody is listening to. Called once per pass.
    fn reap(&self) -> usize {
        0
    }
}

/// Counters that make a stall visible from outside the process.
///
/// `sent` alone is ambiguous when it stops: either nothing is being written,
/// or the loop reading it has died. Those are opposite faults in opposite
/// halves of the system. [`DeliveryMetrics::polls`] is what separates them,
/// and it exists because telling them apart once required attaching to a
/// frozen process.
#[derive(Debug, Default)]
pub struct DeliveryMetrics {
    /// Completed poll attempts, successful or not. Liveness of the loop.
    pub polls: AtomicU64,
    /// Rows handed to the sink.
    pub sent: AtomicU64,
    /// Rows that reached nobody. Ordinary, not a fault.
    pub dropped: AtomicU64,
    /// Polls that returned an error.
    pub errors: AtomicU64,
    /// Polls that panicked. Any non-zero value is a bug worth chasing.
    pub panics: AtomicU64,
    /// Polls abandoned for exceeding [`DeliveryConfig::poll_timeout`].
    pub timeouts: AtomicU64,
    /// Where the cursor has settled.
    pub cursor: AtomicI64,
}

/// Timings for the loop.
#[derive(Debug, Clone, Copy)]
pub struct DeliveryConfig {
    /// How long to wait between polls.
    pub poll_interval: Duration,
    /// How long a single poll may take before it is abandoned.
    ///
    /// A blocking query has no deadline of its own: a connection whose peer
    /// has vanished can leave a read outstanding indefinitely, and a loop that
    /// waits on it stops forever without logging anything. Abandoning the wait
    /// keeps delivery alive at the cost of leaking the blocking thread the
    /// stuck query is on — a trade worth making, because a leaked thread is
    /// visible and recoverable and a silently dead backplane is neither.
    pub poll_timeout: Duration,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            poll_timeout: Duration::from_secs(15),
        }
    }
}

/// Establish the starting cursor, waiting for the database if it is not ready.
///
/// Never guesses. Starting from 0 replays the whole table to every client;
/// starting from `i64::MAX` delivers nothing ever, which is a real bug this
/// codebase has already shipped once.
async fn wait_for_high_water<S: EventSource>(source: &Arc<S>, cfg: &DeliveryConfig) -> i64 {
    loop {
        let probe = source.clone();
        let result = tokio::task::spawn_blocking(move || probe.high_water())
            .await
            .unwrap_or(None);
        if let Some(id) = result {
            return id;
        }
        tokio::time::sleep(cfg.poll_interval.max(Duration::from_millis(500))).await;
    }
}

/// Run the delivery loop until the task is dropped.
///
/// Returns only if `stop` is signalled, which exists for tests; in the server
/// this never returns.
pub async fn run_delivery<S, K>(
    source: Arc<S>,
    sink: Arc<K>,
    metrics: Arc<DeliveryMetrics>,
    cfg: DeliveryConfig,
    mut stop: tokio::sync::watch::Receiver<bool>,
) where
    S: EventSource,
    K: EventSink<Row = S::Row>,
{
    let start_after = wait_for_high_water(&source, &cfg).await;
    let mut relay = Relay::new(start_after);
    metrics.cursor.store(relay.cursor(), Ordering::Relaxed);

    loop {
        if *stop.borrow_and_update() {
            return;
        }

        let cursor = relay.cursor();
        let polling = source.clone();
        // On a blocking thread, always. The call blocks on a connection
        // checkout and then on a socket, and doing that on a runtime worker
        // stalls every other task scheduled there.
        let handle = tokio::task::spawn_blocking(move || polling.poll(cursor));

        let outcome = tokio::time::timeout(cfg.poll_timeout, handle).await;

        // Counted before interpreting the result: this number answers "is the
        // loop still running", which is a different question from "did the
        // query work".
        metrics.polls.fetch_add(1, Ordering::Relaxed);

        match outcome {
            Ok(Ok(Ok(rows))) => {
                let publish = relay.absorb(rows, chrono::Utc::now().naive_utc());
                metrics.cursor.store(relay.cursor(), Ordering::Relaxed);

                for row in publish {
                    let reached = sink.publish(row);
                    metrics.sent.fetch_add(1, Ordering::Relaxed);
                    if reached == 0 {
                        // Nobody has this world open. The row is already
                        // durable; this was the live nudge, not the record.
                        metrics.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }

                sink.reap();
            }
            Ok(Ok(Err(_message))) => {
                metrics.errors.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Err(_join_error)) => {
                // The poll panicked. Before this loop existed as its own
                // thing, a panic on the polling path ended the delivery task
                // outright: no restart, no error, the backplane simply gone
                // while the process stayed up and healthy-looking.
                metrics.panics.fetch_add(1, Ordering::Relaxed);
            }
            Err(_elapsed) => {
                // The poll is still running and may never finish. We stop
                // waiting on it rather than letting one stuck query end live
                // delivery for every player on the server.
                metrics.timeouts.fetch_add(1, Ordering::Relaxed);
            }
        }

        tokio::time::sleep(cfg.poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use thunderforge_pg_sockets::RowStamp;

    /// A source whose behaviour each test dictates.
    struct Scripted {
        /// What `poll` should do, consumed one entry per call; the last entry
        /// repeats once exhausted.
        script: Mutex<Vec<Behaviour>>,
        calls: AtomicU64,
    }

    #[derive(Clone)]
    enum Behaviour {
        Rows(Vec<RowStamp>),
        Fail,
        Panic,
        Hang,
    }

    impl Scripted {
        fn new(script: Vec<Behaviour>) -> Arc<Self> {
            Arc::new(Self {
                script: Mutex::new(script),
                calls: AtomicU64::new(0),
            })
        }
    }

    impl EventSource for Scripted {
        type Row = RowStamp;

        fn poll(&self, after: i64) -> Result<Vec<Self::Row>, String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
            let script = self.script.lock().unwrap();
            let behaviour = script
                .get(n)
                .or_else(|| script.last())
                .cloned()
                .unwrap_or(Behaviour::Rows(Vec::new()));
            drop(script);

            match behaviour {
                Behaviour::Rows(rows) => {
                    Ok(rows.into_iter().filter(|r| r.id > after).take(8).collect())
                }
                Behaviour::Fail => Err("database went away".into()),
                Behaviour::Panic => panic!("the poll panicked"),
                Behaviour::Hang => {
                    // Long enough to blow well past `poll_timeout` (50ms in
                    // these tests), short enough that the test binary exits.
                    //
                    // Deliberately not an unbounded sleep. Abandoning the
                    // *wait* does not cancel the blocking task — nothing can —
                    // and tokio joins outstanding blocking tasks at shutdown,
                    // so `sleep(an hour)` here hangs the entire test run
                    // rather than failing it. That is the same property the
                    // production loop documents when it trades a leaked
                    // thread for a live backplane; in a test it buys a hung
                    // CI job instead of an outage.
                    std::thread::sleep(Duration::from_millis(400));
                    Ok(Vec::new())
                }
            }
        }

        fn high_water(&self) -> Option<i64> {
            Some(0)
        }
    }

    #[derive(Default)]
    struct Collecting {
        seen: Mutex<Vec<i64>>,
    }

    impl EventSink for Collecting {
        type Row = RowStamp;

        fn publish(&self, row: Self::Row) -> usize {
            self.seen.lock().unwrap().push(row.id);
            1
        }
    }

    fn settled(id: i64) -> RowStamp {
        RowStamp {
            id,
            created_at: chrono::Utc::now().naive_utc() - chrono::Duration::seconds(60),
        }
    }

    /// Drive the loop for a bounded number of passes, then stop it.
    async fn run_for(
        source: Arc<Scripted>,
        sink: Arc<Collecting>,
        passes: u64,
    ) -> Arc<DeliveryMetrics> {
        let metrics = Arc::new(DeliveryMetrics::default());
        let (tx, rx) = tokio::sync::watch::channel(false);
        let cfg = DeliveryConfig {
            poll_interval: Duration::from_millis(10),
            poll_timeout: Duration::from_millis(50),
        };

        let task = tokio::spawn(run_delivery(source, sink, metrics.clone(), cfg, rx));

        // Wait for the loop to make the requested progress rather than
        // sleeping a guessed duration.
        // Never wait indefinitely: a delivery loop that has stopped is exactly
        // what these tests exist to catch, and catching it as a hung test
        // binary would tell a reader nothing about what broke.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while metrics.polls.load(Ordering::Relaxed) < passes {
            assert!(
                std::time::Instant::now() < deadline,
                "the delivery loop stopped: only {} of {passes} polls completed",
                metrics.polls.load(Ordering::Relaxed)
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
        metrics
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rows_are_published_once_each() {
        let source = Scripted::new(vec![Behaviour::Rows(vec![
            settled(1),
            settled(2),
            settled(3),
        ])]);
        let sink = Arc::new(Collecting::default());

        let metrics = run_for(source, sink.clone(), 4).await;

        assert_eq!(*sink.seen.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(metrics.sent.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.cursor.load(Ordering::Relaxed), 3);
    }

    /// A panicking poll must cost one pass, not the entire backplane.
    ///
    /// This is the failure that motivated the whole crate. A panic on the
    /// polling path ended the delivery task with no log and no restart: rows
    /// kept committing, HTTP kept answering, and every client silently stopped
    /// receiving anything for the life of the process.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_panicking_poll_does_not_stop_delivery() {
        let source = Scripted::new(vec![
            Behaviour::Panic,
            Behaviour::Rows(vec![settled(1), settled(2)]),
        ]);
        let sink = Arc::new(Collecting::default());

        let metrics = run_for(source, sink.clone(), 5).await;

        assert_eq!(
            metrics.panics.load(Ordering::Relaxed),
            1,
            "the panic was seen"
        );
        assert!(
            metrics.polls.load(Ordering::Relaxed) >= 5,
            "the loop kept polling after the panic"
        );
        assert_eq!(
            *sink.seen.lock().unwrap(),
            vec![1, 2],
            "delivery resumed on the next pass"
        );
    }

    /// A poll that never returns must not become a permanent outage.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_hanging_poll_is_abandoned_and_the_loop_continues() {
        let source = Scripted::new(vec![Behaviour::Hang, Behaviour::Rows(vec![settled(1)])]);
        let sink = Arc::new(Collecting::default());

        let metrics = run_for(source, sink.clone(), 4).await;

        assert!(
            metrics.timeouts.load(Ordering::Relaxed) >= 1,
            "the stuck poll was abandoned rather than waited on forever"
        );
        assert_eq!(
            *sink.seen.lock().unwrap(),
            vec![1],
            "the next poll delivered normally"
        );
    }

    /// An unreachable database is a condition to survive, not to die of.
    #[tokio::test(flavor = "multi_thread")]
    async fn polls_that_error_are_counted_and_retried() {
        let source = Scripted::new(vec![
            Behaviour::Fail,
            Behaviour::Fail,
            Behaviour::Rows(vec![settled(7)]),
        ]);
        let sink = Arc::new(Collecting::default());

        let metrics = run_for(source, sink.clone(), 5).await;

        assert_eq!(metrics.errors.load(Ordering::Relaxed), 2);
        assert_eq!(*sink.seen.lock().unwrap(), vec![7]);
    }

    /// The liveness counter must keep moving even when nothing is happening,
    /// because that is the whole point of it.
    #[tokio::test(flavor = "multi_thread")]
    async fn polls_advance_even_when_there_is_nothing_to_deliver() {
        let source = Scripted::new(vec![Behaviour::Rows(Vec::new())]);
        let sink = Arc::new(Collecting::default());

        let metrics = run_for(source, sink.clone(), 6).await;

        assert!(metrics.polls.load(Ordering::Relaxed) >= 6);
        assert_eq!(metrics.sent.load(Ordering::Relaxed), 0);
        assert!(sink.seen.lock().unwrap().is_empty());
    }

    /// A backlog deeper than one batch has to drain across passes.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_burst_larger_than_one_batch_drains_over_successive_polls() {
        let rows: Vec<RowStamp> = (1..=20).map(settled).collect();
        let source = Scripted::new(vec![Behaviour::Rows(rows)]);
        let sink = Arc::new(Collecting::default());

        // The scripted source caps each poll at 8 rows, so 20 needs 3 passes.
        let metrics = run_for(source, sink.clone(), 6).await;

        assert_eq!(*sink.seen.lock().unwrap(), (1..=20).collect::<Vec<_>>());
        assert_eq!(metrics.cursor.load(Ordering::Relaxed), 20);
    }
}
