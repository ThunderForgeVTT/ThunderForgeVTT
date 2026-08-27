//! Executing the cross-tab sign-out signal, not merely compiling it.
//!
//! Spec 028 FR-021b/FR-021d.
//!
//! FR-021b is the requirement that a tab holding a live `CryptoKey` stops
//! using it *promptly and without reloading*. The entire mechanism is a
//! message sent on one carrier and received on another object in another tab,
//! and there is no build error for a listener that was registered on the
//! wrong property, a payload the parser rejects, or a channel constructed
//! under a name nobody is listening to. So both carriers are stood up here
//! and the real [`signal::listen`] / [`signal::broadcast_signed_out`] are run
//! against them under `wasm-pack test --node`.
//!
//! ```text
//! wasm-pack test --node crates/thunderforge-cache-browser
//! ```

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use js_sys::{Function, Reflect};
use thunderforge_cache_browser::signal::{self, CacheSignal};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::wasm_bindgen_test;

/// Both carriers, behaving the way browsers do in the respects that matter:
/// a `BroadcastChannel` message reaches every *other* channel object, and a
/// `storage` event carries a key and a new value.
///
/// Delivery is synchronous here where a browser's is not. That difference is
/// deliberate and harmless — what is under test is whether the message is
/// sent, parsed and acted on at all, not when.
const CARRIERS: &str = r#"
(() => {
  const state = { channels: [], storage: new Map(), listeners: [] };

  globalThis.BroadcastChannel = class {
    constructor(name) {
      this.name = name;
      this.onmessage = null;
      this.closed = false;
      state.channels.push(this);
    }
    postMessage(data) {
      for (const other of state.channels) {
        if (other !== this && !other.closed && other.onmessage) {
          other.onmessage({ data });
        }
      }
    }
    close() { this.closed = true; }
  };

  globalThis.localStorage = {
    setItem(key, value) { state.storage.set(key, value); },
    getItem(key) { return state.storage.has(key) ? state.storage.get(key) : null; },
  };

  globalThis.addEventListener = (type, handler) => {
    state.listeners.push({ type, handler });
  };

  globalThis.__signalState = state;
  globalThis.__fireStorage = (key, newValue) => {
    for (const listener of state.listeners) {
      if (listener.type === "storage") { listener.handler({ key, newValue }); }
    }
  };
  globalThis.__channelNames = () => state.channels.map((c) => c.name);
})()
"#;

thread_local! {
    /// Every signal the listener under test was handed.
    static RECEIVED: RefCell<Vec<CacheSignal>> = const { RefCell::new(Vec::new()) };
}

fn eval(source: &str) -> JsValue {
    js_sys::eval(source).expect("test harness javascript should evaluate")
}

fn received() -> Vec<CacheSignal> {
    RECEIVED.with(|slot| slot.borrow().clone())
}

fn stored(key: &str) -> Option<String> {
    let storage = Reflect::get(&js_sys::global(), &JsValue::from_str("localStorage")).ok()?;
    let get: Function = Reflect::get(&storage, &JsValue::from_str("getItem"))
        .ok()?
        .dyn_into()
        .ok()?;
    get.call1(&storage, &JsValue::from_str(key))
        .ok()?
        .as_string()
}

fn fire_storage(key: &str, value: &str) {
    let fire: Function = Reflect::get(&js_sys::global(), &JsValue::from_str("__fireStorage"))
        .expect("the harness publishes __fireStorage")
        .dyn_into()
        .expect("a function");
    fire.call2(
        &JsValue::NULL,
        &JsValue::from_str(key),
        &JsValue::from_str(value),
    )
    .expect("dispatching a storage event should not throw");
}

/// One test, because [`signal::listen`] registers once per page — which is
/// the behaviour a page wants and the behaviour this asserts. Everything that
/// has to be true of the round trip is therefore checked here, in order.
#[wasm_bindgen_test]
fn a_sign_out_in_one_tab_reaches_a_listener_in_another() {
    eval(CARRIERS);

    signal::listen(|received| {
        RECEIVED.with(|slot| slot.borrow_mut().push(received));
    });
    assert!(
        received().is_empty(),
        "registering a listener must not itself signal anything"
    );

    // The listener subscribed under the name the sender uses. A mismatch here
    // is silent in production: the message is sent and simply never arrives.
    let names = eval("__channelNames()");
    let names = js_sys::Array::from(&names);
    assert_eq!(names.length(), 1, "the listener holds exactly one channel");
    assert_eq!(
        names.get(0).as_string().as_deref(),
        Some(signal::SIGNAL_CHANNEL)
    );

    // The channel carrier.
    signal::broadcast_signed_out();
    assert_eq!(
        received(),
        vec![CacheSignal::SignedOut],
        "the listener must be told, without a reload and without polling"
    );

    // The storage carrier, which is what a browser without BroadcastChannel
    // has instead (FR-021d). Its payload is the same one, and the sender
    // wrote it under the agreed key.
    let payload = stored(signal::SIGNAL_STORAGE_KEY)
        .expect("the fallback carrier must be written as well as the channel");
    assert_eq!(signal::parse(&payload), Some(CacheSignal::SignedOut));

    fire_storage(signal::SIGNAL_STORAGE_KEY, &payload);
    assert_eq!(
        received(),
        vec![CacheSignal::SignedOut, CacheSignal::SignedOut],
        "a storage event carrying the signal must be acted on too"
    );

    // Noise on either carrier is ignored rather than mistaken for a sign-out.
    // Both are origin-wide namespaces shared with every other script here.
    fire_storage("some-other-app", &payload);
    fire_storage(signal::SIGNAL_STORAGE_KEY, "not json");
    eval(r#"__signalState.channels[0].onmessage({ data: "not json" })"#);
    eval(r#"__signalState.channels[0].onmessage({ data: 42 })"#);
    eval(r#"__signalState.channels[0].onmessage({})"#);
    assert_eq!(
        received().len(),
        2,
        "nothing but the signal itself may be taken for a sign-out"
    );
}
