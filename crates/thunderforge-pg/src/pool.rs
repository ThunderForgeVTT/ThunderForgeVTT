//! How big the connection pool should be, decided rather than defaulted.
//!
//! The sizing itself is one struct and a couple of rules, but it had been
//! nobody's decision: the server built its pool with `Pool::builder().build()`
//! and inherited r2d2's `max_size` of 10. Ten is not wrong so much as
//! unrelated — it was never chosen against this workload, where nearly every
//! database access happens inside `spawn_blocking` and holds a connection for
//! its duration, and where one busy world can have a hundred of those in
//! flight at once.
//!
//! Kept here, away from `r2d2` itself, so the rules can be tested as rules.

/// A pool configuration, as numbers rather than as a builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSizing {
    pub max_size: u32,
    pub min_idle: u32,
    pub connection_timeout_secs: u64,
}

/// The default when nothing is configured.
pub const DEFAULT_MAX_SIZE: u32 = 32;
/// Connections kept warm so early requests do not each pay for a handshake.
pub const DEFAULT_MIN_IDLE: u32 = 4;
/// Long enough to ride out a burst, short enough that a capacity problem
/// surfaces as itself rather than as a timeout cascade half a minute later.
pub const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 10;

/// Postgres' own default ceiling, shared by every connecting process.
///
/// Not enforced here — this crate cannot know how many instances will run —
/// but named so the arithmetic is visible at the point of decision:
/// `max_size × instances` must stay comfortably under it.
pub const POSTGRES_DEFAULT_MAX_CONNECTIONS: u32 = 100;

impl Default for PoolSizing {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_SIZE,
            min_idle: DEFAULT_MIN_IDLE,
            connection_timeout_secs: DEFAULT_CONNECTION_TIMEOUT_SECS,
        }
    }
}

/// Read the sizing from `DATABASE_POOL_MAX_SIZE`, falling back to the default.
///
/// A value that does not parse, or is zero, is ignored rather than honoured:
/// a pool of zero connections is a server that cannot answer anything, and
/// arriving there through a typo in an environment variable would be a
/// spectacularly confusing outage.
pub fn pool_sizing_from_env() -> PoolSizing {
    let configured = std::env::var("DATABASE_POOL_MAX_SIZE")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .filter(|size| *size > 0);

    PoolSizing::from_max_size(configured.unwrap_or(DEFAULT_MAX_SIZE))
}

impl PoolSizing {
    /// Build a sizing around a chosen `max_size`, keeping the invariants.
    pub fn from_max_size(max_size: u32) -> Self {
        let max_size = max_size.max(1);
        Self {
            max_size,
            // Never ask to keep more connections warm than the pool may hold;
            // r2d2 treats that as a configuration error and panics on build.
            min_idle: DEFAULT_MIN_IDLE.min(max_size),
            connection_timeout_secs: DEFAULT_CONNECTION_TIMEOUT_SECS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_a_deliberate_number_not_r2d2s() {
        let sizing = PoolSizing::default();
        assert_eq!(sizing.max_size, 32);
        assert_ne!(
            sizing.max_size, 10,
            "10 is the inherited default this replaced"
        );
    }

    /// `min_idle > max_size` makes r2d2 panic at startup.
    #[test]
    fn a_tiny_pool_does_not_ask_to_keep_more_connections_warm_than_it_holds() {
        let sizing = PoolSizing::from_max_size(2);
        assert_eq!(sizing.max_size, 2);
        assert!(sizing.min_idle <= sizing.max_size);
    }

    #[test]
    fn a_zero_max_size_is_refused_rather_than_producing_a_dead_server() {
        assert_eq!(PoolSizing::from_max_size(0).max_size, 1);
    }

    /// The number a reader has to be able to do in their head.
    #[test]
    fn the_default_leaves_room_for_more_than_one_instance() {
        let sizing = PoolSizing::default();
        assert!(
            sizing.max_size * 3 <= POSTGRES_DEFAULT_MAX_CONNECTIONS,
            "three instances at the default must still fit under Postgres' own ceiling"
        );
    }
}
