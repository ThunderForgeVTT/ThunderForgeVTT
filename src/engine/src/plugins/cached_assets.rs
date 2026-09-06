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
#[derive(Resource)]
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
    /// Whether a fetched blob may be written to disk (FR-024).
    ///
    /// Defaults to storing, so a browser that has never completed a budget
    /// pass behaves exactly as it did before budgets existed. Only a pass
    /// that actually reported `insufficient` turns it off, and the next pass
    /// that finds room turns it back on — the flag is a fact about the last
    /// measurement, never a latch.
    storable: bool,
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

/// Hand-written rather than derived for one field: `storable` must start
/// `true`.
///
/// `bool::default()` is `false`, which here would mean "refuse to cache
/// anything" — so a derived `Default` would silently disable the entire
/// feature on every browser until a budget pass happened to enable it, and
/// would disable it permanently on any browser whose quota cannot be
/// estimated. The safe default for a *permission* to store is to have it,
/// because the pre-budget behaviour was to always store and the budget can
/// only ever take that away.
impl Default for CanvasAssetCache {
    fn default() -> Self {
        Self {
            readiness: Readiness::default(),
            scope: None,
            world_id: None,
            fingerprints: HashMap::new(),
            storable: true,
            issued: HashMap::new(),
        }
    }
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
    /// Supplied by another client and verified against the server's promise
    /// before anything was done with it (FR-046). Indistinguishable in
    /// outcome from [`Origin::Network`] by design — only faster, and only
    /// sometimes (SC-013). Kept apart from it solely so the diagnostics can
    /// say where the bytes came from.
    Peer,
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
    /// The complete, authoritative set of fingerprints for the open world,
    /// replacing whatever was known before.
    ///
    /// Distinct from [`Control::Fingerprints`] because a sync answers a
    /// question the additive form cannot: *what is no longer ours*. An asset
    /// the server evicted has had its bytes deleted, so leaving its promise
    /// in place would send the next load to the network and store the result
    /// again — quietly undoing the eviction. Replacing the whole map is what
    /// makes revocation stick (FR-015).
    ReplaceFingerprints(Vec<(Uuid, Fingerprint)>),
    /// Whether there is room to store anything at all (FR-024).
    ///
    /// Set from each budget pass. `true` means even releasing everything
    /// permissible left too little room — the open world alone exceeds the
    /// limit, which FR-023 forbids evicting — so the cache keeps serving what
    /// it already holds and stops *adding*. Loads still succeed; they simply
    /// come from the network and are not filed.
    Storable(bool),
    /// The backing store finished opening: usable, or not on this browser.
    ///
    /// Decided off the main thread and applied here, because the resource is
    /// the only thing allowed to hold the answer.
    Readiness(bool),
    /// The user signed out (FR-016a).
    ///
    /// Everything this resource holds describes what the *departing* session
    /// was entitled to read: which scope's directory to open, which world's
    /// blobs are ours, which fingerprints are promised, which handles were
    /// issued. None of it survives the session that produced it, so this
    /// returns the resource to its pre-configuration state and the read path
    /// to plain network loads until somebody configures it again.
    ///
    /// This is *not* what makes the stored bytes unreadable — discarding the
    /// key is (see `forget_world_cache`). This only stops the engine from
    /// pointing at them.
    Forget,
}

static CONTROL_QUEUE: OnceLock<Mutex<Vec<Control>>> = OnceLock::new();
static DELIVERIES: OnceLock<Mutex<Vec<Delivery>>> = OnceLock::new();

fn control_queue() -> &'static Mutex<Vec<Control>> {
    CONTROL_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn deliveries() -> &'static Mutex<Vec<Delivery>> {
    DELIVERIES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Where this session's canvas-asset bytes came from, counted (FR-051).
///
/// # Why a tally exists at all when [`Origin`] was only ever logged
///
/// FR-051 asks the client to report the proportion served locally versus
/// fetched, the bytes avoided, and how much came from a peer rather than the
/// server. Every one of those is a count of deliveries by origin, and the
/// origin was already known at the one point every delivery passes through —
/// it was written to the debug log and then thrown away. A log is not a
/// diagnostics view: SC-017 requires these confirmable during an ordinary
/// session *without developer tooling*, and reading a console is exactly the
/// developer tooling it rules out.
///
/// # Counts, never content
///
/// Eight integers, and deliberately nothing else. No asset ids, no
/// fingerprints, no urls, no timings — the same restraint `peer_transfer_activity`
/// keeps, for the same reason: FR-052 says this information stays on the
/// user's machine, and the cheapest way to keep a promise about what is not
/// transmitted is to never assemble the thing that would be worth
/// transmitting.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct OriginTally {
    cache_items: u64,
    cache_bytes: u64,
    network_items: u64,
    network_bytes: u64,
    peer_items: u64,
    peer_bytes: u64,
    unverified_items: u64,
    unverified_bytes: u64,
    /// Bytes pulled ahead of demand and filed, never handed to a caller.
    ///
    /// Kept apart from the served buckets above because it answers a
    /// different half of FR-051. A prefetch is unambiguously *bytes
    /// transferred* — the wire does not care that nobody was waiting — but
    /// it is not an item *served*, so folding it into `network_items` would
    /// quietly ruin the served-versus-fetched proportion that is the panel's
    /// headline. Counting it nowhere is the worse error: SC-003 asks what a
    /// changed asset cost to bring down, and on a warm world the prefetch is
    /// usually the thing that brings it.
    prefetched_items: u64,
    prefetched_bytes: u64,
    prefetched_peer_items: u64,
    prefetched_peer_bytes: u64,
}

static ORIGIN_TALLY: OnceLock<Mutex<OriginTally>> = OnceLock::new();

fn origin_tally() -> &'static Mutex<OriginTally> {
    ORIGIN_TALLY.get_or_init(|| Mutex::new(OriginTally::default()))
}

/// Count one delivery. Called from the single point every delivery passes
/// through, so no origin can be added later and quietly go uncounted.
///
/// A poisoned lock costs the diagnostics one delivery and nothing else; there
/// is no branch anywhere that reads these numbers, so failing to record must
/// never be allowed to affect the image the user is waiting for.
fn record_delivery(origin: Origin, byte_len: u64) {
    let Ok(mut tally) = origin_tally().lock() else {
        return;
    };
    match origin {
        Origin::Cache => {
            tally.cache_items += 1;
            tally.cache_bytes += byte_len;
        }
        Origin::Network => {
            tally.network_items += 1;
            tally.network_bytes += byte_len;
        }
        Origin::Peer => {
            tally.peer_items += 1;
            tally.peer_bytes += byte_len;
        }
        Origin::NetworkUnverified => {
            tally.unverified_items += 1;
            tally.unverified_bytes += byte_len;
        }
    }
}

/// Count one asset brought in ahead of demand.
///
/// Separate from [`record_delivery`] because a prefetch never reaches a
/// caller: there is no `Origin` for it, only a source. `from_peer` mirrors
/// the same distinction the delivery path draws, so the panel can keep
/// saying how much came from a peer rather than the server.
fn record_prefetch(from_peer: bool, byte_len: u64) {
    let Ok(mut tally) = origin_tally().lock() else {
        return;
    };
    if from_peer {
        tally.prefetched_peer_items += 1;
        tally.prefetched_peer_bytes += byte_len;
    } else {
        tally.prefetched_items += 1;
        tally.prefetched_bytes += byte_len;
    }
}

/// Forget the session's figures.
///
/// Called on sign-out for the same reason every other piece of session state
/// is: the numbers describe what the *departing* session loaded, and leaving
/// them on screen for whoever signs in next would attribute one person's
/// activity to another — a small disclosure, but an unnecessary one.
fn reset_origin_tally() {
    if let Ok(mut tally) = origin_tally().lock() {
        *tally = OriginTally::default();
    }
}

/// The tally as the JSON object the diagnostics panel reads.
///
/// camelCase to match `sync_world_cache` and `peer_transfer_activity`, which
/// are the two other things the TypeScript side parses out of this module.
fn origin_tally_json() -> String {
    let tally = origin_tally()
        .lock()
        .map(|guard| *guard)
        .unwrap_or_default();
    serde_json::json!({
        "cacheItems": tally.cache_items,
        "cacheBytes": tally.cache_bytes,
        "networkItems": tally.network_items,
        "networkBytes": tally.network_bytes,
        "peerItems": tally.peer_items,
        "peerBytes": tally.peer_bytes,
        "unverifiedItems": tally.unverified_items,
        "unverifiedBytes": tally.unverified_bytes,
        "prefetchedItems": tally.prefetched_items,
        "prefetchedBytes": tally.prefetched_bytes,
        "prefetchedPeerItems": tally.prefetched_peer_items,
        "prefetchedPeerBytes": tally.prefetched_peer_bytes,
    })
    .to_string()
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
    // FR-073. The prefetcher runs in a spawned task with no access to the
    // ECS, so the world it is confined to is recorded here, at the one point
    // that always knows: every path that opens a world passes through this
    // function, `run_sync` included.
    #[cfg(target_arch = "wasm32")]
    wasm::note_open_world(world_id);
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
    // FR-070. A live update outranks speculation twice over: it is work the
    // user is waiting on, and it means the plan an in-flight prefetch is
    // draining no longer matches what the server would answer. Bumping the
    // epoch stops that prefetch at its next step; the following sync builds a
    // fresh queue from a fresh plan.
    #[cfg(target_arch = "wasm32")]
    wasm::note_live_update();
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
        apply_control(&mut cache, control);
    }
}

/// One instruction, applied to the resource.
///
/// Split out of the drain so the state machine is exercisable without a
/// `World` — the conditions that matter here are "does an evicted asset stop
/// being cacheable" and "does a different user reset everything", and those
/// deserve tests rather than a browser.
fn apply_control(cache: &mut CanvasAssetCache, control: Control) {
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
        Control::ReplaceFingerprints(entries) => {
            cache.fingerprints.clear();
            // Handles issued under a promise that no longer exists must
            // not be reused: the blob behind one may have just been
            // deleted, and the id would then be honoured forever off the
            // strength of a texture still resident in `Assets<Image>`.
            cache.issued.clear();
            cache.fingerprints.extend(entries);
        }
        Control::Storable(storable) => {
            cache.storable = storable;
        }
        Control::Readiness(ready) => {
            cache.readiness = if ready {
                Readiness::Ready
            } else {
                Readiness::Unavailable
            };
        }
        Control::Forget => {
            cache.scope = None;
            cache.world_id = None;
            cache.fingerprints.clear();
            cache.issued.clear();
            // Back to permissive with everything else: the previous session's
            // budget verdict was about that session's store, and carrying a
            // `false` across a sign-out would leave the next user unable to
            // cache anything until their own first budget pass.
            cache.storable = true;
            // Back to `Unconfigured` rather than `Unavailable`: the cache is
            // not broken, there is simply nobody signed in. A later
            // `Configure` — the same user signing back in, or a different one
            // — finds `scope: None`, so it reopens the backing store from
            // scratch and mints a fresh key instead of quietly reusing
            // handles the previous session opened.
            cache.readiness = Readiness::Unconfigured;
            reset_origin_tally();
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
#[path = "cached_assets/wasm.rs"]
mod wasm;

#[cfg(test)]
#[path = "cached_assets_tests.rs"]
mod tests;
