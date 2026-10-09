//! Instance administration: the mutations only a server operator may call.

use async_graphql::{Context, Error, Result as GraphQLResult};

use super::*;

#[derive(Default)]
pub struct AdminMutation;

#[async_graphql::Object]
impl AdminMutation {
    async fn update_oauth_provider(
        &self,
        ctx: &Context<'_>,
        provider_id: uuid::Uuid,
        config: GraphQLOAuthProviderConfigInput,
    ) -> GraphQLResult<GraphQLOAuthProvider> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_oauth_provider(state, provider_id, config.into())
            .await
            .map(GraphQLOAuthProvider::from)
            .map_err(Error::new)?;

        Ok(result)
    }

    /// Prove the storage answers before an operator walks away from them.
    ///
    /// `ensure_bucket` first, then `health_check`: a bucket that does not
    /// exist yet is the *expected* state on a fresh instance, and reporting
    /// "unreachable" for it would send an operator to re-read credentials
    /// that were right all along. Together they answer the only question
    /// worth asking here — can this instance store a map and serve it back.
    ///
    /// Deliberately not a write-then-delete probe. `delete_object` refuses
    /// any key outside the feedback prefix, so a probe object would be
    /// litter this server cannot clear up.
    async fn test_storage_connection(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLStorageConnectionReport> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;

        let cfg = crate::storage::rustfs::RustFsConfig::resolve(state).await;
        let outcome = match crate::storage::rustfs::ensure_bucket(&cfg).await {
            Ok(()) => crate::storage::rustfs::health_check(&cfg).await,
            Err(error) => Err(error),
        };

        Ok(GraphQLStorageConnectionReport {
            reachable: outcome.is_ok(),
            endpoint: cfg.endpoint,
            bucket: cfg.bucket,
            // The store's own words, not a translation of them. An S3 error
            // names which of the four answers was wrong far better than
            // "could not connect" does.
            detail: outcome.err().map(|error| error.to_string()),
        })
    }

    async fn update_manifest_key(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_manifest_key(state, &key, &value)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)?;

        Ok(result)
    }

    async fn recalculate_disk_usage(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLAdminStats> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let stats = load_admin_stats(state).await.map_err(Error::new)?;
        let disk_usage = calculate_disk_usage(state).map_err(Error::new)?;

        Ok(GraphQLAdminStats {
            disk_usage_bytes: disk_usage.total_bytes,
            disk_usage: disk_usage.into(),
            total_users: stats.total_users,
            total_worlds: stats.total_worlds,
            total_world_tokens: stats.total_world_tokens,
            total_world_events: stats.total_world_events,
            total_policies: stats.total_policies,
        })
    }

    async fn update_two_factor_policy(
        &self,
        ctx: &Context<'_>,
        required_for_all_users: bool,
    ) -> GraphQLResult<GraphQLAuthSecuritySettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let result = persist_two_factor_policy(state, required_for_all_users)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)?;

        Ok(result)
    }
}

/// Counters for the subscription hot path, kept instead of a log line per
/// event.
///
/// # Why this is not just tidiness
///
/// `eprintln!` takes a lock and issues a **blocking** `write(2)`. When stderr
/// is a pipe — which it is in every container, every CI harness and every
/// `cargo run | tee` — a consumer that stops reading for a moment fills the
/// 64KiB pipe buffer, and every one of those writes then blocks the thread it
/// is on until the reader comes back. These writes were happening on the
/// tokio worker threads that carry the subscriptions themselves, once per
/// event **per subscriber**, so a single slow log reader could stall the
/// whole fan-out at once.
///
/// That is not hypothetical: it is the mechanism behind the torture suite's
/// worst run. `scripts/marketing-metrics.mjs` reads the run's output through
/// a pipe and blocked its own event loop on a synchronous `docker stats`
/// every two seconds. With one line per event per subscriber the pipe filled,
/// the server's subscription tasks blocked in `write`, and 11 of 25
/// subscribers received nothing at all — with no panic, no error and no
/// timeout anywhere, because nothing was broken, only stopped. The identical
/// tier run through a file instead of that pipe passed 5/5.
///
/// So the hot path counts and the periodic reporter in
/// `network::listener` prints the totals once every ten seconds. Bounded log
/// volume is the property that matters here, not brevity: a diagnostic that
/// can stop delivery is worse than no diagnostic.
pub mod subscription_metrics {
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

    /// Events handed to a subscriber's socket.
    pub static DELIVERED: AtomicU64 = AtomicU64::new(0);
    /// Subscriptions established, ever. Cumulative: it only goes up.
    ///
    /// The metrics line printed this as `subs_open`, which read as a live
    /// count. On vtt-dev it climbed to 168 with six sockets attached and was
    /// taken for a leak; it was counting opens, and every one of them had
    /// been released. [`OPEN`] is the live number.
    pub static OPENED: AtomicU64 = AtomicU64::new(0);
    /// Subscriptions open right now: raised by [`OpenSubscription::begin`],
    /// lowered when that guard drops.
    ///
    /// The guard lives inside the subscription's stream, so the decrement
    /// does not depend on how the stream ends — a client `complete`, the
    /// socket closing, the stream finishing on its own, or a panic unwinding
    /// through it. A count lowered on only one of those paths would climb
    /// for ever while nothing leaked.
    pub static OPEN: AtomicI64 = AtomicI64::new(0);
    /// Subscriptions refused (no app state, bad id, not a member).
    pub static REFUSED: AtomicU64 = AtomicU64::new(0);
    /// Events a subscriber lost by falling behind the broadcast buffer.
    pub static LAGGED_EVENTS: AtomicU64 = AtomicU64::new(0);
    /// WebSocket connections currently being served.
    ///
    /// Live rather than cumulative on purpose. "How many sockets are attached
    /// right now" is the number that separates *the server stopped sending*
    /// from *the clients went away*, and telling those two apart is what the
    /// worst delivery investigation in this repository spent its time on.
    pub static SOCKETS_OPEN: AtomicI64 = AtomicI64::new(0);

    #[cfg(test)]
    thread_local! {
        /// This thread's share of [`OPEN`], so a test can assert on its own
        /// subscriptions while other tests open theirs in parallel.
        static OPEN_ON_THIS_THREAD: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    }

    /// Subscriptions opened and not yet released on the calling thread.
    #[cfg(test)]
    pub fn open_on_this_thread() -> i64 {
        OPEN_ON_THIS_THREAD.with(std::cell::Cell::get)
    }

    fn adjust_open(by: i64) {
        OPEN.fetch_add(by, Ordering::Relaxed);
        #[cfg(test)]
        OPEN_ON_THIS_THREAD.with(|open| open.set(open.get() + by));
    }

    /// One subscription, counted in [`OPEN`] for as long as this lives.
    ///
    /// Move it into the stream with [`OpenSubscription::hold`]; dropping the
    /// stream is then the only way to release it, and every way a stream
    /// ends drops it.
    #[must_use = "the subscription counts as open only while this is held"]
    pub struct OpenSubscription(());

    impl OpenSubscription {
        /// Count a subscription opened, both cumulatively and live.
        pub fn begin() -> Self {
            OPENED.fetch_add(1, Ordering::Relaxed);
            adjust_open(1);
            Self(())
        }

        /// The same stream, carrying this guard until it is dropped.
        pub fn hold<S: futures_util::Stream>(self, inner: S) -> Held<S> {
            Held {
                inner: Box::pin(inner),
                _open: self,
            }
        }
    }

    impl Drop for OpenSubscription {
        fn drop(&mut self) {
            adjust_open(-1);
        }
    }

    /// A subscription stream that counts itself open until dropped.
    pub struct Held<S> {
        inner: std::pin::Pin<Box<S>>,
        _open: OpenSubscription,
    }

    impl<S: futures_util::Stream> futures_util::Stream for Held<S> {
        type Item = S::Item;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<S::Item>> {
            self.inner.as_mut().poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.inner.size_hint()
        }
    }

    /// One served WebSocket, counted in [`SOCKETS_OPEN`] until dropped — so
    /// a connection task that panics is still uncounted as it unwinds.
    #[must_use = "the socket counts as open only while this is held"]
    pub struct OpenSocket(());

    impl OpenSocket {
        pub fn begin() -> Self {
            SOCKETS_OPEN.fetch_add(1, Ordering::Relaxed);
            Self(())
        }
    }

    impl Drop for OpenSocket {
        fn drop(&mut self) {
            SOCKETS_OPEN.fetch_sub(1, Ordering::Relaxed);
        }
    }

    static SINCE: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    static LAST_LAG_LOG_MS: AtomicU64 = AtomicU64::new(0);

    /// Whether to print a lag line now, at most one every ten seconds.
    ///
    /// Lag is worth a sentence in the log — it means a client's view of the
    /// world is wrong — but it is not worth one per event: a subscriber that
    /// has wedged lags on *every* subsequent event, which is exactly the
    /// runaway volume this module exists to prevent. The count in the
    /// periodic report is the complete number; the line is there so somebody
    /// grepping finds it at all.
    pub fn should_log_lag() -> bool {
        let now = SINCE.elapsed().as_millis() as u64;
        let last = LAST_LAG_LOG_MS.load(Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < 10_000 {
            return false;
        }
        LAST_LAG_LOG_MS
            .compare_exchange(last, now.max(1), Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }

    /// `(sockets_open, open, opened, refused, delivered, lagged_events)`.
    pub fn snapshot() -> (i64, i64, u64, u64, u64, u64) {
        (
            SOCKETS_OPEN.load(Ordering::Relaxed),
            OPEN.load(Ordering::Relaxed),
            OPENED.load(Ordering::Relaxed),
            REFUSED.load(Ordering::Relaxed),
            DELIVERED.load(Ordering::Relaxed),
            LAGGED_EVENTS.load(Ordering::Relaxed),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The cap has to hold for a subscriber that lags on every event.
        ///
        /// This is the shape that made the unrate-limited version dangerous:
        /// a wedged client does not lag once, it lags on everything that
        /// arrives afterwards, so "one line per lag" is one blocking write to
        /// stderr per event for as long as it stays wedged — the same runaway
        /// volume, arriving by a different door.
        #[test]
        fn the_lag_line_is_capped_however_many_times_lag_is_reported() {
            // The first report is always worth printing; the flood behind it
            // is not.
            assert!(should_log_lag(), "the first lag must be findable");
            let printed = (0..10_000).filter(|_| should_log_lag()).count();
            assert_eq!(
                printed, 0,
                "ten thousand further lag reports inside the window must \
                 print nothing; {printed} got through",
            );
        }
    }
}
