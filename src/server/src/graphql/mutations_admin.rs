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
    /// Subscriptions established.
    pub static OPENED: AtomicU64 = AtomicU64::new(0);
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

    /// `(sockets_open, opened, refused, delivered, lagged_events)`.
    pub fn snapshot() -> (i64, u64, u64, u64, u64) {
        (
            SOCKETS_OPEN.load(Ordering::Relaxed),
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
