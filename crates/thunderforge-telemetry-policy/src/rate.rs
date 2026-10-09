//! A token bucket, with the time passed in (FR-042, FR-043).

use std::time::{Duration, Instant};

/// Starts full at its burst and refills at its rate.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    tokens: f64,
    last: Instant,
    rate: f64,
    burst: f64,
}

impl TokenBucket {
    pub fn new(rate_per_s: f64, burst: u32, now: Instant) -> Self {
        let burst = f64::from(burst.max(1));
        Self {
            tokens: burst,
            last: now,
            rate: rate_per_s.max(f64::MIN_POSITIVE),
            burst,
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.burst);
        self.last = now;
    }

    /// Takes one token, or says how long until one is free.
    pub fn try_take(&mut self, now: Instant) -> Result<(), Duration> {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((1.0 - self.tokens) / self.rate))
        }
    }

    /// Whether the bucket has been idle long enough to be full again, so
    /// evicting it changes nothing a sender can see.
    pub fn is_idle(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last).as_secs_f64() >= self.burst / self.rate
    }

    /// A wait as `Retry-After` seconds, rounded up, at least one.
    pub fn retry_after_secs(wait: Duration) -> u64 {
        let secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
        secs.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_then_refill() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new(2.0, 3, t0);
        for _ in 0..3 {
            assert!(b.try_take(t0).is_ok());
        }
        let wait = b.try_take(t0).unwrap_err();
        assert_eq!(wait, Duration::from_millis(500));
        assert!(b.try_take(t0 + Duration::from_millis(499)).is_err());
        assert!(b.try_take(t0 + Duration::from_millis(500)).is_ok());
        // Never more than the burst, however long it waits.
        let later = t0 + Duration::from_secs(3600);
        for _ in 0..3 {
            assert!(b.try_take(later).is_ok());
        }
        assert!(b.try_take(later).is_err());
    }

    #[test]
    fn retry_after_rounds_up() {
        assert_eq!(TokenBucket::retry_after_secs(Duration::from_millis(200)), 1);
        assert_eq!(
            TokenBucket::retry_after_secs(Duration::from_millis(1001)),
            2
        );
        assert_eq!(TokenBucket::retry_after_secs(Duration::from_secs(3)), 3);
        assert_eq!(TokenBucket::retry_after_secs(Duration::ZERO), 1);
        let t0 = Instant::now();
        let mut b = TokenBucket::new(0.25, 1, t0);
        assert!(b.try_take(t0).is_ok());
        let wait = b.try_take(t0).unwrap_err();
        assert_eq!(TokenBucket::retry_after_secs(wait), 4);
    }

    #[test]
    fn idle_after_a_full_refill() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new(5.0, 60, t0);
        assert!(b.try_take(t0).is_ok());
        assert!(!b.is_idle(t0 + Duration::from_secs(11)));
        assert!(b.is_idle(t0 + Duration::from_secs(12)));
    }
}
