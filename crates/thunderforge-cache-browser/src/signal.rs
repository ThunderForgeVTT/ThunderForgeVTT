//! Cross-tab notification that a session ended (T055b).
//!
//! Spec 028 FR-016a, FR-021b, FR-021d.
//!
//! # The hole this closes
//!
//! Sign-out deletes the stored `CryptoKey` record, and that is what makes the
//! bytes on disk inert — for a tab that has to go and read the record. It is
//! not what makes them inert for a tab that read the record an hour ago and
//! is still holding the live `CryptoKey` in memory. That tab keeps decrypting
//! happily until something makes it reload. **A key discarded from storage
//! while another tab holds it is not discarded** (FR-021b), so the discard has
//! to be announced, not merely performed.
//!
//! # Two carriers, because one is not enough
//!
//! [`broadcast_signed_out`] sends on a `BroadcastChannel` *and* stamps a key
//! in `localStorage`. That is not redundancy for its own sake:
//!
//! - `BroadcastChannel` reaches workers as well as windows and is the right
//!   primitive, but it is not everywhere.
//! - A `localStorage` write fires a `storage` event in every *other* window of
//!   the profile, and has done in every browser for well over a decade. It
//!   does not reach workers and it does not fire in the originating window,
//!   so it cannot replace the channel — but it means the degraded path for
//!   FR-021b is "a different mechanism" rather than "nothing".
//!
//! Both carry the same payload and the handler is idempotent, so receiving
//! both is not a problem worth suppressing. Forgetting twice is forgetting.
//!
//! Nothing here can fail in a way a caller must handle. A signal that cannot
//! be sent leaves the other tab exactly where today's code leaves it, and a
//! signal that cannot be listened for costs a listener, not a session.

use serde::{Deserialize, Serialize};

/// The `BroadcastChannel` name. Mirrored in
/// `apps/web/src/services/worldCache.ts`, which sends the same payload from
/// pages that never mounted the engine.
pub const SIGNAL_CHANNEL: &str = "thunderforge-cache";

/// The `localStorage` key used as the fallback carrier.
///
/// Its *value* is meaningless beyond having to differ from the last one — a
/// `storage` event only fires when the value actually changes — which is why
/// [`encode`] takes a nonce.
pub const SIGNAL_STORAGE_KEY: &str = "thunderforge-cache:signal";

/// What one tab tells the others.
///
/// Exactly one variant today, and the wire form is a tagged object rather
/// than a bare string precisely so a second one can be added without every
/// listener having to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheSignal {
    /// The session ended. Drop any in-memory key and stop serving from the
    /// cache, now, without waiting for a reload (FR-021b).
    SignedOut,
}

/// The wire kind for [`CacheSignal::SignedOut`].
const KIND_SIGNED_OUT: &str = "signed-out";

#[derive(Serialize, Deserialize)]
struct Wire<'a> {
    kind: &'a str,
    /// Only there to make consecutive `localStorage` writes differ. Carries
    /// no meaning and is never read back.
    nonce: &'a str,
}

/// Render a signal for the wire.
///
/// `nonce` must differ between consecutive sends or the `storage` fallback
/// silently does nothing; a timestamp is enough and is what the callers use.
pub fn encode(signal: CacheSignal, nonce: &str) -> String {
    let kind = match signal {
        CacheSignal::SignedOut => KIND_SIGNED_OUT,
    };
    serde_json::json!({ "kind": kind, "nonce": nonce }).to_string()
}

/// Read a signal off the wire, or `None`.
///
/// Every rejection is silent and total. This listens on an origin-wide
/// channel and an origin-wide storage area, so anything at all may arrive
/// here — another script's message, a key someone hand-edited in devtools, a
/// payload from a future version of this app. None of that is an error
/// condition; it is simply not a signal, and the only safe response is to
/// ignore it.
pub fn parse(raw: &str) -> Option<CacheSignal> {
    let wire: Wire<'_> = serde_json::from_str(raw).ok()?;
    match wire.kind {
        KIND_SIGNED_OUT => Some(CacheSignal::SignedOut),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{broadcast_signed_out, listen};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::RefCell;

    use js_sys::{Array, Function, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};

    use super::{CacheSignal, SIGNAL_CHANNEL, SIGNAL_STORAGE_KEY, encode, parse};

    thread_local! {
        /// The channel object and its listener closures, kept alive for the
        /// life of the page. A `Closure` dropped while the platform still
        /// holds a reference to it becomes a call into freed memory, so
        /// these are deliberately never released — there is exactly one set
        /// per page and the page outliving them is the point.
        static LISTENING: RefCell<Option<Registration>> = const { RefCell::new(None) };
    }

    struct Registration {
        _channel: Option<JsValue>,
        _on_message: Option<Closure<dyn FnMut(JsValue)>>,
        _on_storage: Option<Closure<dyn FnMut(JsValue)>>,
    }

    /// Tell every other tab of this profile that the session ended.
    ///
    /// Best effort by construction and **never blocking**: sign-out must not
    /// wait on, or be failed by, anything here. A browser with neither
    /// carrier leaves other tabs on today's behaviour, which is FR-021d.
    pub fn broadcast_signed_out() {
        let payload = encode(CacheSignal::SignedOut, &nonce());

        if let Some(channel) = new_channel()
            && let Ok(post) = Reflect::get(&channel, &JsValue::from_str("postMessage"))
            && let Ok(post) = post.dyn_into::<Function>()
        {
            let _ = post.call1(&channel, &JsValue::from_str(&payload));
            // Closing releases the channel promptly. The messages already
            // posted are delivered regardless.
            if let Ok(close) = Reflect::get(&channel, &JsValue::from_str("close"))
                && let Ok(close) = close.dyn_into::<Function>()
            {
                let _ = close.call0(&channel);
            }
        }

        if let Some(storage) = local_storage()
            && let Ok(set_item) = Reflect::get(&storage, &JsValue::from_str("setItem"))
            && let Ok(set_item) = set_item.dyn_into::<Function>()
        {
            let _ = set_item.call2(
                &storage,
                &JsValue::from_str(SIGNAL_STORAGE_KEY),
                &JsValue::from_str(&payload),
            );
        }
    }

    /// Run `handler` whenever another tab signals. Registers once per page.
    ///
    /// The handler must be idempotent: both carriers may deliver the same
    /// signal, and the same tab may be told twice. Every action it is used
    /// for here — drop a key, stop reading the cache — already is.
    pub fn listen(handler: impl Fn(CacheSignal) + 'static) {
        if LISTENING.with(|slot| slot.borrow().is_some()) {
            return;
        }

        let handler = std::rc::Rc::new(handler);

        let mut channel_value = None;
        let mut on_message = None;
        if let Some(channel) = new_channel() {
            let deliver = handler.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let Ok(data) = Reflect::get(&event, &JsValue::from_str("data")) else {
                    return;
                };
                if let Some(raw) = data.as_string()
                    && let Some(signal) = parse(&raw)
                {
                    deliver(signal);
                }
            });
            if Reflect::set(
                &channel,
                &JsValue::from_str("onmessage"),
                closure.as_ref().unchecked_ref(),
            )
            .is_ok()
            {
                on_message = Some(closure);
                channel_value = Some(channel);
            }
        }

        // The `storage` event only reaches windows, and only ones other than
        // the sender. That is exactly the audience FR-021b names.
        let mut on_storage = None;
        if let Ok(add) = Reflect::get(&js_sys::global(), &JsValue::from_str("addEventListener"))
            && let Ok(add) = add.dyn_into::<Function>()
        {
            let deliver = handler.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let key = Reflect::get(&event, &JsValue::from_str("key"))
                    .ok()
                    .and_then(|k| k.as_string());
                if key.as_deref() != Some(SIGNAL_STORAGE_KEY) {
                    return;
                }
                // A cleared key arrives as a null `newValue`. Nothing to do:
                // absence is not a signal.
                let Ok(new_value) = Reflect::get(&event, &JsValue::from_str("newValue")) else {
                    return;
                };
                if let Some(raw) = new_value.as_string()
                    && let Some(signal) = parse(&raw)
                {
                    deliver(signal);
                }
            });
            let args = Array::of2(
                &JsValue::from_str("storage"),
                closure.as_ref().unchecked_ref(),
            );
            if Reflect::apply(&add, &js_sys::global(), &args).is_ok() {
                on_storage = Some(closure);
            }
        }

        LISTENING.with(|slot| {
            *slot.borrow_mut() = Some(Registration {
                _channel: channel_value,
                _on_message: on_message,
                _on_storage: on_storage,
            });
        });
    }

    /// A fresh `BroadcastChannel`, or `None` where the browser has none.
    ///
    /// Constructed reflectively rather than through `web-sys` so that the
    /// absence of the API is an `Option` here rather than a link-time
    /// dependency, which is what lets FR-021d be a runtime fallback.
    fn new_channel() -> Option<JsValue> {
        let ctor = Reflect::get(&js_sys::global(), &JsValue::from_str("BroadcastChannel")).ok()?;
        if ctor.is_undefined() || ctor.is_null() {
            return None;
        }
        let ctor: Function = ctor.dyn_into().ok()?;
        Reflect::construct(&ctor, &Array::of1(&JsValue::from_str(SIGNAL_CHANNEL))).ok()
    }

    /// `localStorage`, or `None` — it is absent in workers and throws when
    /// site data is blocked.
    fn local_storage() -> Option<JsValue> {
        let storage = Reflect::get(&js_sys::global(), &JsValue::from_str("localStorage")).ok()?;
        if storage.is_undefined() || storage.is_null() {
            return None;
        }
        Some(storage)
    }

    /// Something different every time, so a `storage` write is seen as a
    /// change. `Date.now` is plenty; two sign-outs in the same millisecond
    /// are the same sign-out.
    fn nonce() -> String {
        js_sys::Date::now().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_out_round_trips() {
        let wire = encode(CacheSignal::SignedOut, "1");
        assert_eq!(parse(&wire), Some(CacheSignal::SignedOut));
    }

    #[test]
    fn the_nonce_does_not_change_the_meaning() {
        // The fallback carrier needs consecutive values to differ, but a
        // listener that treated two nonces as two different signals — or as
        // an unrecognised one — would break the mechanism it exists for.
        let first = encode(CacheSignal::SignedOut, "1");
        let second = encode(CacheSignal::SignedOut, "2");
        assert_ne!(first, second, "a repeated value fires no storage event");
        assert_eq!(parse(&first), parse(&second));
    }

    #[test]
    fn the_typescript_payload_is_understood() {
        // `apps/web/src/services/worldCache.ts` sends this exact shape from
        // pages that never mounted the engine, which is the common sign-out.
        // If this drifts, sign-out stops reaching other tabs and nothing
        // fails loudly.
        assert_eq!(
            parse(r#"{"kind":"signed-out","nonce":"1758000000000"}"#),
            Some(CacheSignal::SignedOut)
        );
    }

    #[test]
    fn anything_else_on_the_channel_is_ignored() {
        for raw in [
            "",
            "not json",
            "{}",
            r#"{"kind":"something-else","nonce":"1"}"#,
            r#"{"kind":42,"nonce":"1"}"#,
            r#""signed-out""#,
            r#"{"nonce":"1"}"#,
        ] {
            assert_eq!(parse(raw), None, "{raw} should not be taken as a signal");
        }
    }

    #[test]
    fn the_channel_and_storage_names_are_namespaced() {
        // Both namespaces are origin-wide and shared with every other script
        // on the origin.
        assert!(SIGNAL_CHANNEL.starts_with("thunderforge-cache"));
        assert!(SIGNAL_STORAGE_KEY.starts_with("thunderforge-cache"));
    }
}
