//! How much room the browser is giving this origin, and what to do about it.
//!
//! # Why the arithmetic is separated from the call
//!
//! `navigator.storage.estimate()` is one `await` and returns two numbers. The
//! interesting part is not fetching them — it is deciding what they mean, and
//! that decision has edges worth pinning: a browser that declines to estimate,
//! a quota of zero, a usage that exceeds the quota (which happens, because
//! both figures are approximations and the quota moves), and the difference
//! between "getting full" and "full".
//!
//! So [`Estimate`] is a plain struct with plain methods and a native test, and
//! the browser call is six lines at the bottom under `cfg(wasm32)`. This is
//! the same split the rest of this crate uses and for the same reason: the
//! part worth testing is the part that decides something.
//!
//! # These numbers are deliberately vague, and saying so is the point
//!
//! The spec calls the result an *estimate*, and browsers pad it on purpose —
//! a precise figure is a cross-origin fingerprinting signal and a way to probe
//! what else the user has stored. So nothing here should be rendered as though
//! it were a disk usage readout. `usage` is what this origin is charged for,
//! **including** IndexedDB, caches and service-worker storage, not just the
//! blobs this crate wrote — which is why the cache's own index total and this
//! number will never agree, and why showing them side by side without saying
//! that would just look like a bug.

/// What the browser says about this origin's storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    /// Bytes this origin is charged for, across every storage API.
    pub usage: u64,
    /// Bytes it may use before writes start failing.
    pub quota: u64,
}

/// How close to full this origin is, in the terms a caller acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressure {
    /// Room to spare. Cache freely.
    Comfortable,
    /// Past [`Estimate::TIGHT_AT`]. Keep caching, but this is the point at
    /// which an eviction pass is worth running before the browser makes the
    /// decision for us.
    Tight,
    /// Past [`Estimate::CRITICAL_AT`]. The next sizeable write is likely to be
    /// refused, and a refusal mid-write is how a half-file gets left behind.
    Critical,
}

impl Estimate {
    /// Where "comfortable" ends.
    ///
    /// 80% rather than a byte count because quotas differ by two orders of
    /// magnitude between a phone and a desktop, and a fixed threshold would be
    /// either meaningless on one or permanently tripped on the other.
    pub const TIGHT_AT: f64 = 0.80;
    /// Where "tight" ends. Chosen below any browser's own eviction trigger so
    /// this instance gets to choose what to drop before the browser does.
    pub const CRITICAL_AT: f64 = 0.95;

    /// The fraction of the quota in use, clamped to `0.0..=1.0`.
    ///
    /// A zero quota reads as full rather than as an error: a browser that
    /// offers no room is not a browser with infinite room, and dividing by it
    /// is the other way to get that wrong.
    pub fn fraction_used(self) -> f64 {
        if self.quota == 0 {
            return 1.0;
        }
        // Both figures are approximations the browser is free to revise, and
        // usage above quota is a state they really do report. Clamped rather
        // than treated as impossible.
        (self.usage as f64 / self.quota as f64).clamp(0.0, 1.0)
    }

    pub fn pressure(self) -> Pressure {
        let used = self.fraction_used();
        if used >= Self::CRITICAL_AT {
            Pressure::Critical
        } else if used >= Self::TIGHT_AT {
            Pressure::Tight
        } else {
            Pressure::Comfortable
        }
    }

    /// Bytes left before the quota, saturating at zero.
    pub fn remaining(self) -> u64 {
        self.quota.saturating_sub(self.usage)
    }

    /// Whether a write of `bytes` is worth attempting.
    ///
    /// Not a guarantee — only the browser knows, and it may revise the quota
    /// between this call and the write. It exists so a caller can skip an
    /// attempt that is nearly certain to fail, because a failed write is not
    /// free: it can leave the zero-length file `BlobShape::Incomplete` exists
    /// to describe.
    pub fn can_probably_fit(self, bytes: u64) -> bool {
        self.remaining() >= bytes
    }
}

/// Ask the browser. `None` when it declines to say, which is a real answer and
/// not an error: Safari has historically returned nothing here, and a caller's
/// correct response is to carry on caching rather than to stop.
#[cfg(target_arch = "wasm32")]
pub async fn estimate() -> Option<Estimate> {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    // Read off `globalThis` rather than `window`, exactly as `opfs.rs` does.
    // A dedicated worker has a `navigator` and no `window`, and this crate is
    // meant to keep working if its caller ever moves into one.
    let navigator =
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("navigator")).ok()?;
    let storage = js_sys::Reflect::get(&navigator, &JsValue::from_str("storage")).ok()?;
    if storage.is_undefined() || storage.is_null() {
        return None;
    }

    let storage: web_sys::StorageManager = storage.unchecked_into();
    let value = JsFuture::from(storage.estimate().ok()?).await.ok()?;

    // Read the two fields reflectively rather than through a typed dictionary.
    // `StorageEstimate` is a dictionary in the IDL, so web-sys generates it as
    // a *builder* for passing values in, not a reader for taking them out.
    let read = |key: &str| -> Option<f64> {
        js_sys::Reflect::get(&value, &JsValue::from_str(key))
            .ok()
            .and_then(|v| v.as_f64())
    };

    // Both fields are optional in the IDL. A browser that reports one and not
    // the other tells us nothing actionable, so that is `None` too rather than
    // a half-answer with a zero in it — a zero quota means "full" above, and
    // inventing one would report a healthy origin as critical.
    Some(Estimate {
        usage: read("usage")?.max(0.0) as u64,
        quota: read("quota")?.max(0.0) as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(usage: u64, quota: u64) -> Estimate {
        Estimate { usage, quota }
    }

    #[test]
    fn an_empty_origin_is_comfortable() {
        assert_eq!(at(0, 1_000).pressure(), Pressure::Comfortable);
        assert_eq!(at(0, 1_000).remaining(), 1_000);
    }

    #[test]
    fn the_thresholds_are_where_they_say_they_are() {
        assert_eq!(at(799, 1_000).pressure(), Pressure::Comfortable);
        assert_eq!(at(800, 1_000).pressure(), Pressure::Tight);
        assert_eq!(at(949, 1_000).pressure(), Pressure::Tight);
        assert_eq!(at(950, 1_000).pressure(), Pressure::Critical);
    }

    #[test]
    fn a_quota_of_zero_is_full_rather_than_a_division_by_zero() {
        // A browser offering no room is not a browser offering infinite room,
        // which is what the other reading of 0 would produce.
        let estimate = at(0, 0);
        assert_eq!(estimate.fraction_used(), 1.0);
        assert_eq!(estimate.pressure(), Pressure::Critical);
        assert_eq!(estimate.remaining(), 0);
    }

    #[test]
    fn usage_above_quota_is_a_state_browsers_report_and_it_clamps() {
        // Both numbers are approximations the browser revises independently,
        // so this is reachable in the wild rather than nonsense to reject.
        let estimate = at(1_500, 1_000);
        assert_eq!(estimate.fraction_used(), 1.0);
        assert_eq!(estimate.pressure(), Pressure::Critical);
        assert_eq!(estimate.remaining(), 0);
    }

    #[test]
    fn a_write_that_would_not_fit_is_not_worth_attempting() {
        let estimate = at(900, 1_000);
        assert!(estimate.can_probably_fit(100));
        assert!(!estimate.can_probably_fit(101));
        // And at the boundary, because "exactly fits" is the case an off-by-one
        // gets wrong and it is the common one when evicting to make room.
        assert!(at(0, 10).can_probably_fit(10));
    }
}
