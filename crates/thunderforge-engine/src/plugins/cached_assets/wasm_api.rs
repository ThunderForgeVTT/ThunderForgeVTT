//! What the browser may call: one world's cache sync, peer transfer, peer
//! adjudication, and the offline outbox.
//!
//! Split from `wasm.rs` for file length; the handles it works on live there.

use super::*;

/// Bring the local cache into agreement with the server for one world,
/// then point the read path at the result.
///
/// This is the one entry point the web client calls on world open, and
/// the only one that needs to exist: identity, manifest, request, plan
/// application and the resulting promises are all decided here (R1).
/// TypeScript passes two ids and reads a summary; it never learns what
/// is cached, and never decides what to fetch or discard.
///
/// `user_id` is the authenticated user's uuid — the *scope is derived
/// from it here*, via `UserScope::for_user`, so no caller can file one
/// user's bytes under another's directory by passing the wrong string.
///
/// **Never rejects and never throws.** Every failure — a bad id, no
/// OPFS, no key, no network, a malformed plan — resolves to a summary
/// with `status: "degraded"` and leaves the client on exactly today's
/// behaviour: plain network loads. A cache problem must not be able to
/// stop a world from opening.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn sync_world_cache(world_id: String, user_id: String) -> String {
    run_sync(world_id, user_id).await.to_string()
}

/// Start peer-assisted distribution for one world (spec 028 T088–T091).
///
/// **Only ever called once TypeScript has asked `isPeerTransferEnabled()`**
/// (`apps/web/src/services/peerTransfer.ts`). The gate is there and not
/// here on purpose: what the setting prevents is the *connection*, since
/// that is when a direct peer link reveals an IP address, so it has to be
/// consulted before any of this runs rather than before bytes move. The
/// honest way to guarantee "disabled means no connection was ever made"
/// is for this function not to be reached at all.
///
/// `session_id` is a client-generated uuid minted per page load, and
/// `send_signal` is `(toSessionId, payload) => void` — TypeScript owns the
/// `graphql-ws` connection the signals ride (ADR-048), so the transport
/// stays there and the protocol stays here.
///
/// Returns whether it started. `false` is not an error: it means the ids
/// did not parse, and the client is on server-only transfer, which is a
/// supported way to run.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_peer_transfer(
    world_id: String,
    session_id: String,
    send_signal: js_sys::Function,
) -> bool {
    let Ok(world_uuid) = Uuid::parse_str(&world_id) else {
        return false;
    };
    peer::enable(world_uuid, session_id, send_signal);

    // What the serving half is allowed to read, and the only thing it can
    // read. `read_blob` decrypts and verifies against the blob's own
    // filename, so a `Some` here is content this client genuinely holds
    // and has checked — which is exactly the precondition T091 puts on
    // serving anything at all.
    peer::set_provider(std::rc::Rc::new(move |fingerprint: Fingerprint| {
        Box::pin(async move {
            let handles = HANDLES.with(|slot| slot.borrow().clone())?;
            handles
                .store
                .read_blob(world_uuid, &fingerprint, &handles.key)
                .await
                .ok()
                .flatten()
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<u8>>>>>
    }));
    true
}

/// Stop peer transfer and close every channel (FR-049, FR-050).
///
/// Called when the user turns the setting off, when the world closes, and
/// on unload. Idempotent.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn stop_peer_transfer() {
    peer::disable();
}

/// Offer a connection to one session from the world's roster.
///
/// The newcomer initiates, always: a client that has just joined queries
/// `peerSessions` and offers to each name it gets back. Nobody offers to a
/// newcomer, which makes offer glare structurally impossible instead of
/// something to resolve.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn offer_to_peer(session_id: String) {
    peer::connect_to(session_id).await;
}

/// Deliver one relayed signal. The server never interprets these, and
/// neither does anything between here and `RTCPeerConnection`.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn receive_peer_signal(from_session_id: String, payload: String) {
    peer::on_signal(from_session_id, payload).await;
}

/// Begin peer-adjudicated play while the server is unreachable
/// (spec 028 T096/T098/T100, FR-057 to FR-059).
///
/// **TypeScript decides that the server is gone**, from the heartbeat and
/// nothing else (`apps/web/src/engine/world/sync/heartbeat.ts`), for the
/// same reason the setting gate lives there: one liveness signal, asked in
/// one place. This side decides the rest — the roster is whoever has an
/// open channel at this moment, and play stops the instant any of them
/// goes.
///
/// `gm_user` is the user the **server** named as Game Master, learned
/// while still connected; it is the only authority in the exchange that a
/// peer did not supply. `on_applied` is `(changeJson) => void`, how an
/// adjudicated move reaches the scene.
///
/// Returns whether play started. `false` means plain offline, where the
/// outbox already handles everything correctly.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn begin_peer_adjudication(
    self_user: String,
    gm_user: String,
    on_applied: js_sys::Function,
) -> bool {
    let (Ok(self_user), Ok(gm_user)) = (Uuid::parse_str(&self_user), Uuid::parse_str(&gm_user))
    else {
        return false;
    };
    peer::begin_adjudication(self_user, gm_user, on_applied)
}

/// Whether peer-adjudicated play is running this instant.
///
/// The answer a player's client cannot work out for itself from counts:
/// it is only true once the Game Master's channel has identified itself
/// as the user the server named (FR-059).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn peer_adjudication_active() -> bool {
    peer::adjudication_active()
}

/// The server is reachable again. Play stops and everything adjudicated
/// is owed a submission (FR-062).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn peer_adjudication_server_returned() {
    peer::adjudication_server_returned();
}

/// Stop peer-adjudicated play: the world closed, or peer transfer was
/// turned off.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn end_peer_adjudication() {
    peer::adjudication_end();
}

/// Everything applied while server-isolated, as JSON, for the Game
/// Master's client to submit over its own authenticated session.
///
/// Provisional, all of it: the server re-authorizes every change and may
/// reject any of them, and its decision is final (FR-062).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn peer_adjudication_submissions() -> String {
    peer::adjudication_submissions()
}

/// Put one token movement to the table (T100, T101).
///
/// Position, rotation and scale, and there is no parameter for anything
/// else — creation, deletion and permission changes are not adjudicable
/// by peers under any circumstances (FR-060), which is enforced by the
/// shape of `TokenTransform` rather than by a check here.
///
/// `false` is "adjudicated play is not running; queue it in the outbox
/// instead", which is the caller's single fall-back.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn propose_token_transform(
    entity_id: String,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
) -> bool {
    let Ok(entity_id) = Uuid::parse_str(&entity_id) else {
        return false;
    };
    let mut transform = peer::TokenTransform::default();
    if let (Some(x), Some(y)) = (x, y) {
        transform = transform.with_position(x, y);
    }
    if let Some(rotation) = rotation {
        transform = transform.with_rotation(rotation);
    }
    if let Some(scale) = scale {
        transform = transform.with_scale(scale);
    }
    if transform.is_empty() {
        return false;
    }
    peer::adjudication_propose(entity_id, transform)
}

/// What the FR-049 indicator should show, as the JSON object
/// `reportPeerTransferActivity` takes.
///
/// Counters only — no peer identities, no addresses, no timings. The panel
/// exists to disclose that peer transfer is happening, not to describe who
/// is in the game (FR-052, FR-054).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn peer_transfer_activity() -> String {
    peer::activity().to_json()
}

/// Where this session's canvas-asset bytes came from, as the JSON object
/// the cache diagnostics panel reads (FR-051).
///
/// Counts and byte totals by origin, and nothing else — see
/// [`super::super::OriginTally`]. Zeroes are a truthful answer here, unlike in
/// `engine_stats`: a session that has loaded nothing really has loaded
/// nothing, and the panel needs to be able to say so.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn canvas_asset_origins() -> String {
    super::super::origin_tally_json()
}

/// Queue one edit made while disconnected (spec 028 US7, FR-037).
///
/// `command` is the emitted world-store command as JSON text, stored
/// verbatim and never parsed here — the server replays it through the
/// ordinary mutation path, which is what makes re-authorization at
/// reconnect automatic rather than a mechanism of its own.
///
/// **Await this before treating the edit as accepted.** The whole value
/// of an outbox is that it survives the tab closing, and a caller that
/// reports success without waiting for the write has reintroduced exactly
/// the loss it exists to prevent. The failure is reported rather than
/// swallowed for the same reason: this is the one place in the cache
/// where "it did not work" must reach the user, because unlike every
/// other failure here it cannot be recovered by fetching again.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn queue_offline_change(
    world_id: String,
    local_id: String,
    command: String,
    is_game_master: bool,
) -> String {
    let (Ok(world_uuid), Ok(local_uuid)) = (Uuid::parse_str(&world_id), Uuid::parse_str(&local_id))
    else {
        return serde_json::json!({ "queued": false, "reason": "bad-id" }).to_string();
    };

    let role = if is_game_master {
        thunderforge_cache_core::conflict::Role::GameMaster
    } else {
        thunderforge_cache_core::conflict::Role::Player
    };

    let Ok(outbox) = OutboxStore::open().await else {
        return serde_json::json!({ "queued": false, "reason": "no-store" }).to_string();
    };
    match outbox.append(world_uuid, local_uuid, &command, role).await {
        Ok(change) => serde_json::json!({
            "queued": true,
            "localId": change.local_id.to_string(),
            "seq": change.enqueued_seq,
        })
        .to_string(),
        Err(err) => {
            warn!(target: "cached_assets", "could not queue offline change: {err}");
            serde_json::json!({ "queued": false, "reason": "write-failed" }).to_string()
        }
    }
}

/// Everything queued for a world, in the order it must be replayed.
///
/// Returns a JSON array of `{localId, command}` — the shape
/// `reconcileQueuedChanges` takes — so the TypeScript side forwards it
/// without needing to understand a single command.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn read_queued_changes(world_id: String) -> String {
    let Ok(world_uuid) = Uuid::parse_str(&world_id) else {
        return "[]".to_string();
    };
    let Ok(outbox) = OutboxStore::open().await else {
        return "[]".to_string();
    };
    let Ok(changes) = outbox.for_world(world_uuid).await else {
        return "[]".to_string();
    };
    let wire: Vec<serde_json::Value> = changes
        .into_iter()
        .map(|change| {
            serde_json::json!({
                "localId": change.local_id.to_string(),
                // Parsed back to a value so it travels as JSON rather
                // than as a string containing JSON. A command we cannot
                // parse is still sent, as a string, and the server
                // rejects it as `INVALID` — which is an outcome, and
                // therefore not silent loss.
                "command": serde_json::from_str::<serde_json::Value>(&change.command)
                    .unwrap_or(serde_json::Value::String(change.command.clone())),
            })
        })
        .collect();
    serde_json::Value::Array(wire).to_string()
}

/// Drop the queued changes the server accounted for, keeping the rest.
///
/// `outcomes_json` is the mutation's reply. Anything it does not mention
/// stays queued (FR-041) and is answered for on the next reconnect —
/// re-sending an applied change is safe, because the server gives exactly
/// one outcome per submitted change, while dropping one is not
/// recoverable.
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn forget_reconciled_changes(outcomes_json: String) -> String {
    #[derive(serde::Deserialize)]
    struct WireOutcome {
        #[serde(rename = "localId")]
        local_id: String,
        applied: bool,
    }

    let Ok(wire) = serde_json::from_str::<Vec<WireOutcome>>(&outcomes_json) else {
        return serde_json::json!({ "remaining": -1, "reason": "bad-outcomes" }).to_string();
    };
    let outcomes: Vec<thunderforge_cache_core::queue::ReconcileOutcome> = wire
        .into_iter()
        .filter_map(|entry| {
            Some(thunderforge_cache_core::queue::ReconcileOutcome {
                local_id: Uuid::parse_str(&entry.local_id).ok()?,
                applied: entry.applied,
                reason: None,
            })
        })
        .collect();

    let Ok(outbox) = OutboxStore::open().await else {
        return serde_json::json!({ "remaining": -1, "reason": "no-store" }).to_string();
    };
    match outbox.forget_resolved(&outcomes).await {
        Ok(remaining) => serde_json::json!({ "remaining": remaining.len() }).to_string(),
        Err(err) => {
            warn!(target: "cached_assets", "could not drain the outbox: {err}");
            serde_json::json!({ "remaining": -1, "reason": "read-failed" }).to_string()
        }
    }
}
