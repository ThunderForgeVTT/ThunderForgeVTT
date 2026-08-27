//! Canvas image reads routed through the local encrypted cache before the
//! network.
//!
//! Spec `028-client-world-cache` T027, ADR-052, research.md R1.
//!
//! Research R1 put the asset read path in the engine for one concrete
//! reason: canvas image bytes are fetched by Bevy's `AssetServer` from
//! inside WASM (`GET /canvas-assets/{id}.webp`, see
//! `src/server/src/canvas_assets_serve.rs`), not by TypeScript. A cache
//! that intercepted at the TS layer would miss the single largest category
//! of bytes this feature exists to stop re-transferring. This module is
//! where that interception actually happens.
//!
//! # The seam: why bytes-and-`Assets<Image>` rather than an `AssetReader`
//!
//! `AssetServer::load()` takes a path and does its own fetching, so it does
//! not consult a cache on its own. There were two ways in, and this module
//! deliberately takes the second.
//!
//! **Rejected — a custom [`bevy::asset::io::AssetReader`] behind an
//! `AssetSource`.** It is the idiomatic Bevy answer and R1 named it as a
//! candidate mechanism. Three facts rule it out here, and all three are
//! properties of Bevy 0.18 rather than opinions:
//!
//! 1. *It cannot be registered from a plugin added after `DefaultPlugins`.*
//!    `App::register_asset_source` checks for an existing `AssetServer`
//!    resource and logs `"… must be registered before AssetPlugin"` when it
//!    finds one, because `AssetPlugin::build` has already consumed the
//!    builder map. Registering it earlier would mean `lib.rs` doing part of
//!    this plugin's job before the plugin exists — precisely the coupling
//!    Constitution Principle II forbids, and it would break the
//!    "remove the line, get today's behaviour" property below.
//! 2. *The trait is `Send + Sync + 'static`.* The state a cache read needs —
//!    `FileSystemDirectoryHandle`, `CryptoKey` — is `JsValue`, which is
//!    neither. Satisfying the bound would mean a thread-local side channel
//!    reached into from inside the reader, i.e. the same global state as
//!    below with an extra trait in the way.
//! 3. *An `AssetReader` is handed a path and nothing else.* The cache is
//!    keyed by fingerprint and every byte-accepting path has to verify
//!    against the fingerprint **the server promised**. There is nowhere in
//!    the `read(&self, path)` signature to carry that promise, so a reader
//!    would have to consult a global map anyway to know what it was even
//!    checking against.
//!
//! **Chosen — fetch the bytes here and insert them into `Assets<Image>`.**
//! [`Assets::reserve_handle`] hands out a `Handle<Image>` for content that
//! has not arrived yet, so the caller gets a handle synchronously (exactly
//! what `AssetServer::load` gives it today) while an async task resolves
//! the bytes and [`Assets::insert`]s the decoded image against that
//! handle's id. Decoding happens on the main thread via
//! [`Image::from_buffer`], which is where Bevy's own `ImageLoader` decodes
//! on wasm regardless — there are no real threads there without
//! `SharedArrayBuffer` — so this changes *where the bytes came from* and
//! nothing about when the frame cost lands.
//!
//! # Removability (Constitution Principle II)
//!
//! This plugin owns exactly one resource, [`CanvasAssetCache`], and callers
//! reach it through [`load_canvas_image`] with an `Option<ResMut<…>>`. Drop
//! `CachedAssetsPlugin` from the `App` builder and that option is `None` on
//! every call, so every load is a plain `asset_server.load(path)` — today's
//! behaviour, byte for byte. Nothing outside this module reads the cache's
//! state, and this module reads nothing of anyone else's.
//!
//! # Degradation
//!
//! A cache problem must never become a failed asset load; the worst it may
//! cost is the network fetch that would have happened anyway. So *every*
//! one of these falls through to a plain fetch of the same URL:
//!
//! - the plugin is not registered, or the browser has no OPFS/WebCrypto;
//! - the cache has not been told who the user is or which world is open;
//! - no server-promised fingerprint is known for the asset;
//! - the blob is absent, will not decrypt, or does not hash to its own
//!   filename (all three are `Ok(None)` from `read_blob` by design);
//! - the fetched bytes do not match the promised fingerprint.
//!
//! The last one is worth being explicit about. The bytes came from the
//! authenticated, world-authorized route, so they are rendered; what does
//! *not* happen is storing them, because a fingerprint we cannot reproduce
//! is one we could never invalidate against. That is a stale promise on our
//! side, not a reason to show the user a broken map.
//!
//! # Trust
//!
//! Every byte-accepting path goes through
//! [`thunderforge_cache_core::fingerprint::verify`], and only through it:
//! reads are verified inside `OpfsStore::read_blob`, writes inside
//! `OpfsStore::write_blob`, and the network response explicitly below.
//! Nothing here compares a digest by hand.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use bevy::asset::AssetId;
use bevy::prelude::*;
use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;

/// Routes canvas image loads through the local cache, falling back to the
/// network on anything unexpected.
///
/// Add it after `DefaultPlugins` (it needs `Assets<Image>` to exist) and
/// before nothing in particular — no other plugin depends on it, by design.
pub struct CachedAssetsPlugin;

impl Plugin for CachedAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CanvasAssetCache>()
            .add_systems(Update, (drain_control_queue, drain_deliveries).chain());
    }
}

/// How far along the cache is in becoming usable.
///
/// `Unavailable` is terminal for the scope that produced it, on purpose: a
/// browser that has no OPFS will not grow one mid-session, and retrying every
/// frame would turn a supported degradation into a log flood. Only a change
/// of user scope reopens the question.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Readiness {
    /// Nobody has said who the user is or which world is open yet.
    #[default]
    Unconfigured,
    /// Opening OPFS and the session key.
    Opening,
    /// Usable.
    Ready,
    /// The platform cannot support the cache. Never retried.
    Unavailable,
}

/// The plugin's entire observable state.
///
/// Deliberately plain data: no `JsValue`, no OPFS handle, nothing that is
/// not `Send + Sync`. The browser-side handles live in a thread-local in the
/// wasm module below, because they cannot satisfy `Resource`'s bounds and
/// pretending otherwise would mean unsafe impls over browser objects.
#[derive(Resource, Default)]
pub struct CanvasAssetCache {
    readiness: Readiness,
    /// The OPFS scope currently open, if any.
    ///
    /// Held so a *change* of user is detected: cache paths are scoped per
    /// user (FR-016, T036), and continuing to read the previous user's
    /// directory after a session switch is a disclosure bug, not a stale
    /// cache. A new scope reopens the backing store from scratch.
    scope: Option<String>,
    /// The world whose blobs we may read and write. Cache paths are scoped
    /// per world, so being wrong about this reads someone else's directory —
    /// hence `Option`, and hence no default.
    world_id: Option<Uuid>,
    /// What the server most recently promised for each canvas asset.
    ///
    /// Absent means "no promise", which means no cache participation at all
    /// for that asset — not "not cached". The two are different: a fetch
    /// with nothing to verify against cannot safely be stored.
    fingerprints: HashMap<Uuid, Fingerprint>,
    /// Ids of images this module has already resolved, keyed by the identity
    /// they were resolved under.
    ///
    /// This is a handle-reuse table, **not** a second texture cache: it
    /// stores `AssetId`s, and a hit is only honoured while
    /// [`Assets::get_strong_handle`] confirms the image is still resident.
    /// Once the last strong handle elsewhere drops and Bevy frees the
    /// texture, the entry stops being usable and the asset is resolved
    /// again. Ownership of texture residency stays with
    /// `BackgroundTextureCache` (Constitution Principle I).
    issued: HashMap<(Uuid, Fingerprint), AssetId<Image>>,
}

impl CanvasAssetCache {
    /// Whether the cache is in a state where a lookup could succeed.
    pub fn is_ready(&self) -> bool {
        self.readiness == Readiness::Ready && self.world_id.is_some()
    }
}

/// Load a canvas image, through the cache when one is present.
///
/// The `cache` argument is an `Option` because that is what removability
/// looks like at a call site: without [`CachedAssetsPlugin`] the resource
/// does not exist, the option is `None`, and this is `asset_server.load`
/// with extra steps.
pub(crate) fn load_canvas_image(
    path: &str,
    cache: Option<&mut CanvasAssetCache>,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
) -> Handle<Image> {
    if let Some(cache) = cache
        && let Some(handle) = try_cached(cache, path, images)
    {
        return handle;
    }
    asset_server.load(path.to_owned())
}

/// A canvas asset URL split into the parts the cache needs.
#[derive(Clone, PartialEq, Eq, Debug)]
struct CanvasAssetPath {
    asset_id: Uuid,
    /// The image extension, passed straight to `ImageType::Extension` so the
    /// decoder is chosen the same way Bevy's `ImageLoader` chooses it.
    extension: String,
}

/// Recognise `…/canvas-assets/<uuid>.<ext>` and nothing else.
///
/// Strict by intent. This module may only touch bytes served by
/// `canvas_assets_serve`, because that route is the one whose bytes the
/// server fingerprints (T017). Anything else — token art on an arbitrary
/// URL, a bundled asset — must reach `AssetServer` untouched, so an
/// unrecognised path returns `None` rather than being guessed at.
fn parse_canvas_asset_path(path: &str) -> Option<CanvasAssetPath> {
    let path = path.split(['?', '#']).next()?;
    let (prefix, segment) = path.rsplit_once('/')?;
    if !prefix.ends_with("canvas-assets") {
        return None;
    }
    let (id, extension) = segment.rsplit_once('.')?;
    Some(CanvasAssetPath {
        asset_id: Uuid::parse_str(id).ok()?,
        extension: extension.to_ascii_lowercase(),
    })
}

/// Try to satisfy a load from the cache, spawning the resolve task if the
/// bytes are not in hand yet.
///
/// `None` means "not our problem" and is the caller's cue to fall back. It
/// is returned for every condition in the module docs' degradation list.
fn try_cached(
    cache: &mut CanvasAssetCache,
    path: &str,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if !cache.is_ready() {
        return None;
    }
    let parsed = parse_canvas_asset_path(path)?;
    let world_id = cache.world_id?;
    let fingerprint = *cache.fingerprints.get(&parsed.asset_id)?;

    let key = (parsed.asset_id, fingerprint);
    if let Some(id) = cache.issued.get(&key).copied() {
        if let Some(handle) = images.get_strong_handle(id) {
            return Some(handle);
        }
        // The image was freed. The entry is now a lie, so drop it and
        // resolve again rather than handing back a dangling id.
        cache.issued.remove(&key);
    }

    let handle = images.reserve_handle();
    cache.issued.insert(key, handle.id());
    spawn_resolve(ResolveRequest {
        world_id,
        asset_id: parsed.asset_id,
        fingerprint,
        url: path.to_owned(),
        extension: parsed.extension,
        handle: handle.clone(),
    });
    Some(handle)
}

/// Everything the async resolve needs, gathered on the main thread so the
/// task borrows nothing from the ECS.
struct ResolveRequest {
    world_id: Uuid,
    asset_id: Uuid,
    fingerprint: Fingerprint,
    url: String,
    extension: String,
    handle: Handle<Image>,
}

/// Where the bytes for a delivery came from. Logged, never branched on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Origin {
    /// Read from OPFS, decrypted, and verified against its own filename.
    Cache,
    /// Fetched and verified against the server's promise, then stored.
    Network,
    /// Fetched, but the promise we held did not match. Rendered, not stored.
    NetworkUnverified,
}

/// Bytes on their way back to the main thread.
struct Delivery {
    handle: Handle<Image>,
    bytes: Vec<u8>,
    extension: String,
    origin: Origin,
    url: String,
}

/// An instruction from outside the engine, waiting to be applied.
///
/// A queue rather than a direct write because the callers are JS entry
/// points with no `World` in hand — the same shape `lib.rs` already uses for
/// `apply_world_command`.
enum Control {
    /// Who the user is and which world is open.
    Configure { scope: String, world_id: Uuid },
    /// The server's current fingerprints for a set of canvas assets.
    Fingerprints(Vec<(Uuid, Fingerprint)>),
    /// The backing store finished opening: usable, or not on this browser.
    ///
    /// Decided off the main thread and applied here, because the resource is
    /// the only thing allowed to hold the answer.
    Readiness(bool),
}

static CONTROL_QUEUE: OnceLock<Mutex<Vec<Control>>> = OnceLock::new();
static DELIVERIES: OnceLock<Mutex<Vec<Delivery>>> = OnceLock::new();

fn control_queue() -> &'static Mutex<Vec<Control>> {
    CONTROL_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn deliveries() -> &'static Mutex<Vec<Delivery>> {
    DELIVERIES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Point the cache at a user and a world.
///
/// `user_scope` is the value `UserScope` will confine every OPFS path to;
/// `world_id` must be a UUID. Called by the web client once a session and an
/// open world are both known. Until it is, the cache stays `Unconfigured`
/// and every load is a plain fetch — there is no default scope, because
/// guessing one would file one user's bytes under another's directory.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn configure_canvas_asset_cache(user_scope: &str, world_id: &str) {
    let Ok(world_id) = Uuid::parse_str(world_id) else {
        warn!(target: "cached_assets", "ignoring cache config: {world_id} is not a uuid");
        return;
    };
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::Configure {
            scope: user_scope.to_owned(),
            world_id,
        });
    }
}

/// Publish the server's current fingerprints for canvas assets.
///
/// Takes a JSON object of `{"<asset uuid>": "<64 hex chars>"}` — the shape
/// `worldSyncPlan` already speaks in (contracts/graphql-delta-sync.md).
/// Entries that do not parse are dropped individually: a malformed
/// fingerprint costs that one asset its cache participation, and taking the
/// whole batch down over it would cost every asset theirs.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_canvas_asset_fingerprints(json: &str) {
    let parsed: Result<HashMap<String, String>, _> = serde_json::from_str(json);
    let Ok(parsed) = parsed else {
        warn!(target: "cached_assets", "ignoring fingerprints: not a json object of id->hex");
        return;
    };
    let entries: Vec<(Uuid, Fingerprint)> = parsed
        .into_iter()
        .filter_map(|(id, hex)| {
            Some((
                Uuid::parse_str(&id).ok()?,
                Fingerprint::from_hex(&hex).ok()?,
            ))
        })
        .collect();
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::Fingerprints(entries));
    }
}

/// Applies whatever the JS entry points above have queued.
fn drain_control_queue(mut cache: ResMut<CanvasAssetCache>) {
    let Ok(mut queue) = control_queue().lock() else {
        return;
    };
    for control in queue.drain(..) {
        match control {
            Control::Configure { scope, world_id } => {
                if cache.world_id != Some(world_id) {
                    // Issued ids are only meaningful within the world they
                    // were resolved under; carrying them across a world
                    // change would hand out another world's texture.
                    cache.issued.clear();
                    cache.world_id = Some(world_id);
                }
                // A new scope means a different user, so nothing already
                // resolved may be reused and the store must be reopened —
                // even from `Unavailable`, which was a verdict about a
                // scope, not about the browser.
                if cache.scope.as_deref() != Some(scope.as_str()) {
                    cache.scope = Some(scope.clone());
                    cache.issued.clear();
                    cache.fingerprints.clear();
                    cache.readiness = Readiness::Opening;
                    open_backing_store(scope);
                }
            }
            Control::Fingerprints(entries) => {
                for (asset_id, fingerprint) in entries {
                    // A superseded fingerprint invalidates the handle issued
                    // under the old one; `issued` is keyed by both, so the
                    // stale row is simply never consulted again. Removing it
                    // eagerly is not worth a scan.
                    cache.fingerprints.insert(asset_id, fingerprint);
                }
            }
            Control::Readiness(ready) => {
                cache.readiness = if ready {
                    Readiness::Ready
                } else {
                    Readiness::Unavailable
                };
            }
        }
    }
}

/// Decodes resolved bytes and inserts them against the handle the caller has
/// been holding since it asked for them.
///
/// Decode failure leaves the handle empty, which is the same visible outcome
/// as a failed `AssetServer` load — a load that did not produce an image.
fn drain_deliveries(mut images: ResMut<Assets<Image>>) {
    let drained: Vec<Delivery> = match deliveries().lock() {
        Ok(mut queue) if !queue.is_empty() => queue.drain(..).collect(),
        _ => return,
    };

    for delivery in drained {
        let byte_len = delivery.bytes.len();
        match decode_image(&delivery.bytes, &delivery.extension) {
            Ok(image) => {
                if images.insert(delivery.handle.id(), image).is_err() {
                    warn!(
                        target: "cached_assets",
                        "dropped {} bytes for {}: handle generation expired",
                        byte_len, delivery.url,
                    );
                    continue;
                }
                debug!(
                    target: "cached_assets",
                    "{:?}: {} ({byte_len} bytes)", delivery.origin, delivery.url,
                );
            }
            Err(err) => {
                warn!(
                    target: "cached_assets",
                    "could not decode {} ({byte_len} bytes, {:?}): {err}",
                    delivery.url, delivery.origin,
                );
            }
        }
    }
}

/// Decode image bytes the way Bevy's own `ImageLoader` would.
///
/// The four constants match `ImageLoaderSettings::default()`: sRGB, the
/// default sampler, main-and-render-world usage, and no compressed formats
/// (the engine enables neither basis-universal nor ktx2). Diverging from
/// them here would make a cached image render differently from the same
/// image loaded through `AssetServer` — the one difference this seam must
/// never introduce.
fn decode_image(bytes: &[u8], extension: &str) -> Result<Image, String> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};

    Image::from_buffer(
        bytes,
        ImageType::Extension(extension),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .map_err(|err| err.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn open_backing_store(_scope: String) {
    // Off wasm there is no OPFS to open. Nothing calls this — the engine
    // only ships to wasm32 — but keeping the module compiling natively is
    // what lets the path parsing below be tested under plain `cargo test`.
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_resolve(_request: ResolveRequest) {}

#[cfg(target_arch = "wasm32")]
use wasm::{open_backing_store, spawn_resolve};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::RefCell;
    use std::rc::Rc;

    use bevy::prelude::*;
    use thunderforge_cache_browser::index::IndexStore;
    use thunderforge_cache_browser::opfs::{OpfsStore, UserScope};
    use thunderforge_cache_browser::{IndexEntry, crypto};
    use thunderforge_cache_core::{ItemId, fingerprint};
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

    thread_local! {
        static HANDLES: RefCell<Option<Rc<Handles>>> = const { RefCell::new(None) };
    }

    /// Open OPFS and the session key, then mark the cache usable.
    ///
    /// Every failure here ends at `Readiness::Unavailable`, which is not an
    /// error state so much as "this browser does not have the feature". The
    /// user gets today's load times and no diagnostic beyond one log line.
    pub(super) fn open_backing_store(scope: String) {
        spawn_local(async move {
            let scope = match UserScope::new(scope) {
                Ok(scope) => scope,
                Err(err) => {
                    warn!(target: "cached_assets", "cache disabled: bad user scope: {err}");
                    mark_unavailable();
                    return;
                }
            };
            let key = match crypto::load_or_create(&scope).await {
                Ok(key) => key,
                Err(err) => {
                    warn!(target: "cached_assets", "cache disabled: no session key: {err}");
                    mark_unavailable();
                    return;
                }
            };
            let store = match OpfsStore::open(scope).await {
                Ok(store) => store,
                Err(err) => {
                    warn!(target: "cached_assets", "cache disabled: no OPFS: {err}");
                    mark_unavailable();
                    return;
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
            HANDLES.with(|slot| {
                *slot.borrow_mut() = Some(Rc::new(Handles {
                    store,
                    key,
                    index: RefCell::new(index),
                }));
            });
            mark_ready();
            info!(target: "cached_assets", "canvas asset cache ready");
        });
    }

    /// Resolve one asset: local first, network second, store on the way back.
    pub(super) fn spawn_resolve(request: ResolveRequest) {
        spawn_local(async move { resolve(request).await });
    }

    async fn resolve(request: ResolveRequest) {
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
        let Some(bytes) = fetch(&request.url).await else {
            warn!(target: "cached_assets", "fetch failed for {}", request.url);
            return;
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

        if let Some(handles) = handles {
            // `write_blob` re-verifies before anything is encrypted, so a
            // bug between here and there cannot file bad bytes under a good
            // name. Failure to store is a slower next visit, nothing more.
            match handles
                .store
                .write_blob(request.world_id, &request.fingerprint, &bytes, &handles.key)
                .await
            {
                Ok(_) => record_index(&handles, request, bytes.len() as u64).await,
                Err(err) => {
                    warn!(target: "cached_assets", "could not cache {}: {err}", request.url);
                }
            }
        }

        deliver(request, bytes, Origin::Network);
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

    fn mark_ready() {
        push_readiness(true);
    }

    fn mark_unavailable() {
        push_readiness(false);
    }

    /// Readiness is decided off the main thread but owned by the resource,
    /// so it travels back the same way everything else does.
    fn push_readiness(ready: bool) {
        if let Ok(mut queue) = control_queue().lock() {
            queue.push(Control::Readiness(ready));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_canvas_asset_urls() {
        let parsed =
            parse_canvas_asset_path("/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp")
                .expect("canvas asset url");
        assert_eq!(parsed.extension, "webp");
        assert_eq!(
            parsed.asset_id,
            Uuid::parse_str("2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e").unwrap()
        );
    }

    /// Bevy asks for `<uuid>.webp.meta` before the image itself. That is not
    /// a canvas asset, and routing it through the cache would look up a
    /// fingerprint for an id that does not parse.
    #[test]
    fn rejects_meta_and_foreign_paths() {
        assert!(
            parse_canvas_asset_path(
                "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp.meta"
            )
            .is_none()
        );
        assert!(parse_canvas_asset_path("/assets/tokens/goblin.png").is_none());
        assert!(parse_canvas_asset_path("/api/canvas-assets/not-a-uuid.webp").is_none());
        assert!(parse_canvas_asset_path("/api/canvas-assets/").is_none());
    }

    /// An unconfigured cache must behave exactly like no cache at all.
    #[test]
    fn unconfigured_cache_never_intercepts() {
        let mut cache = CanvasAssetCache::default();
        let mut images = Assets::<Image>::default();
        assert!(!cache.is_ready());
        assert!(
            try_cached(
                &mut cache,
                "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp",
                &mut images,
            )
            .is_none()
        );
    }

    /// Ready, but with no promise for this asset: still a plain fetch.
    #[test]
    fn unknown_fingerprint_falls_through() {
        let mut cache = CanvasAssetCache {
            readiness: Readiness::Ready,
            world_id: Some(Uuid::nil()),
            ..default()
        };
        let mut images = Assets::<Image>::default();
        assert!(cache.is_ready());
        assert!(
            try_cached(
                &mut cache,
                "/api/canvas-assets/2a3f6f2e-0f5e-4a1b-9c3d-8d0a1b2c3d4e.webp",
                &mut images,
            )
            .is_none()
        );
    }
}
