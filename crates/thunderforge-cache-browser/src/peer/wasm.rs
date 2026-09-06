//! The `RTCPeerConnection`/`RTCDataChannel` glue, and nothing else.
//!
//! Every rule this module has is above; this half only moves frames. That
//! split is why "verify before storing" is checkable under `cargo test` on a
//! machine with no browser on it, and it is the same split the rest of the
//! crate uses.
//!
//! # No STUN, no TURN
//!
//! Deliberately. Every participant reaches the same server over the same
//! network path already, and host ICE candidates are enough for the cases
//! this feature exists for — several players in one household, or on one
//! office network, pulling the same map. A STUN server would be a third
//! party learning who is playing with whom, for a marginal increase in the
//! number of peer pairs that connect, and FR-052/FR-054 rule out paying that
//! price. A pair that cannot connect falls back to the server, like every
//! other failure here.

#[path = "wasm_transport.rs"]
mod transport;
pub use transport::*;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use js_sys::{Function, Reflect, Uint8Array};
use thunderforge_cache_core::delta::SyncPlan;
use thunderforge_cache_core::{Fingerprint, fingerprint};
use uuid::Uuid;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::{
    AdjudicatedChange, Adjudication, AdjudicationMessage, AdjudicationStep, DeclineReason,
    DownloadStep, Fallback, PeerActivity, PeerDownload, PeerMessage, PeerServer, PeerTrust,
    PlanScope, STALL_MS, ServeDecision, TokenTransform, serve_frames,
};

/// The data-channel label. Both ends open with the same one so a channel
/// belonging to some future feature is never mistaken for this one.
const CHANNEL_LABEL: &str = "thunderforge-cache";

/// How often an in-flight download is asked whether it has given up.
///
/// The pure state machine owns the actual deadlines ([`STALL_MS`],
/// [`super::DEADLINE_MS`]); this is only how often it is consulted, and
/// it is a fraction of the shorter one so the answer is never much late.
const TICK_MS: i32 = (STALL_MS / 4) as i32;

/// Reads one locally-held blob, verified, or `None`.
///
/// Injected rather than reached for, because the OPFS handle and the
/// session key belong to the engine, not to this crate's peer module. It
/// also means the serving path physically cannot read anything the
/// engine has not agreed to expose.
pub type BlobProvider = Rc<dyn Fn(Fingerprint) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>>>>>;

thread_local! {
    static FABRIC: RefCell<Option<Fabric>> = const { RefCell::new(None) };
}

struct Fabric {
    world_id: Uuid,
    session_id: String,
    /// `(toSessionId, payload) => void`, supplied by TypeScript, which
    /// owns the `graphql-ws` connection the signals ride (ADR-048).
    send_signal: Function,
    scope: PlanScope,
    server: PeerServer,
    trust: PeerTrust,
    activity: PeerActivity,
    provider: Option<BlobProvider>,
    links: BTreeMap<String, Rc<PeerLink>>,
    /// Peer-adjudicated play, while the three conditions hold (T098).
    /// `None` is the ordinary state: the server is reachable, or it is
    /// not and this client is simply offline.
    adjudication: Option<Adjudication>,
    /// `(changeJson) => void`, supplied by the engine: how an adjudicated
    /// move reaches the scene. Injected for the same reason
    /// [`BlobProvider`] is — the world store belongs to the engine, and
    /// this crate must not be able to write to it on its own.
    on_applied: Option<Function>,
}

/// One peer: its connection, its channel, and at most one download.
///
/// One download at a time per peer on purpose. Two in flight would need
/// the frames interleaved and demultiplexed by fingerprint, which is a
/// reassembly problem whose inputs an untrusted party chooses. Serial is
/// slower in the rare case and impossible to confuse in every case.
struct PeerLink {
    session: String,
    connection: web_sys::RtcPeerConnection,
    channel: RefCell<Option<web_sys::RtcDataChannel>>,
    download: RefCell<Option<PeerDownload>>,
    /// The `resolve` of the promise `try_fetch` is awaiting.
    waiter: RefCell<Option<Function>>,
    outcome: RefCell<Option<Vec<u8>>>,
    ticker: RefCell<Option<i32>>,
    /// Closures the platform holds pointers into. Dropping one while the
    /// browser still has it registered is a call into freed memory, so
    /// they live as long as the link does.
    retained: RefCell<Vec<Closure<dyn FnMut(JsValue)>>>,
}

fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

fn global_fn(name: &str) -> Option<Function> {
    Reflect::get(&js_sys::global(), &JsValue::from_str(name))
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

/// Start peer transfer for one world.
///
/// Called only after TypeScript has asked `isPeerTransferEnabled()`. The
/// check belongs there and not here because what the setting prevents is
/// the *connection* — the IP exposure happens when a channel opens, not
/// when bytes move — so the gate has to sit before any of this runs, and
/// the honest way to guarantee that is for this to never be called.
pub fn enable(world_id: Uuid, session_id: String, send_signal: Function) {
    FABRIC.with(|slot| {
        *slot.borrow_mut() = Some(Fabric {
            world_id,
            session_id,
            send_signal,
            scope: PlanScope::none(world_id),
            server: PeerServer::new(world_id),
            trust: PeerTrust::new(),
            activity: PeerActivity::default(),
            provider: None,
            links: BTreeMap::new(),
            adjudication: None,
            on_applied: None,
        });
    });
}

/// Stop, close every channel, and forget everything.
///
/// The user turned it off, the world closed, or the page is going away.
/// Nothing survives (FR-050): connections are closed rather than left to
/// the garbage collector, because a channel that outlives the world is a
/// channel still carrying this client's address to someone.
pub fn disable() {
    let fabric = FABRIC.with(|slot| slot.borrow_mut().take());
    if let Some(fabric) = fabric {
        for link in fabric.links.values() {
            link.shut_down();
        }
    }
}

/// Whether peer transfer is running at all.
pub fn is_active() -> bool {
    FABRIC.with(|slot| slot.borrow().is_some())
}

/// Open channels right now.
pub fn peer_count() -> usize {
    FABRIC.with(|slot| {
        slot.borrow().as_ref().map_or(0, |f| {
            f.links.values().filter(|link| link.is_open()).count()
        })
    })
}

/// Replace the entitlement scope with the server's latest plan (T089).
///
/// Wholesale, never merged. A scope that accumulated across syncs would
/// let a client go on asking for content the server has stopped listing,
/// which is the same revocation hole `apply_plan` exists to close on the
/// storage side.
pub fn set_plan(plan: &SyncPlan) {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.scope = PlanScope::from_plan(fabric.world_id, plan);
        }
    });
}

/// Declare what this client holds and has verified (T091).
pub fn set_held(fingerprints: impl IntoIterator<Item = Fingerprint>) {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.server.holds_only(fingerprints);
        }
    });
}

/// One more fingerprint is on disk, verified, and servable.
pub fn note_stored(fingerprint: Fingerprint) {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.server.holds(fingerprint);
        }
    });
}

/// Membership in this world ended: stop serving now, mid-transfer if need
/// be (FR-050).
pub fn membership_lost() {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.server.membership_lost();
        }
    });
}

/// Supply the reader the serving path uses.
pub fn set_provider(provider: BlobProvider) {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.provider = Some(provider);
        }
    });
}

/// What the FR-049 indicator should show.
pub fn activity() -> PeerActivity {
    let mut activity = FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(PeerActivity::default(), |f| f.activity)
    });
    activity.connected_peers = peer_count();
    activity
}

// -----------------------------------------------------------------
// Peer adjudication (T098, T100, T101)
// -----------------------------------------------------------------

/// Begin peer-adjudicated play (FR-057).
///
/// **The caller decides that the server is unreachable, and it decides it
/// from the heartbeat** (`engine/world/sync/heartbeat.ts`) — the one
/// liveness signal this feature has. Nothing in this crate forms a second
/// opinion about connectivity, because two opinions is how a client ends
/// up queueing edits during an idle moment while reporting a connection it
/// does not have.
///
/// The participant roster is the peers with an open channel *right now*,
/// fixed at this moment and never widened. Losing any of them ends play
/// (FR-058); a session that arrives afterwards cannot join, because the
/// order was agreed at the start and a newcomer has not seen it.
///
/// `gm_user` is the user the **server** named as Game Master, learned
/// while still connected. It is the only authority in the whole exchange
/// that a peer did not supply.
///
/// Returns whether play started. `false` means plain offline, where the
/// outbox already handles everything correctly.
pub fn begin_adjudication(self_user: Uuid, gm_user: Uuid, on_applied: Function) -> bool {
    let hello = FABRIC.with(|slot| {
        let mut borrowed = slot.borrow_mut();
        let fabric = borrowed.as_mut()?;
        let roster: Vec<String> = fabric
            .links
            .values()
            .filter(|link| link.is_open())
            .map(|link| link.session.clone())
            .collect();
        let adjudication =
            Adjudication::begin(fabric.session_id.clone(), self_user, gm_user, roster)?;
        let hello = adjudication.hello();
        fabric.adjudication = Some(adjudication);
        fabric.on_applied = Some(on_applied);
        Some(hello)
    });

    // Say who this client is. Nobody else can tell which channel belongs
    // to the Game Master until the claim arrives, and a player's client
    // does not adjudicate until it has.
    match hello {
        Some(hello) => {
            broadcast(&[hello]);
            true
        }
        None => false,
    }
}

/// Whether peer-adjudicated play is running this instant.
pub fn adjudication_active() -> bool {
    FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|fabric| fabric.adjudication.as_ref())
            .is_some_and(Adjudication::is_adjudicating)
    })
}

/// The server is reachable again: stop, and keep what was applied for
/// submission (FR-062).
pub fn adjudication_server_returned() {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut()
            && let Some(adjudication) = fabric.adjudication.as_mut()
        {
            adjudication.server_returned();
        }
    });
}

/// Stop peer-adjudicated play: the world closed, or the user turned peer
/// transfer off.
pub fn adjudication_end() {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut()
            && let Some(adjudication) = fabric.adjudication.as_mut()
        {
            adjudication.end();
        }
    });
}

/// Everything applied while server-isolated, as the JSON the reconcile
/// mutation takes. Empty when there is nothing owing.
pub fn adjudication_submissions() -> String {
    FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|fabric| fabric.adjudication.as_ref())
            .map_or_else(|| "[]".to_string(), Adjudication::submissions_json)
    })
}

/// Propose one token movement (T100, T101).
///
/// `false` is "adjudicated play is not running, queue it in the outbox
/// instead" — the caller has one fall-back and does not need to know
/// which of the three conditions failed.
pub fn adjudication_propose(entity_id: Uuid, transform: TokenTransform) -> bool {
    let step = FABRIC.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .and_then(|fabric| fabric.adjudication.as_mut())
            .map(|adjudication| adjudication.propose(entity_id, transform))
    });
    match step {
        Some(step) => handle_adjudication(step),
        None => false,
    }
}

/// Act on one step: broadcast what it says to broadcast, apply what it
/// says to apply. Returns whether anything happened.
fn handle_adjudication(step: AdjudicationStep) -> bool {
    match step {
        AdjudicationStep::Broadcast { frames, applied } => {
            broadcast(&frames);
            if let Some(change) = applied {
                deliver_applied(&change);
            }
            true
        }
        AdjudicationStep::Applied(change) => {
            deliver_applied(&change);
            true
        }
        // A refusal is not an error path and has nothing to say to a
        // user: the change goes to the outbox exactly as it would have
        // without any of this.
        AdjudicationStep::Ignore | AdjudicationStep::Refused(_) => false,
    }
}

fn broadcast(frames: &[Vec<u8>]) {
    let links: Vec<Rc<PeerLink>> = FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or_else(Vec::new, |fabric| fabric.links.values().cloned().collect())
    });
    for link in links {
        for frame in frames {
            // A send that fails is a peer that has gone; `onclose` ends
            // adjudicated play, and there is nothing useful to do here.
            let _ = link.send(frame);
        }
    }
}

fn deliver_applied(change: &AdjudicatedChange) {
    let callback = FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|fabric| fabric.on_applied.clone())
    });
    if let Some(callback) = callback {
        let _ = callback.call1(
            &JsValue::NULL,
            &JsValue::from_str(&change.to_json().to_string()),
        );
    }
}
