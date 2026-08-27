//! Cross-tab mutual exclusion, via the Web Locks API (T055a, T055c).
//!
//! Spec 028 FR-021, FR-021a, FR-021c, FR-021d.
//!
//! # Why a lock at all
//!
//! Every other guard in this crate is within one tab. `Handles` is a
//! thread-local, the shared-open future in the engine is a thread-local, and
//! a `RefCell` says nothing whatsoever to a second tab. But a second tab is
//! the *ordinary* case here — a world open in one, a character sheet in
//! another — and two of the operations this crate performs are not safe to
//! interleave across them:
//!
//! 1. **Creating the session key.** Two tabs that both find no key both
//!    generate one, and the loser's writes become unreadable. That degrades
//!    to a cache miss rather than corruption (FR-016c), but a cache that
//!    silently never works is still broken.
//! 2. **Evicting while another tab is fetching.** One tab applying an
//!    eviction can delete the blob another tab has just written, leaving an
//!    index row pointing at nothing until the FR-019 repair notices.
//!
//! `navigator.locks` exists for exactly this: a named, origin-scoped lock
//! held across tabs and workers, released automatically if the tab holding it
//! dies. Nothing else in the platform has that last property, which is what
//! makes it usable in a path a user can close a tab in the middle of.
//!
//! # Nothing here may hang
//!
//! Every acquisition is bounded (see [`acquire_exclusive`]) and every failure
//! to acquire proceeds **unlocked** rather than failing. That is FR-021d
//! stated as code: without coordination the behaviour is today's — a possible
//! extra fetch, a possible ineffective cache — and never a failed load.
//!
//! Sign-out deliberately takes no lock at all. Discarding a key is always
//! safe, is never wrong to do twice, and must not be delayed by a lock some
//! other tab is holding.

use uuid::Uuid;

/// Prefix every lock this crate takes shares.
///
/// Web Lock names are a flat, origin-wide namespace shared with every other
/// script on the origin, so an unprefixed name like `"key"` is a collision
/// waiting to happen with code that has nothing to do with this cache.
pub const LOCK_PREFIX: &str = "thunderforge-cache";

/// How long to wait for the session-key lock before giving up and
/// generating one unlocked.
///
/// Generous because what happens behind this lock is short — one IndexedDB
/// read, one key generation, one write — and because losing the race is the
/// expensive outcome (a whole cache written under a key nobody keeps), while
/// waiting is nearly free: this runs during the cache's own cold start, with
/// no rendering behind it.
pub const KEY_LOCK_TIMEOUT_MS: f64 = 3_000.0;

/// How long an eviction pass waits for its world's lock.
///
/// Shorter than the key lock: an eviction that never happens costs disk
/// space until the next sync, which the budget pass reclaims anyway.
pub const WORLD_LOCK_TIMEOUT_MS: f64 = 2_000.0;

/// How long a *write* waits for its world's lock.
///
/// Deliberately short. This one sits behind an asset the user is waiting to
/// see, and the failure mode of not getting it is that a blob is written
/// while another tab is evicting — recoverable by the FR-018/FR-019 repair —
/// whereas the failure mode of waiting is a visibly slower image.
pub const WRITE_LOCK_TIMEOUT_MS: f64 = 250.0;

/// The lock serialising session-key creation for one user scope (FR-021a).
///
/// Named per scope rather than globally: two different users' keys have no
/// reason to contend, and a global name would make one user's cold start
/// wait on another's.
pub fn key_creation_lock(scope: &str) -> String {
    format!("{LOCK_PREFIX}:key:{scope}")
}

/// The lock serialising sync and eviction for one world (FR-021c).
///
/// Per world, because that is the granularity eviction works at and the
/// granularity two tabs actually collide at — the common multi-tab case is
/// *different* worlds, which must not contend at all.
pub fn world_sync_lock(world_id: Uuid) -> String {
    format!("{LOCK_PREFIX}:world:{world_id}")
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{Guard, acquire_exclusive};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::Cell;

    use js_sys::{Array, Function, Object, Promise, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    use crate::global_property;

    /// A held Web Lock. Released on drop, and idempotently.
    ///
    /// Release is a resolve of the promise the lock callback returned — the
    /// platform holds the lock exactly as long as that promise is pending.
    /// Doing it in `Drop` rather than at an explicit call site is what keeps
    /// an early `return` or a `?` from stranding the lock for every other
    /// tab on the origin until this one is closed.
    pub struct Guard {
        release: Function,
        released: Cell<bool>,
    }

    impl Guard {
        /// Give the lock up now. Safe to call more than once.
        pub fn release(&self) {
            if self.released.replace(true) {
                return;
            }
            let _ = self.release.call0(&JsValue::NULL);
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            self.release();
        }
    }

    /// Take `name` exclusively, waiting at most `timeout_ms`.
    ///
    /// `None` means "proceed without coordination": the browser has no
    /// `navigator.locks`, the request was rejected, or the wait ran out.
    /// Every caller treats those the same way, which is what makes FR-021d
    /// a property of this function rather than of each call site.
    ///
    /// # Why the timeout is a race rather than an `AbortSignal`
    ///
    /// `AbortSignal.timeout` would be tidier, but it is not available
    /// everywhere this must run, and combining it with `ifAvailable` is a
    /// `TypeError` by specification. So the grant is raced against a timer,
    /// and — the part that matters — **a request that loses the race is
    /// released the moment it is eventually granted**, because the promise
    /// the callback returns has already been resolved. Abandoning a queued
    /// request without that would hand the lock to a caller that is no
    /// longer waiting and hold it there forever.
    pub async fn acquire_exclusive(name: &str, timeout_ms: f64) -> Option<Guard> {
        let locks = lock_manager()?;
        let request: Function = Reflect::get(&locks, &JsValue::from_str("request"))
            .ok()?
            .dyn_into()
            .ok()?;

        // Resolved to release the lock; pending for as long as it is held.
        let mut release: Option<Function> = None;
        let held = Promise::new(&mut |resolve, _reject| release = Some(resolve));
        let release = release?;

        // Resolved with `true` when the lock is granted, `false` when the
        // request is refused or rejected. Never left pending.
        let mut grant: Option<Function> = None;
        let granted = Promise::new(&mut |resolve, _reject| grant = Some(resolve));
        let grant = grant?;

        let timer = timeout_promise(timeout_ms);
        // With no timer there is no safe way to bound a wait, so ask for the
        // lock only if it is free. An ineffective lock is acceptable
        // (FR-021d); an unbounded wait in a user-visible path is not.
        let if_available = timer.is_none();

        let on_granted = grant.clone();
        let held_for_callback = held.clone();
        let callback = Closure::once_into_js(move |lock: JsValue| -> JsValue {
            // `ifAvailable` hands the callback `null` when the lock was
            // busy. That is a refusal, not a grant.
            let acquired = !(lock.is_null() || lock.is_undefined());
            let _ = on_granted.call1(&JsValue::NULL, &JsValue::from_bool(acquired));
            if acquired {
                held_for_callback.into()
            } else {
                JsValue::UNDEFINED
            }
        });

        let options = Object::new();
        Reflect::set(
            &options,
            &JsValue::from_str("mode"),
            &JsValue::from_str("exclusive"),
        )
        .ok()?;
        if if_available {
            Reflect::set(&options, &JsValue::from_str("ifAvailable"), &JsValue::TRUE).ok()?;
        }

        let requested: Promise = request
            .call3(&locks, &JsValue::from_str(name), &options, &callback)
            .ok()?
            .dyn_into()
            .ok()?;

        // The request promise itself is raced alongside the grant, which is
        // what keeps a *rejected* request from stalling until the timeout: a
        // rejection there rejects the race, and `JsFuture` hands that back as
        // an `Err` this treats like any other non-grant. While the lock is
        // genuinely held it stays pending, so it can never beat the grant.
        let outcome = match timer {
            Some(timer) => {
                let race = Promise::race(&Array::of3(&granted, &requested, &timer));
                JsFuture::from(race).await
            }
            None => {
                let race = Promise::race(&Array::of2(&granted, &requested));
                JsFuture::from(race).await
            }
        };

        if outcome.ok().and_then(|value| value.as_bool()) == Some(true) {
            return Some(Guard {
                release,
                released: Cell::new(false),
            });
        }

        // Either refused, or the wait ran out. Resolve the held promise now
        // so that a grant arriving later releases immediately instead of
        // parking the lock on a caller that has moved on.
        let _ = release.call0(&JsValue::NULL);
        None
    }

    /// `navigator.locks`, or `None` where the browser has no Web Locks.
    fn lock_manager() -> Option<JsValue> {
        let navigator = global_property("navigator").ok()?;
        let locks = Reflect::get(&navigator, &JsValue::from_str("locks")).ok()?;
        if locks.is_undefined() || locks.is_null() {
            return None;
        }
        Some(locks)
    }

    /// A promise that resolves to `undefined` after `ms`.
    ///
    /// Resolving to `undefined` rather than `false` is deliberate: the caller
    /// tests for exactly `Some(true)`, so anything that is not a grant reads
    /// as a non-grant without a second comparison to keep in step.
    fn timeout_promise(ms: f64) -> Option<Promise> {
        let set_timeout: Function =
            Reflect::get(&js_sys::global(), &JsValue::from_str("setTimeout"))
                .ok()?
                .dyn_into()
                .ok()?;
        let mut scheduled = false;
        let promise = Promise::new(&mut |resolve, _reject| {
            scheduled = set_timeout
                .call2(&JsValue::NULL, &resolve, &JsValue::from_f64(ms))
                .is_ok();
        });
        scheduled.then_some(promise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_locks_are_per_scope() {
        assert_ne!(key_creation_lock("alice"), key_creation_lock("bob"));
        assert_eq!(key_creation_lock("alice"), key_creation_lock("alice"));
    }

    #[test]
    fn world_locks_are_per_world() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert_ne!(world_sync_lock(a), world_sync_lock(b));
        assert_eq!(world_sync_lock(a), world_sync_lock(a));
    }

    #[test]
    fn key_and_world_locks_never_collide() {
        // A scope is `[A-Za-z0-9_-]` and a world is a uuid, so the only
        // thing keeping the two namespaces apart is the infix. Worth
        // asserting, because a collision would make one user's cold start
        // wait on an unrelated world's eviction.
        let world = Uuid::from_u128(7);
        assert_ne!(
            world_sync_lock(world),
            key_creation_lock(&world.to_string())
        );
    }

    #[test]
    fn every_lock_name_is_prefixed() {
        for name in [
            key_creation_lock("scope"),
            world_sync_lock(Uuid::from_u128(3)),
        ] {
            assert!(
                name.starts_with(LOCK_PREFIX),
                "{name} would share the origin's flat lock namespace unprefixed"
            );
        }
    }

    #[test]
    fn waits_are_bounded_and_ordered_by_what_they_block() {
        // The ordering is the policy: nothing user-visible waits as long as
        // a cold start does, and no wait is unbounded.
        let waits = [
            ("write", WRITE_LOCK_TIMEOUT_MS),
            ("eviction", WORLD_LOCK_TIMEOUT_MS),
            ("key creation", KEY_LOCK_TIMEOUT_MS),
        ];
        for pair in waits.windows(2) {
            let (shorter, longer) = (pair[0], pair[1]);
            assert!(
                shorter.1 < longer.1,
                "{} must not wait as long as {}",
                shorter.0,
                longer.0
            );
        }
        for (what, wait) in waits {
            assert!(
                wait.is_finite() && wait > 0.0,
                "the {what} wait must be bounded"
            );
        }
    }
}
