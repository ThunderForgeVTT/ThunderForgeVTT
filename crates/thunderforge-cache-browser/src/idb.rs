//! Minimal IndexedDB plumbing, shared by the key store and the index.
//!
//! Spec 028 research.md R2 (IndexedDB for the index and the outbox; no WASM
//! SQLite in v1 — a key-value workload does not justify a megabyte of
//! additional WASM in a bundle whose size is itself a user story).
//!
//! This is deliberately not a general IndexedDB wrapper. It does exactly what
//! the four stores in data-model.md need — get, put, delete, list, clear on a
//! string key — because every capability added here is a capability that has
//! to be reasoned about in a browser rather than in `cargo test`.
//!
//! `IDBRequest` is bridged to a `Promise` and then to a Rust future. That
//! detour exists because `wasm_bindgen_futures` only speaks `Promise`, and
//! hand-rolling a waker for a DOM event target would be strictly more code
//! doing strictly the same thing.

use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    IdbDatabase, IdbFactory, IdbOpenDbRequest, IdbRequest, IdbTransactionMode,
    IdbVersionChangeEvent,
};

use crate::{ALL_STORES, CacheError, DB_NAME, DB_VERSION, Result, global_property, js_err};

/// An open handle to the cache database.
pub struct Db {
    db: IdbDatabase,
}

impl Db {
    /// Open [`DB_NAME`], creating every store in [`ALL_STORES`] on first use.
    ///
    /// All four stores are created together even though this crate only reads
    /// two of them, because adding a store later requires a version bump and
    /// a `versionchange` transaction that blocks on every other open tab. A
    /// store that exists and is empty costs nothing.
    pub async fn open() -> Result<Self> {
        let factory: IdbFactory = global_property("indexedDB")?.unchecked_into();
        let request: IdbOpenDbRequest =
            factory.open_with_u32(DB_NAME, DB_VERSION).map_err(js_err)?;

        let upgrade = Closure::<dyn FnMut(IdbVersionChangeEvent)>::new(
            move |event: IdbVersionChangeEvent| {
                let Some(target) = event.target() else { return };
                let Ok(request) = target.dyn_into::<IdbOpenDbRequest>() else {
                    return;
                };
                let Ok(result) = request.result() else { return };
                let Ok(db) = result.dyn_into::<IdbDatabase>() else {
                    return;
                };
                let existing = db.object_store_names();
                for name in ALL_STORES {
                    if !(0..existing.length()).any(|i| existing.get(i).as_deref() == Some(name)) {
                        let _ = db.create_object_store(name);
                    }
                }
            },
        );
        request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));

        let db = await_request(request.unchecked_ref()).await?;
        drop(upgrade);
        let db: IdbDatabase = db
            .dyn_into()
            .map_err(|_| CacheError::Unsupported("IndexedDB"))?;
        Ok(Self { db })
    }

    /// Read one value. Absent keys are `None`, not an error — IndexedDB
    /// itself reports a miss as a successful request resolving to
    /// `undefined`, and that is the right shape for a cache.
    pub async fn get(&self, store: &str, key: &str) -> Result<Option<JsValue>> {
        let tx = self
            .db
            .transaction_with_str_and_mode(store, IdbTransactionMode::Readonly)
            .map_err(js_err)?;
        let request = tx
            .object_store(store)
            .map_err(js_err)?
            .get(&JsValue::from_str(key))
            .map_err(js_err)?;
        let value = await_request(&request).await?;
        Ok(if value.is_undefined() || value.is_null() {
            None
        } else {
            Some(value)
        })
    }

    /// Write one value under an out-of-line string key.
    ///
    /// Out-of-line keys are what let the `keys` store hold a bare `CryptoKey`
    /// (which has no writable properties to hang a key path off) and let the
    /// `index` store be keyed by an `ItemId` wire string that is not part of
    /// the stored record.
    pub async fn put(&self, store: &str, key: &str, value: &JsValue) -> Result<()> {
        let tx = self
            .db
            .transaction_with_str_and_mode(store, IdbTransactionMode::Readwrite)
            .map_err(js_err)?;
        let request = tx
            .object_store(store)
            .map_err(js_err)?
            .put_with_key(value, &JsValue::from_str(key))
            .map_err(js_err)?;
        await_request(&request).await?;
        Ok(())
    }

    /// Remove one value. Absent is success.
    pub async fn delete(&self, store: &str, key: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_str_and_mode(store, IdbTransactionMode::Readwrite)
            .map_err(js_err)?;
        let request = tx
            .object_store(store)
            .map_err(js_err)?
            .delete(&JsValue::from_str(key))
            .map_err(js_err)?;
        await_request(&request).await?;
        Ok(())
    }

    /// Every key/value pair in a store, in key order.
    pub async fn entries(&self, store: &str) -> Result<Vec<(String, JsValue)>> {
        let tx = self
            .db
            .transaction_with_str_and_mode(store, IdbTransactionMode::Readonly)
            .map_err(js_err)?;
        let object_store = tx.object_store(store).map_err(js_err)?;
        let keys = await_request(&object_store.get_all_keys().map_err(js_err)?).await?;
        let values = await_request(&object_store.get_all().map_err(js_err)?).await?;
        let keys = js_sys::Array::from(&keys);
        let values = js_sys::Array::from(&values);
        Ok((0..keys.length())
            .filter_map(|i| keys.get(i).as_string().map(|k| (k, values.get(i))))
            .collect())
    }

    /// Empty a store.
    pub async fn clear(&self, store: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_str_and_mode(store, IdbTransactionMode::Readwrite)
            .map_err(js_err)?;
        let request = tx
            .object_store(store)
            .map_err(js_err)?
            .clear()
            .map_err(js_err)?;
        await_request(&request).await?;
        Ok(())
    }
}

/// Bridge one `IDBRequest` to its result.
///
/// `Closure::once_into_js` hands ownership of each handler to JS and frees
/// the Rust side when it fires. Exactly one of the two always fires — an
/// `IDBRequest` settles or the transaction is aborted — so the unfired
/// handler is retained for the lifetime of the request object and released
/// with it, rather than leaked as `Closure::forget` would.
async fn await_request(request: &IdbRequest) -> Result<JsValue> {
    let promise = Promise::new(&mut |resolve, reject| {
        let success_request = request.clone();
        let success = Closure::once_into_js(move |_event: JsValue| {
            let value = success_request.result().unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::NULL, &value);
        });
        let error_request = request.clone();
        let failure = Closure::once_into_js(move |_event: JsValue| {
            let error = error_request
                .error()
                .ok()
                .flatten()
                .map(JsValue::from)
                .unwrap_or_else(|| JsValue::from_str("IndexedDB request failed"));
            let _ = reject.call1(&JsValue::NULL, &error);
        });
        request.set_onsuccess(Some(success.unchecked_ref()));
        request.set_onerror(Some(failure.unchecked_ref()));
    });
    JsFuture::from(promise).await.map_err(js_err)
}
