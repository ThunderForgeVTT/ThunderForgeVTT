//! The browser half of the canvas asset cache: the handles it runs on, and
//! the resolve path a canvas image load ends in.

#[path = "wasm_api.rs"]
mod api;

#[path = "wasm_sync.rs"]
mod sync_api;
pub use sync_api::*;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use bevy::prelude::*;
use futures_util::FutureExt;
use futures_util::future::Shared;
use thunderforge_cache_browser::index::IndexStore;
use thunderforge_cache_browser::opfs::{OpfsStore, UserScope};
use thunderforge_cache_browser::outbox::OutboxStore;
use thunderforge_cache_browser::prefetch::{PrefetchItem, PrefetchQueue, Pressure, Step};
use thunderforge_cache_browser::{CacheSignal, IndexEntry, crypto};
use thunderforge_cache_browser::{locks, peer, signal, sync};
use thunderforge_cache_core::delta::SyncPlan;
use thunderforge_cache_core::{Fingerprint, ItemId, fingerprint};
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use super::{Control, Delivery, Origin, ResolveRequest, control_queue, deliveries};

/// The browser-side handles the cache runs on.
///
/// Thread-local rather than a Bevy resource because none of these are
/// `Send + Sync` — `OpfsStore` holds a `FileSystemDirectoryHandle` and
/// `SessionKey` a `CryptoKey`, both `JsValue`s. wasm is single-threaded,
/// so a thread-local is the honest representation; an `unsafe impl Send`
/// over browser objects would be a lie told to satisfy a bound.
struct Handles {
    store: OpfsStore,
    key: crypto::SessionKey,
    /// Taken for the duration of an index write and put back after.
    ///
    /// `IndexStore::tick` needs `&mut`, and two resolves can be in
    /// flight at once, so this is a non-blocking mutex: whoever finds it
    /// empty opens their own rather than waiting. That costs an extra
    /// IndexedDB open in the rare overlap and keeps the seq monotonic
    /// (a fresh `open` reseeds it from the stored rows).
    index: RefCell<Option<IndexStore>>,
}

/// An open in progress, kept so concurrent askers share one.
type LocalOpen = Pin<Box<dyn Future<Output = Option<Rc<Handles>>>>>;
type PendingOpen = Shared<LocalOpen>;

thread_local! {
    static HANDLES: RefCell<Option<Rc<Handles>>> = const { RefCell::new(None) };
    static OPENING: RefCell<Option<(UserScope, PendingOpen)>> =
        const { RefCell::new(None) };
    /// FR-024: whether a fetched blob may be written.
    ///
    /// Duplicated here rather than read off `CanvasAssetCache` because
    /// the write happens in a `spawn_local` task, which has no access to
    /// the Bevy world — the same reason `HANDLES` lives here. The
    /// resource keeps its own copy for the main-thread side; both are set
    /// from the one budget pass, so they cannot disagree about anything
    /// except for the frame it takes the control queue to drain.
    ///
    /// Starts `true` for the reason the resource's does: the permission
    /// to store is the pre-budget behaviour, and a budget can only take
    /// it away.
    static STORABLE: Cell<bool> = const { Cell::new(true) };
    /// FR-073: the world the user currently has open.
    ///
    /// The prefetcher is a spawned task that outlives the sync which
    /// started it, so it cannot read the open world off the ECS resource
    /// — and must not assume the world it was created for is still the
    /// one on screen. Set from `configure_canvas_asset_cache`, which
    /// every world open passes through.
    static OPEN_WORLD: Cell<Option<Uuid>> = const { Cell::new(None) };
    /// FR-070/FR-072: which plan is current.
    ///
    /// Bumped by every sync and by every live fingerprint update. A
    /// prefetch queue carries the epoch it was built under and stops when
    /// it no longer matches, which is what keeps a superseded plan from
    /// continuing to spend bandwidth on fingerprints the server has
    /// already moved past.
    static PLAN_EPOCH: Cell<u64> = const { Cell::new(0) };
    /// FR-070: user-initiated loads outstanding right now.
    ///
    /// Incremented for the whole of a `resolve` — the cache read and any
    /// network fetch behind it — because both are work somebody is
    /// watching a blank canvas waiting for. The prefetcher does not go
    /// through `resolve`, so it cannot count itself and yield to its own
    /// traffic.
    static DEMAND_IN_FLIGHT: Cell<usize> = const { Cell::new(0) };
}

/// Record which world is open (FR-073).
pub(super) fn note_open_world(world_id: Uuid) {
    OPEN_WORLD.with(|slot| slot.set(Some(world_id)));
}

/// Mark every queued speculative plan stale (FR-070).
pub(super) fn note_live_update() {
    PLAN_EPOCH.with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
}

/// Take the next plan epoch, for a queue about to be built from a plan
/// that has just arrived.
fn begin_plan() -> u64 {
    PLAN_EPOCH.with(|epoch| {
        let next = epoch.get().wrapping_add(1);
        epoch.set(next);
        next
    })
}

/// Counts one user-initiated load for as long as it is in flight.
///
/// A guard rather than a pair of calls because the paths it wraps have
/// several early returns, and a demand fetch that returns early without
/// decrementing would leave the prefetcher yielding to a load that
/// finished minutes ago — a cache that silently never warms, which is
/// precisely the failure nobody notices.
struct DemandGuard;

impl DemandGuard {
    fn new() -> Self {
        DEMAND_IN_FLIGHT.with(|n| n.set(n.get().saturating_add(1)));
        Self
    }
}

impl Drop for DemandGuard {
    fn drop(&mut self) {
        DEMAND_IN_FLIGHT.with(|n| n.set(n.get().saturating_sub(1)));
    }
}

/// Everything outside the queue that decides what it may do next.
///
/// Every field is read at the moment it is asked for. That is the point:
/// all four of these move while a prefetch is draining, and a queue
/// deciding on a snapshot taken when the sync finished would be deciding
/// on a tab that no longer exists.
fn pressure(fallback_world: Uuid, in_use: u64, limit: u64) -> Pressure {
    Pressure {
        open_world: OPEN_WORLD.with(Cell::get).unwrap_or(fallback_world),
        plan_epoch: PLAN_EPOCH.with(Cell::get),
        demand_in_flight: DEMAND_IN_FLIGHT.with(Cell::get),
        in_use_bytes: in_use,
        limit_bytes: limit,
        may_store: may_store(),
    }
}

/// Hand the event loop back for `ms`, so nothing here occupies a turn the
/// active scene wanted.
///
/// Reached through `js_sys::Reflect` rather than `web-sys` for the same
/// reason `cookie_value` is: this crate does not depend on `web-sys`, and
/// must not assume it is running on a `Window`. A scope with no
/// `setTimeout` resolves immediately rather than leaving the prefetch
/// task suspended forever on a promise nothing will settle.
async fn yield_for(ms: i32) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        match js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        {
            Some(set_timeout) => {
                let _ = set_timeout.call2(&global, &resolve, &JsValue::from_f64(f64::from(ms)));
            }
            None => {
                let _ = resolve.call0(&JsValue::NULL);
            }
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Record the latest budget verdict for the write path (FR-024).
fn set_storable(storable: bool) {
    STORABLE.with(|flag| flag.set(storable));
}

/// Whether there is room to file what was just fetched.
fn may_store() -> bool {
    STORABLE.with(|flag| flag.get())
}

/// Open OPFS and the session key, then mark the cache usable.
///
/// Every failure here ends at `Readiness::Unavailable`, which is not an
/// error state so much as "this browser does not have the feature". The
/// user gets today's load times and no diagnostic beyond one log line.
pub(super) fn open_backing_store(scope: String) {
    // Before anything is opened, so that a tab which is *about* to hold a
    // key is already listening for the news that it should not.
    listen_for_sign_out();
    spawn_local(async move {
        let Ok(scope) = UserScope::new(scope) else {
            warn!(target: "cached_assets", "cache disabled: bad user scope");
            mark_unavailable();
            return;
        };
        ensure_handles(scope).await;
    });
}

/// The handles for `scope`, opening them if this is the first ask.
///
/// Two callers can want them at once — a `Configure` drained on the main
/// thread and a sync triggered from JS in the same tick — and opening
/// twice is not merely wasteful: `crypto::load_or_create` would race
/// itself, generate two keys, and leave everything written under the
/// loser unreadable. So the in-flight open is shared, and the second
/// caller awaits the first's result rather than starting its own.
async fn ensure_handles(scope: UserScope) -> Option<Rc<Handles>> {
    if let Some(existing) = HANDLES.with(|slot| slot.borrow().clone())
        && existing.store.scope() == &scope
    {
        return Some(existing);
    }

    let pending = OPENING.with(|slot| {
        let mut slot = slot.borrow_mut();
        // A pending open for a *different* scope is not ours to wait on;
        // the user changed, and their directory is a different one.
        if let Some((pending_scope, future)) = slot.as_ref()
            && *pending_scope == scope
        {
            return future.clone();
        }
        let future: PendingOpen = (Box::pin(open_handles(scope.clone())) as LocalOpen).shared();
        *slot = Some((scope.clone(), future.clone()));
        future
    });

    let handles = pending.await;
    OPENING.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(pending, _)| *pending == scope) {
            *slot = None;
        }
    });
    handles
}

async fn open_handles(scope: UserScope) -> Option<Rc<Handles>> {
    let key = match crypto::load_or_create(&scope).await {
        Ok(key) => key,
        Err(err) => {
            warn!(target: "cached_assets", "cache disabled: no session key: {err}");
            mark_unavailable();
            return None;
        }
    };
    let store = match OpfsStore::open(scope).await {
        Ok(store) => store,
        Err(err) => {
            warn!(target: "cached_assets", "cache disabled: no OPFS: {err}");
            mark_unavailable();
            return None;
        }
    };
    let index = match IndexStore::open().await {
        Ok(index) => Some(index),
        // A missing index does not stop reads or writes; it only
        // means the first write opens its own. Worth a line, not a
        // shutdown.
        Err(err) => {
            warn!(target: "cached_assets", "cache index unavailable: {err}");
            None
        }
    };
    let handles = Rc::new(Handles {
        store,
        key,
        index: RefCell::new(index),
    });
    HANDLES.with(|slot| {
        *slot.borrow_mut() = Some(handles.clone());
    });
    mark_ready();
    info!(target: "cached_assets", "canvas asset cache ready");
    Some(handles)
}

/// Subscribe to the cross-tab sign-out signal (FR-021b).
///
/// # Why storage deletion is not enough
///
/// `discardWorldCache` deletes the stored `CryptoKey` record, and for a
/// tab that has yet to read that record, that is the end of it. This tab
/// is not that tab. It is holding a live `CryptoKey` in `Handles` — the
/// key survives page loads by design (SC-002) — and it will go on
/// decrypting cached blobs until something makes it stop. Nothing in the
/// storage layer can make it stop, because it never looks at storage
/// again. Hence a signal.
///
/// Registered once per page and never unregistered; see
/// [`signal::listen`].
fn listen_for_sign_out() {
    signal::listen(|signal| match signal {
        CacheSignal::SignedOut => {
            info!(target: "cached_assets", "another tab signed out; dropping the cache key");
            forget_in_memory();
        }
    });
}

/// Drop everything this tab holds that could still read the cache.
///
/// The whole of what a *receiving* tab has to do, and deliberately no
/// more: the tab that initiated the sign-out has already deleted the
/// stored key and started the reclamation, and repeating either here
/// would be a second delete of an already-deleted record and a second
/// pass over a directory being removed. What only this tab can do is let
/// go of the `CryptoKey` in its own memory and stop pointing the read
/// path at content it is no longer entitled to.
///
/// Idempotent, because both signal carriers may deliver — and because
/// forgetting twice is forgetting.
fn forget_in_memory() {
    HANDLES.with(|slot| slot.borrow_mut().take());
    OPENING.with(|slot| slot.borrow_mut().take());
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::Forget);
    }
}

/// Resolve one asset: local first, network second, store on the way back.
pub(super) fn spawn_resolve(request: ResolveRequest) {
    spawn_local(async move { resolve(request).await });
}

async fn resolve(request: ResolveRequest) {
    // FR-070. Held for the whole resolve, cache read included: the user
    // is looking at an empty sprite until this finishes, and a prefetch
    // that ran alongside it would be competing for the connection they
    // are waiting on.
    let _demand = DemandGuard::new();

    let Some(handles) = HANDLES.with(|slot| slot.borrow().clone()) else {
        // Configured but the store vanished. Not expected; still a
        // network fetch rather than a broken image.
        fetch_and_deliver(&request, None).await;
        return;
    };

    match handles
        .store
        .read_blob(request.world_id, &request.fingerprint, &handles.key)
        .await
    {
        // A hit. `read_blob` has already decrypted the blob and verified
        // it against its own filename, so these bytes are as trusted as
        // anything the server could hand us.
        Ok(Some(bytes)) => {
            touch_index(&handles, request.asset_id).await;
            deliver(&request, bytes, Origin::Cache);
            return;
        }
        // Absent, undecryptable, or corrupt — indistinguishable by
        // design (FR-016c), and all three mean "fetch it".
        Ok(None) => {}
        Err(err) => {
            warn!(
                target: "cached_assets",
                "cache read failed for {}: {err}", request.url,
            );
        }
    }

    fetch_and_deliver(&request, Some(handles)).await;
}

/// The fallback every degradation lands on: fetch the same URL Bevy
/// would have, verify it if we hold a promise, store it if it verifies.
async fn fetch_and_deliver(request: &ResolveRequest, handles: Option<Rc<Handles>>) {
    // FR-044/FR-048. A peer is asked first and is never waited on: every
    // way this can go wrong — no peer, a declining peer, a slow peer, a
    // peer sending bytes that hash to something else — answers `None`
    // and lands on exactly the fetch below, which is what this function
    // did before peers existed. The bytes it does return have already
    // passed `fingerprint::verify` inside `PeerDownload`; they are
    // re-verified below with the network's, because one trust choke
    // point is the rule and two callers of it is not a second one.
    let from_peer = peer::try_fetch(request.fingerprint).await;
    let peer_supplied = from_peer.is_some();

    let bytes = match from_peer {
        Some(bytes) => bytes,
        None => match fetch(&request.url).await {
            Some(bytes) => bytes,
            None => {
                warn!(target: "cached_assets", "fetch failed for {}", request.url);
                return;
            }
        },
    };

    // The single sanctioned trust choke point. Nothing here compares a
    // digest by hand, and nothing is stored that did not pass.
    if fingerprint::verify(&bytes, &request.fingerprint).is_err() {
        warn!(
            target: "cached_assets",
            "{} did not match its promised fingerprint; rendering it, not caching it",
            request.url,
        );
        deliver(request, bytes, Origin::NetworkUnverified);
        return;
    }

    // FR-024, the "fetch without storing" degradation. The budget pass
    // found that even releasing everything permissible leaves no room, so
    // the bytes are delivered and not filed. This is the one branch here
    // that is not a failure: the user sees their content, and the store
    // is left holding what it already had rather than thrashing.
    let origin = if peer_supplied {
        Origin::Peer
    } else {
        Origin::Network
    };

    if !may_store() {
        debug!(
            target: "cached_assets",
            "no room to cache {}; delivering without storing", request.url,
        );
        deliver(request, bytes, origin);
        return;
    }

    if let Some(handles) = handles {
        // FR-021c: the same per-world lock `sync::apply_plan` takes, so
        // this write does not land in the middle of another tab's
        // eviction pass. Short-waited and ignored if refused — the bytes
        // are already in hand and the user is waiting for them, so the
        // worst outcome of going ahead unlocked is the race this path
        // has always had.
        let _lock = locks::acquire_exclusive(
            &locks::world_sync_lock(request.world_id),
            locks::WRITE_LOCK_TIMEOUT_MS,
        )
        .await;

        // `write_blob` re-verifies before anything is encrypted, so a
        // bug between here and there cannot file bad bytes under a good
        // name. Failure to store is a slower next visit, nothing more.
        match handles
            .store
            .write_blob(request.world_id, &request.fingerprint, &bytes, &handles.key)
            .await
        {
            Ok(_) => {
                record_index(&handles, request, bytes.len() as u64).await;
                // Now held and verified, so it may be served on (T091).
                // After the store, never before: announcing it earlier
                // would promise a peer bytes that are not there yet.
                peer::note_stored(request.fingerprint);
            }
            Err(err) => {
                warn!(target: "cached_assets", "could not cache {}: {err}", request.url);
            }
        }
    }

    deliver(request, bytes, origin);
}

/// Same-origin GET of the authenticated `/canvas-assets/{id}` route.
///
/// `fetch` defaults to `credentials: "same-origin"`, which is what
/// carries the session cookie the route authenticates on — the same
/// reason Bevy's own wasm asset reader works against it today.
async fn fetch(url: &str) -> Option<Vec<u8>> {
    let response = gloo_net::http::Request::get(url).send().await.ok()?;
    if !response.ok() {
        return None;
    }
    response.binary().await.ok()
}

/// Hand bytes back to the main thread for decoding.
fn deliver(request: &ResolveRequest, bytes: Vec<u8>, origin: Origin) {
    // Counted here rather than at the drain, because this is the point
    // every path — hit, fetch, peer, unverified — actually funnels
    // through. Counting at `drain_deliveries` would silently omit any
    // delivery whose image failed to decode, and "the bytes crossed the
    // wire" is the fact FR-051 reports on, not "the picture appeared".
    super::record_delivery(origin, bytes.len() as u64);
    if let Ok(mut queue) = deliveries().lock() {
        queue.push(Delivery {
            handle: request.handle.clone(),
            bytes,
            extension: request.extension.clone(),
            origin,
            url: request.url.clone(),
        });
    }
}

/// Record what we now hold, so eviction, the budget and the delta
/// manifest can all see it. A blob with no index row is invisible to all
/// three and would eventually be collected as an orphan.
async fn record_index(handles: &Rc<Handles>, request: &ResolveRequest, byte_size: u64) {
    let Some(mut index) = borrow_index(handles).await else {
        return;
    };
    let seq = index.tick();
    let entry = IndexEntry::new(request.fingerprint, byte_size, request.world_id, seq);
    if let Err(err) = index
        .put(ItemId::CanvasAsset(request.asset_id), &entry)
        .await
    {
        warn!(target: "cached_assets", "could not index {}: {err}", request.url);
    }
    return_index(handles, index);
}

/// Move an item to the back of the LRU queue on a cache hit.
async fn touch_index(handles: &Rc<Handles>, asset_id: Uuid) {
    let Some(mut index) = borrow_index(handles).await else {
        return;
    };
    let _ = index.touch(ItemId::CanvasAsset(asset_id)).await;
    return_index(handles, index);
}

async fn borrow_index(handles: &Rc<Handles>) -> Option<IndexStore> {
    if let Some(index) = handles.index.borrow_mut().take() {
        return Some(index);
    }
    IndexStore::open().await.ok()
}

fn return_index(handles: &Rc<Handles>, index: IndexStore) {
    *handles.index.borrow_mut() = Some(index);
}

/// Where `worldSyncPlan` is served. Same-origin and rooted, so the
/// session cookie rides along exactly as it does for the asset route —
/// the engine never holds a token of its own.
const GRAPHQL_ENDPOINT: &str = "/api/graphql";

/// The authenticated byte route, matching the paths TypeScript already
/// hands the engine for canvas images (`WorldPage.tsx`). Extensions are
/// optional on that route but kept, because Bevy chooses its decoder
/// from them.
const ASSET_URL_PREFIX: &str = "/api/canvas-assets";
