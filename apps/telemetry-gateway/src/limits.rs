//! The token buckets (FR-042, FR-043, R28).
//!
//! An address is read, hashed with a key made once per process and dropped:
//! only the hash keys a bucket, and the key dies with the process. Both maps
//! are bounded, and evict buckets idle long enough to be full again.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::hash::RandomState;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use thunderforge_telemetry_policy::TokenBucket;

/// The time, passed in so a test can move it by hand.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(Instant::now)
}

/// The client's address: the entry `trusted_hops` from the right of
/// `X-Forwarded-For` (the leftmost when there are fewer), or the TCP peer
/// when there is no header or `trusted_hops` is 0.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, trusted_hops: usize) -> Option<IpAddr> {
    if trusted_hops == 0 {
        return peer;
    }
    let entries: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if entries.is_empty() {
        return peer;
    }
    let at = entries.len().saturating_sub(trusted_hops);
    entries[at].parse().ok().or(peer)
}

struct Buckets {
    map: HashMap<u64, TokenBucket>,
    max: usize,
    rate: f64,
    burst: u32,
    last_sweep: Option<Instant>,
}

impl Buckets {
    fn new(max: usize, rate: f64, burst: u32) -> Self {
        Self {
            map: HashMap::new(),
            max: max.max(1),
            rate,
            burst,
            last_sweep: None,
        }
    }

    /// How long a full map makes a new key wait: one full refill.
    fn full_wait(&self) -> Duration {
        Duration::from_secs_f64(f64::from(self.burst.max(1)) / self.rate.max(f64::MIN_POSITIVE))
    }

    fn take(&mut self, key: u64, now: Instant) -> Result<(), Duration> {
        if let Some(bucket) = self.map.get_mut(&key) {
            return bucket.try_take(now);
        }
        if self.map.len() >= self.max {
            // At most one sweep a second, so a flood of new keys cannot make
            // every request walk the whole map.
            let due = self
                .last_sweep
                .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(1));
            if due {
                self.map.retain(|_, b| !b.is_idle(now));
                self.last_sweep = Some(now);
            }
            if self.map.len() >= self.max {
                return Err(self.full_wait());
            }
        }
        let mut bucket = TokenBucket::new(self.rate, self.burst, now);
        let taken = bucket.try_take(now);
        self.map.insert(key, bucket);
        taken
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

pub struct Limits {
    hasher: RandomState,
    ip: Mutex<Buckets>,
    instance: Mutex<Buckets>,
    clock: Clock,
}

impl Limits {
    pub fn new(config: &crate::Config, clock: Clock) -> Self {
        Self {
            hasher: RandomState::new(),
            ip: Mutex::new(Buckets::new(
                config.max_ip_keys,
                config.ip_rate,
                config.ip_burst,
            )),
            instance: Mutex::new(Buckets::new(
                config.max_instance_keys,
                config.instance_rate,
                config.instance_burst,
            )),
            clock,
        }
    }

    /// One request from `ip`, or how long until it may send again.
    pub fn take_ip(&self, ip: IpAddr) -> Result<(), Duration> {
        let key = self.hasher.hash_one(ip);
        let now = (self.clock)();
        lock(&self.ip).take(key, now)
    }

    /// One resource from instance `id`, or how long until it may send again.
    pub fn take_instance(&self, id: &str) -> Result<(), Duration> {
        let key = self.hasher.hash_one(id);
        let now = (self.clock)();
        lock(&self.instance).take(key, now)
    }

    /// How many address buckets are held.
    pub fn ip_keys(&self) -> usize {
        lock(&self.ip).len()
    }
}

fn lock(m: &Mutex<Buckets>) -> std::sync::MutexGuard<'_, Buckets> {
    // A panic while holding the lock leaves only counts behind; keep going.
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(xff: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in xff {
            h.append("x-forwarded-for", HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn the_client_is_the_entry_trusted_hops_from_the_right() {
        let peer: Option<IpAddr> = Some("10.0.0.9".parse().unwrap());
        let h = headers(&["198.51.100.1, 203.0.113.77"]);
        assert_eq!(client_ip(&h, peer, 1), "203.0.113.77".parse().ok());
        assert_eq!(client_ip(&h, peer, 2), "198.51.100.1".parse().ok());
        assert_eq!(client_ip(&h, peer, 5), "198.51.100.1".parse().ok());
        assert_eq!(client_ip(&h, peer, 0), peer);
        assert_eq!(client_ip(&HeaderMap::new(), peer, 1), peer);
        let split = headers(&["198.51.100.1", "203.0.113.77"]);
        assert_eq!(client_ip(&split, peer, 1), "203.0.113.77".parse().ok());
        assert_eq!(client_ip(&headers(&["not-an-ip"]), peer, 1), peer);
    }

    #[test]
    fn a_full_map_evicts_idle_buckets_and_refuses_otherwise() {
        let start = Instant::now();
        let mut b = Buckets::new(2, 1.0, 2);
        assert!(b.take(1, start).is_ok());
        assert!(b.take(2, start).is_ok());
        // Full, and neither bucket is idle yet.
        assert_eq!(b.take(3, start), Err(Duration::from_secs(2)));
        // Two seconds on, both are full again and may go.
        let later = start + Duration::from_secs(2);
        assert!(b.take(3, later).is_ok());
        assert_eq!(b.len(), 1);
    }
}
