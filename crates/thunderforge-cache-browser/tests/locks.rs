//! Executing `locks::acquire_exclusive`, not merely compiling it.
//!
//! Spec 028 FR-021a/FR-021c/FR-021d.
//!
//! Everything pure in this crate is covered by native unit tests, which
//! leaves the browser glue: promises handed to a platform API, a callback
//! closure the platform invokes, a race, and a guard whose `Drop` resolves
//! the promise that holds the lock. None of that is exercised by a build, and
//! all of it is the sort of thing that compiles perfectly while doing nothing
//! — which is exactly how this feature has failed before.
//!
//! `navigator.locks` is small enough to stand up by hand, so these run the
//! real function under `wasm-pack test --node` against a `LockManager`
//! written below that implements the queueing and `ifAvailable` semantics the
//! specification defines. OPFS, IndexedDB and WebCrypto have no equivalently
//! cheap stand-in and stay covered by the e2e suite.
//!
//! ```text
//! wasm-pack test --node crates/thunderforge-cache-browser
//! ```

#![cfg(target_arch = "wasm32")]

use js_sys::{Promise, Reflect};
use thunderforge_cache_browser::locks;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

/// A `navigator.locks` that behaves like the specification says: exclusive by
/// name, queued in arrival order, released when the callback's promise
/// settles, and handing `null` to an `ifAvailable` request for a busy lock.
const FAKE_LOCK_MANAGER: &str = r#"
(() => {
  const held = new Map();
  const queue = new Map();

  function release(name) {
    held.delete(name);
    const waiting = queue.get(name);
    if (waiting && waiting.length) {
      const next = waiting.shift();
      grant(name, next.callback, next.resolve, next.reject);
    }
  }

  function grant(name, callback, resolve, reject) {
    held.set(name, true);
    let result;
    try {
      result = callback({ name, mode: "exclusive" });
    } catch (err) {
      release(name);
      reject(err);
      return;
    }
    Promise.resolve(result).then(
      (value) => { release(name); resolve(value); },
      (err) => { release(name); reject(err); },
    );
  }

  const manager = {
    request(name, options, callback) {
      return new Promise((resolve, reject) => {
        if (held.has(name)) {
          if (options && options.ifAvailable) {
            Promise.resolve(callback(null)).then(resolve, reject);
            return;
          }
          if (!queue.has(name)) { queue.set(name, []); }
          queue.get(name).push({ callback, resolve, reject });
          return;
        }
        grant(name, callback, resolve, reject);
      });
    },
  };

  globalThis.__heldLocks = held;
  installNavigator({ locks: manager });
})()
"#;

/// Node defines `navigator` itself, so replacing it needs more than an
/// assignment.
const INSTALL_NAVIGATOR: &str = r#"
globalThis.installNavigator = (value) => {
  try {
    Object.defineProperty(globalThis, "navigator", {
      value,
      configurable: true,
      writable: true,
    });
  } catch (err) {
    globalThis.navigator = value;
  }
};
"#;

fn eval(source: &str) {
    js_sys::eval(source).expect("test harness javascript should evaluate");
}

fn install_locks() {
    eval(INSTALL_NAVIGATOR);
    eval(FAKE_LOCK_MANAGER);
}

/// Whether the fake manager currently considers `name` held.
fn is_held(name: &str) -> bool {
    let held = Reflect::get(&js_sys::global(), &JsValue::from_str("__heldLocks"))
        .expect("the fake manager publishes its held set");
    let has: js_sys::Function = Reflect::get(&held, &JsValue::from_str("has"))
        .expect("a Map")
        .dyn_into()
        .expect("Map.has");
    has.call1(&held, &JsValue::from_str(name))
        .expect("Map.has should not throw")
        .as_bool()
        .unwrap_or(false)
}

/// Let queued microtasks — releases, in particular — run.
async fn settle() {
    for _ in 0..8 {
        let _ = JsFuture::from(Promise::resolve(&JsValue::NULL)).await;
    }
}

#[wasm_bindgen_test]
async fn a_free_lock_is_taken_and_given_back_on_drop() {
    install_locks();

    let guard = locks::acquire_exclusive("test:free", 1_000.0)
        .await
        .expect("a free lock must be granted");
    assert!(is_held("test:free"), "the platform should see it as held");

    drop(guard);
    settle().await;
    assert!(
        !is_held("test:free"),
        "dropping the guard must release the lock; a lock held past its \
         guard is held until the tab closes"
    );
}

#[wasm_bindgen_test]
async fn a_second_holder_waits_and_then_gets_it() {
    // The property the whole mechanism rests on: while one holder has it,
    // nobody else does.
    install_locks();

    let first = locks::acquire_exclusive("test:queue", 1_000.0)
        .await
        .expect("granted");
    drop(first);
    settle().await;

    let second = locks::acquire_exclusive("test:queue", 1_000.0)
        .await
        .expect("the lock must be available once the first holder is gone");
    assert!(is_held("test:queue"));
    drop(second);
    settle().await;
    assert!(!is_held("test:queue"));
}

#[wasm_bindgen_test]
async fn a_contended_lock_times_out_instead_of_hanging() {
    // FR-021d, and the reason every wait here is bounded: a lock nobody
    // releases must not be able to stall a user-visible path.
    install_locks();

    let held = locks::acquire_exclusive("test:busy", 1_000.0)
        .await
        .expect("granted");

    let refused = locks::acquire_exclusive("test:busy", 20.0).await;
    assert!(
        refused.is_none(),
        "a busy lock must time out, not wait forever"
    );
    assert!(is_held("test:busy"), "the first holder still has it");

    drop(held);
    settle().await;
}

#[wasm_bindgen_test]
async fn a_timed_out_request_does_not_strand_the_lock() {
    // The subtle one. A request that loses the race is still queued at the
    // platform, and will be granted the moment the current holder lets go.
    // If that grant were to park the lock on a caller that has already given
    // up, the lock would be held — by nobody, for nothing — until the tab
    // closed, and every other tab on the origin would block on it forever.
    install_locks();

    let held = locks::acquire_exclusive("test:strand", 1_000.0)
        .await
        .expect("granted");

    assert!(
        locks::acquire_exclusive("test:strand", 20.0)
            .await
            .is_none(),
        "the second request must time out"
    );

    drop(held);
    settle().await;

    assert!(
        !is_held("test:strand"),
        "the abandoned request must release the instant it is granted"
    );

    // And the lock is genuinely usable again, not merely reported free.
    let again = locks::acquire_exclusive("test:strand", 1_000.0)
        .await
        .expect("the lock must be available to the next caller");
    drop(again);
    settle().await;
}

#[wasm_bindgen_test]
async fn different_names_do_not_contend() {
    // Key locks are per scope and world locks per world precisely so that the
    // ordinary multi-tab case — two different worlds — never waits.
    install_locks();

    let a = locks::acquire_exclusive(&locks::world_sync_lock(uuid::Uuid::from_u128(1)), 1_000.0)
        .await
        .expect("granted");
    let b = locks::acquire_exclusive(&locks::world_sync_lock(uuid::Uuid::from_u128(2)), 1_000.0)
        .await
        .expect("a different world must not queue behind the first");

    drop(a);
    drop(b);
    settle().await;
}

#[wasm_bindgen_test]
async fn a_browser_without_web_locks_degrades_rather_than_failing() {
    // FR-021d. `None` is the caller's instruction to proceed unlocked, which
    // is today's behaviour: possibly an extra fetch, never a failed load.
    eval(INSTALL_NAVIGATOR);
    eval("installNavigator({})");

    assert!(
        locks::acquire_exclusive("test:absent", 1_000.0)
            .await
            .is_none()
    );

    eval("installNavigator(undefined)");
    assert!(
        locks::acquire_exclusive("test:absent", 1_000.0)
            .await
            .is_none()
    );
}

#[wasm_bindgen_test]
async fn a_rejected_request_degrades_immediately() {
    // A `request` that throws must not leave the caller waiting out the full
    // timeout — the grant promise would otherwise stay pending forever and
    // the timer would be the only way out.
    eval(INSTALL_NAVIGATOR);
    eval(
        r#"installNavigator({ locks: { request() { return Promise.reject(new Error("nope")); } } })"#,
    );

    assert!(
        locks::acquire_exclusive("test:rejects", 60_000.0)
            .await
            .is_none(),
        "a rejection must degrade at once rather than wait out the timeout"
    );
}

#[wasm_bindgen_test]
async fn releasing_twice_is_harmless() {
    install_locks();

    let guard = locks::acquire_exclusive("test:idempotent", 1_000.0)
        .await
        .expect("granted");
    guard.release();
    guard.release();
    drop(guard);
    settle().await;

    assert!(!is_held("test:idempotent"));
}
