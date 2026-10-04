//! Forgetting a signed-out user's cache, and the sync and prefetch work the
//! API beside it delegates to.
//!
//! Split from `wasm.rs` for file length; the handles it works on live there.

use super::*;

/// Discard the signed-out user's session key, then reclaim their disk
/// space in the background (FR-016a, FR-016b).
///
/// The two halves are deliberately unequal. Discarding the key is one
/// IndexedDB delete, is awaited, and is what actually makes this user's
/// stored blobs inert — immediately, whether or not a single byte is ever
/// deleted. Reclaiming the bytes is spawned and never awaited, because a
/// multi-gigabyte OPFS directory cannot be wiped before the tab closes and
/// waiting for it would make sign-out feel broken.
///
/// **Reclamation can never restore readability.** It only deletes: index
/// rows and blob files. There is no path through it that writes a key, and
/// it runs strictly after `crypto::forget` has already resolved, so a
/// reclamation that fails, is interrupted by the tab closing, or never
/// runs at all leaves ciphertext whose key is gone. That ordering is the
/// whole reason this feature encrypts rather than merely deletes, so do
/// not move the `forget` below the spawn.
///
/// **Never rejects and never throws.** Sign-out is the caller's business
/// and a cache problem has no standing to interfere with it, so every
/// failure — a malformed id, no IndexedDB, no OPFS — resolves to a summary
/// naming the reason and nothing more.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn forget_world_cache(user_id: String) -> String {
    run_forget(user_id).await.to_string()
}

pub(super) async fn run_forget(user_id: String) -> serde_json::Value {
    // Before anything that can fail: stop this tab reading the cache.
    //
    // The stored key is not the only copy — `Handles` holds a live
    // `CryptoKey` that would keep decrypting happily long after the
    // IndexedDB record is gone. Dropping it here is as much a part of
    // FR-016a as the delete below, and doing it first means even the
    // bad-uuid path leaves no usable key behind in this tab.
    forget_in_memory();

    let Ok(user_uuid) = Uuid::parse_str(&user_id) else {
        // Nothing scope-specific can be done, but the handles are already
        // gone and the read path is already switched off.
        return serde_json::json!({
            "status": "degraded",
            "reason": "user id is not a uuid",
            "keyDiscarded": false,
        });
    };
    let scope = UserScope::for_user(user_uuid);

    // FR-016a. Awaited, because the answer to "is that user's cache inert
    // yet" must be true by the time this resolves.
    let discarded = match crypto::forget(&scope).await {
        Ok(()) => true,
        Err(err) => {
            warn!(target: "cached_assets", "could not discard session key: {err}");
            false
        }
    };

    // FR-016b. Spawned either way — when the key survived, deleting the
    // bytes is the only remaining defence, so a failed `forget` makes
    // reclamation more important, not less.
    spawn_local(reclaim(scope));

    serde_json::json!({
        "status": if discarded { "forgotten" } else { "degraded" },
        "keyDiscarded": discarded,
        "reclaiming": true,
    })
}

/// Give back the disk the now-inert content occupies. Best-effort, in the
/// background, and safe to fail at any point (FR-016b).
///
/// Nothing here opens a session key. `OpfsStore::open` needs none, and
/// `crypto::load_or_create` is deliberately not called: minting a fresh
/// key mid-reclamation would put a usable key back in the store for a
/// scope we just forgot.
pub(super) async fn reclaim(scope: UserScope) {
    // Index rows first. They are only a map of what we believe we hold;
    // losing them costs a cold cache, never readability. If this fails the
    // rows stay, point at blobs that are about to disappear, and the
    // existing missing-blob repair (FR-019) plus the demand path heal it.
    match IndexStore::open().await {
        Ok(index) => {
            if let Err(err) = index.clear().await {
                debug!(target: "cached_assets", "index reclamation failed: {err}");
            }
        }
        Err(err) => debug!(target: "cached_assets", "index reclamation skipped: {err}"),
    }

    // Then the bytes. One directory removal for the whole scope; if it is
    // interrupted, what remains on disk is ciphertext without a key.
    match OpfsStore::open(scope).await {
        Ok(store) => match store.remove_scope().await {
            Ok(()) => info!(target: "cached_assets", "reclaimed the signed-out cache"),
            Err(err) => {
                debug!(target: "cached_assets", "blob reclamation failed: {err}");
            }
        },
        Err(err) => debug!(target: "cached_assets", "blob reclamation skipped: {err}"),
    }

    // The outbox is deliberately NOT cleared here (spec 028 T081,
    // FR-041).
    //
    // Everything above is *cache*: a copy of content the server still
    // holds, worth nothing once the key is gone. The outbox is the
    // opposite — it is the only copy of work the user did and the server
    // has never seen. Reclaiming it would be the one deletion in this
    // whole path that destroys something rather than freeing something.
    //
    // It also survives key loss on its own terms: entries are plaintext
    // commands, not ciphertext, so discarding the session key leaves them
    // as readable as they ever were. They simply cannot be *submitted*
    // until somebody signs in again, which is a reason to keep them and
    // report them, not a reason to drop them.
    //
    // Stated here rather than left to the absence of a call, because the
    // next person adding a store to `ALL_STORES` and a matching `clear`
    // to this function would be doing an obviously reasonable thing.
}

pub(super) fn degraded(reason: &str) -> serde_json::Value {
    debug!(target: "cached_assets", "world cache sync degraded: {reason}");
    serde_json::json!({ "status": "degraded", "reason": reason })
}

pub(super) async fn run_sync(world_id: String, user_id: String) -> serde_json::Value {
    let Ok(world_uuid) = Uuid::parse_str(&world_id) else {
        return degraded("world id is not a uuid");
    };
    let Ok(user_uuid) = Uuid::parse_str(&user_id) else {
        return degraded("user id is not a uuid");
    };
    let scope = UserScope::for_user(user_uuid);

    // Identity first, before anything can fail. Even a sync that
    // degrades leaves the read path pointed at the right directory, so
    // an offline reopen can still serve what is already on disk.
    super::super::configure_canvas_asset_cache(scope.as_str(), &world_id);

    let Some(handles) = ensure_handles(scope).await else {
        return degraded("cache unavailable on this browser");
    };
    let Some(index) = borrow_index(&handles).await else {
        return degraded("cache index unavailable");
    };

    let manifest = sync::manifest_for_open_world(&index, world_uuid).await;
    let held = manifest.len();
    let body = sync::sync_request_body(&manifest);

    let outcome = match post_sync(&body).await {
        Ok(text) => sync::parse_sync_plan(&text),
        Err(err) => Err(thunderforge_cache_browser::sync::SyncError::Transport(err)),
    };

    let outcome = match outcome {
        Ok(outcome) => outcome,
        // FR-014/FR-015. The server answered, and the answer was "you may
        // not have this world". There is no plan and therefore no `evict`
        // list, so the per-item path that normally enforces revocation
        // has nothing to work with — yet this is the *strongest* form of
        // revocation there is. Whole-world refusal gets its own discard.
        //
        // Only this variant. `Transport` (nothing answered), `Server`
        // (answered, but not about entitlement) and `Malformed` all fall
        // through to the branch below and keep the cache, because "my
        // wifi dropped" must never cost a user their stored world. See
        // `sync::is_authorization_refusal` for how the two are told
        // apart.
        Err(thunderforge_cache_browser::sync::SyncError::Forbidden(reason)) => {
            // FR-050, before the discard rather than after it. Serving is
            // stopped by the same answer that revokes the content, and it
            // is stopped first: a discard is several awaits long, and a
            // peer asking during them would otherwise be served bytes
            // this client has just been told it may not have.
            peer::membership_lost();
            let discarded = sync::discard_world(&handles.store, &index, world_uuid).await;
            return_index(&handles, index);

            // Whatever the discard managed, the read path stops pointing
            // at this world *now*: an empty promise set means no load can
            // consult the cache, so any bytes a failed discard left
            // behind are unreachable rather than merely stale. Pushed
            // unconditionally rather than derived from the index, so an
            // index we could not read cannot leave a promise standing.
            if let Ok(mut queue) = control_queue().lock() {
                queue.push(Control::ReplaceFingerprints(Vec::new()));
            }

            warn!(
                target: "cached_assets",
                "world cache discarded: access to {world_id} was refused: {reason}",
            );
            // Reported, never raised: a discard that could not finish is
            // for the sign-out reclamation (FR-016b) and the FR-019
            // repair pass to mop up, and this call must not throw.
            return serde_json::json!({
                "status": "revoked",
                "worldId": world_id,
                "held": held,
                "discarded": discarded.rows,
                "indexCleared": discarded.index_cleared,
                "blobsCleared": discarded.blobs_cleared,
                "complete": discarded.complete(),
            });
        }
        Err(err) => {
            // The server is unreachable or disagreed with us. What we
            // hold is still what we last verified, so publish it: an
            // offline reopen reads from disk rather than failing to
            // read at all.
            publish_fingerprints(&index, world_uuid, &SyncPlan::default()).await;
            return_index(&handles, index);
            warn!(target: "cached_assets", "world cache sync failed: {err}");
            let mut summary = degraded(&err.to_string());
            summary["held"] = serde_json::json!(held);
            return summary;
        }
    };

    let applied = sync::apply_plan(&handles.store, &index, world_uuid, &outcome.plan).await;

    // FR-019, after the plan rather than before it. The index and the
    // disk drift for ordinary reasons — a blob is written before its row,
    // a row removed before its blob, a tab closed between two awaits —
    // and `apply_plan` has just performed the largest batch of both
    // operations this world will see. Reconciling here therefore repairs
    // this pass's own interruptions as well as any left by a previous
    // session, and it does so while the caller is already waiting on a
    // sync, so it costs no additional user-visible moment.
    //
    // It is one directory listing and one index range read per open. An
    // orphan is rare; the cost when there is nothing to do is the two
    // reads and no writes at all.
    let repaired = sync::repair_world(&handles.store, &index, world_uuid).await;
    if repaired.rows_dropped > 0 || repaired.blobs_reclaimed > 0 || repaired.failed > 0 {
        info!(
            target: "cached_assets",
            "repaired world cache: {} row(s) dropped, {} blob(s) reclaimed, \
             {} unfinished kept, {} failure(s)",
            repaired.rows_dropped,
            repaired.blobs_reclaimed,
            repaired.unfinished_kept,
            repaired.failed,
        );
    }

    // FR-022/FR-023, after the repair and before the prefetch — the only
    // point in the pass where the index is both accurate and not yet
    // about to grow. Running it before `repair_world` would plan against
    // rows that are known lies, and running it after the prefetch would
    // mean admitting bytes first and asking about the budget afterwards.
    //
    // `incoming` is what this open intends to add, so the limit is
    // checked against where the store is *going*, not where it has been.
    let incoming: u64 = outcome.plan.fetch.iter().map(|item| item.byte_size).sum();
    let budget = sync::enforce_budget(&handles.store, &index, world_uuid, incoming).await;
    if budget.evicted > 0 || budget.failed > 0 || budget.insufficient {
        info!(
            target: "cached_assets",
            "budget pass: {} row(s) evicted, {} blob(s) removed, {} failure(s), \
             {}/{} bytes in use{}",
            budget.evicted,
            budget.blobs_removed,
            budget.failed,
            budget.in_use_bytes,
            budget.limit_bytes,
            if budget.insufficient {
                " (insufficient: fetching without storing)"
            } else {
                ""
            },
        );
    }
    // FR-024. A store with no room keeps serving what it holds and stops
    // adding; loads still succeed, from the network, unfiled. Published
    // even when nothing was evicted, because the *recovery* direction
    // matters as much: a pass that finds room again must turn storing
    // back on.
    set_storable(!budget.insufficient);
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::Storable(!budget.insufficient));
    }
    if budget.unknown_quota {
        debug!(
            target: "cached_assets",
            "no storage estimate available; leaving the store as it is",
        );
    }

    publish_fingerprints(&index, world_uuid, &outcome.plan).await;

    // T089/T091, and the reason both live here rather than in the peer
    // module: this is the only place the server's answer exists. What may
    // be *asked for* is `plan.fetch` and nothing else; what may be
    // *served* is what the index says is on disk, after the eviction,
    // repair and budget passes above have had their say. Both are
    // replaced wholesale on every sync, so a fingerprint the server has
    // stopped listing stops being requestable and a blob just evicted
    // stops being offered, without either needing its own invalidation.
    peer::set_plan(&outcome.plan);
    if let Ok(entries) = index.for_world(world_uuid).await {
        peer::set_held(entries.into_iter().map(|(_, entry)| entry.fingerprint));
    }

    return_index(&handles, index);

    // T116/FR-072. The queue is built from *this* caller's plan and
    // nothing else — `PrefetchQueue::from_plan` is its only constructor,
    // so the permission boundary the server drew around `outcome.plan` is
    // the same one the prefetch runs inside. Stamping it with the world
    // and the epoch is what lets it stop when either moves (FR-070,
    // FR-073).
    let epoch = begin_plan();
    let queue = PrefetchQueue::from_plan(world_uuid, epoch, &outcome.plan);
    let prefetching = queue.remaining();

    // Deliberately not awaited: warming the cache is the next visit's
    // benefit, and making this visit wait for it would trade the thing
    // the user is watching for a thing they are not.
    // Post-eviction occupancy, not `budget.in_use_bytes`.
    //
    // `in_use_bytes` is what the index held *before* this pass, and the
    // prefetch gate reads its argument as what the store holds *now*. So a
    // pass that evicted a world to make room told the prefetch the room
    // was still occupied, `admit_speculative` refused the first item, and
    // the queue stopped with `NoRoom` — the open world's own art never got
    // written, by the very pass performed to fit it.
    let occupied = budget.in_use_bytes.saturating_sub(budget.freed_bytes);
    spawn_local(prefetch(handles, queue, occupied, budget.limit_bytes));

    serde_json::json!({
        "status": "synced",
        "worldId": world_id,
        "held": held,
        "fetch": outcome.plan.fetch.len(),
        "evicted": applied.evicted,
        "blobsRemoved": applied.blobs_removed,
        "evictFailures": applied.failed,
        "rowsRepaired": repaired.rows_dropped,
        "blobsReclaimed": repaired.blobs_reclaimed,
        "unfinishedKept": repaired.unfinished_kept,
        "repairFailures": repaired.failed,
        "budgetLimit": budget.limit_bytes,
        "budgetInUse": budget.in_use_bytes,
        "budgetEvicted": budget.evicted,
        "budgetBlobsRemoved": budget.blobs_removed,
        "budgetFailures": budget.failed,
        "budgetInsufficient": budget.insufficient,
        "budgetQuotaUnknown": budget.unknown_quota,
        "prefetching": prefetching,
        "canonicalVersion": outcome.canonical_version,
        // Reachability, not holdings: true iff someone else is live in
        // this world. Reported for diagnostics and never used as a gate —
        // every peer path already lands on the server when no channel is
        // open, and this would be stale the moment somebody joins.
        "peerAvailable": outcome.peer_available,
    })
}

/// The double-submit CSRF token the server requires on every
/// state-changing method for a session (`auth_middleware.rs`,
/// `require_csrf_for_session`).
///
/// GraphQL is served over POST, so a query is a "state-changing method"
/// as far as that middleware is concerned and a manifest sent without
/// this header comes back 403 — which is how this was found. The cookie
/// is deliberately not `HttpOnly` precisely so the client can echo it,
/// which is the whole of the double-submit pattern; nothing secret is
/// being read out of the page here.
pub(super) const CSRF_COOKIE: &str = "csrf_token";
pub(super) const CSRF_HEADER: &str = "x-csrf-token";

/// Read one cookie by name, via `document.cookie`.
///
/// Reached through `js_sys::Reflect` rather than `web_sys::window()`
/// because this crate does not depend on `web-sys` directly, and because
/// the engine is not entitled to assume it is running on a `Window` —
/// `document` being absent (a worker) is simply "no token", handled by
/// the `Option` like any other miss.
pub(super) fn cookie_value(name: &str) -> Option<String> {
    let document = js_sys::Reflect::get(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from_str("document"),
    )
    .ok()?;
    let cookies =
        js_sys::Reflect::get(&document, &wasm_bindgen::JsValue::from_str("cookie")).ok()?;
    let cookies = cookies.as_string()?;
    cookies.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key.trim() == name).then(|| value.trim().to_owned())
    })
}

/// POST the manifest. Same-origin, so credentials ride along by default.
pub(super) async fn post_sync(body: &str) -> Result<String, String> {
    let mut builder =
        gloo_net::http::Request::post(GRAPHQL_ENDPOINT).header("Content-Type", "application/json");
    if let Some(token) = cookie_value(CSRF_COOKIE) {
        builder = builder.header(CSRF_HEADER, &token);
    }
    let request = builder
        .body(body.to_owned())
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    response.text().await.map_err(|err| err.to_string())
}

/// Hand the read path the complete set of promises for this world.
///
/// Held rows first (the contract's silence means they are current),
/// then the plan's fetches on top (a superseded asset's new fingerprint
/// must win over the one we hold). Replaced wholesale rather than
/// merged, so an evicted asset loses its promise and cannot be quietly
/// re-cached by the next load.
pub(super) async fn publish_fingerprints(index: &IndexStore, world_id: Uuid, plan: &SyncPlan) {
    let mut entries = sync::canvas_fingerprints(index, world_id).await;
    for item in &plan.fetch {
        if let ItemId::CanvasAsset(asset_id) = item.id {
            entries.retain(|(id, _)| *id != asset_id);
            entries.push((asset_id, item.fingerprint));
        }
    }
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::ReplaceFingerprints(entries));
    }
}

/// How long to stand aside when the user is loading something.
///
/// Long enough that a burst of demand fetches is not re-polled dozens of
/// times, short enough that a quiet moment is used rather than slept
/// through.
pub(super) const PREFETCH_YIELD_MS: i32 = 250;

/// A breath between speculative fetches, so consecutive items cannot
/// occupy the connection back to back. This is the mechanism behind
/// SC-024: the active scene's requests interleave rather than queueing
/// behind a run of prefetches.
pub(super) const PREFETCH_PACE_MS: i32 = 50;

/// How many consecutive yields before giving up on this visit.
///
/// A tab that is loading something continuously for two minutes is not
/// going to hand the prefetcher a quiet moment, and a task that polls
/// forever is a leak with a timer attached. Stopping costs nothing: the
/// next sync builds a fresh queue, and everything unfetched is still
/// reachable on demand.
pub(super) const PREFETCH_MAX_YIELDS: u32 = 480;

/// Fetch planned canvas assets ahead of demand (FR-069 – FR-073).
///
/// Every decision here belongs to [`PrefetchQueue`], which is pure and
/// unit-tested natively; this function does the I/O it is told to and
/// reports back what actually landed. What is left is worth naming:
///
/// - **Sequential, and paced.** A burst of parallel requests for a whole
///   world would contend with the scene load this is supposed to be
///   invisible to (FR-070, SC-024).
/// - **Nothing verified by hand.** `record_fetched` writes through
///   `OpfsStore::write_blob`, which is where the fingerprint check
///   happens, so bytes that are not what the server promised are never
///   stored and never counted.
/// - **No Service Worker, no push, no background sync** (FR-073). This is
///   an ordinary task in the open tab; it cannot outlive the page,
///   because there is nothing here to outlive it with.
pub(super) async fn prefetch(
    handles: Rc<Handles>,
    mut queue: PrefetchQueue,
    in_use: u64,
    limit: u64,
) {
    let world_id = queue.world_id();
    // The store's occupancy *after* the budget pass, carried forward as
    // this task stores things. Deliberately not re-read from the index
    // each step: that is an IndexedDB scan per item, and the figure would
    // still be a moment stale.
    //
    // This comment used to say the figure errs high because the eviction
    // pass may have freed bytes since, and that erring high is the safe
    // direction for a check whose purpose is to stop early. That is
    // exactly backwards after an eviction: erring high suppresses the
    // write the eviction was performed to make room for. The caller now
    // subtracts what the pass freed, so this starts from what the store
    // actually holds (FR-071).
    let mut in_use = in_use;
    let mut yields: u32 = 0;

    loop {
        match queue.step(&pressure(world_id, in_use, limit)) {
            Step::Fetch(item) => {
                yields = 0;
                let stored = fetch_one(&handles, world_id, item).await;
                queue.record_stored(stored);
                in_use = in_use.saturating_add(stored);
                yield_for(PREFETCH_PACE_MS).await;
            }
            Step::Yield => {
                yields += 1;
                if yields > PREFETCH_MAX_YIELDS {
                    debug!(
                        target: "cached_assets",
                        "prefetch stood aside for the whole visit; leaving the rest to demand",
                    );
                    return;
                }
                yield_for(PREFETCH_YIELD_MS).await;
            }
            Step::Stop(reason) => {
                debug!(target: "cached_assets", "prefetch finished: {reason:?}");
                return;
            }
        }
    }
}

/// Fetch and store one speculative item. Returns the bytes stored, which
/// is zero for anything already held, unreachable, or refused — none of
/// which is a failure worth surfacing, and all of which must cost the
/// visit allowance nothing.
pub(super) async fn fetch_one(handles: &Rc<Handles>, world_id: Uuid, item: PrefetchItem) -> u64 {
    // Already on disk under this exact fingerprint — deduplicated
    // content another item already brought in.
    if handles
        .store
        .has_blob(world_id, &item.fingerprint)
        .await
        .unwrap_or(false)
    {
        return 0;
    }

    // FR-044. The prefetch is where peer transfer pays best: several
    // clients open the same scene within seconds of each other and
    // want the identical map. `None` is the server, as everywhere.
    let url = format!("{ASSET_URL_PREFIX}/{}.webp", item.asset_id);
    let from_peer_bytes = peer::try_fetch(item.fingerprint).await;
    let from_peer = from_peer_bytes.is_some();
    let bytes = match from_peer_bytes {
        Some(bytes) => bytes,
        None => match fetch(&url).await {
            Some(bytes) => bytes,
            None => return 0,
        },
    };
    let Some(mut index) = borrow_index(handles).await else {
        return 0;
    };
    let stored = sync::record_fetched(
        &handles.store,
        &mut index,
        &handles.key,
        world_id,
        ItemId::CanvasAsset(item.asset_id),
        &item.fingerprint,
        &bytes,
    )
    .await;
    return_index(handles, index);

    match stored {
        Ok(()) => {
            peer::note_stored(item.fingerprint);
            // Counted only once the bytes are actually filed, so the
            // figure the panel shows is what the disk gained rather than
            // what the wire carried into a failed write.
            super::super::record_prefetch(from_peer, bytes.len() as u64);
            bytes.len() as u64
        }
        Err(err) => {
            warn!(target: "cached_assets", "could not prefetch {url}: {err}");
            0
        }
    }
}

pub(super) fn mark_ready() {
    push_readiness(true);
}

pub(super) fn mark_unavailable() {
    push_readiness(false);
}

/// Readiness is decided off the main thread but owned by the resource,
/// so it travels back the same way everything else does.
pub(super) fn push_readiness(ready: bool) {
    if let Ok(mut queue) = control_queue().lock() {
        queue.push(Control::Readiness(ready));
    }
}
