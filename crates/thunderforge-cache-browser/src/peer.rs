//! Peer-assisted content distribution: asking another client for a *hash*.
//!
//! Spec 028 T088–T091 and T098/T100/T101, FR-044 to FR-050 and FR-057 to
//! FR-063, SC-012/SC-013/SC-014, `contracts/peer-protocol.md`, ADR-052.
//!
//! # The one idea the whole protocol rests on
//!
//! Peers are asked for a **hash**, never for a thing. A peer cannot
//! substitute different content, because the requester verifies the hash it
//! asked for before storing — so a malicious peer can waste bandwidth and
//! nothing else. That single property is what lets both endpoints be
//! untrusted, and it is why nothing in this module contains an authorization
//! decision. The requester's entitlement came from the server's own
//! `SyncPlan`; the server already decided.
//!
//! # Strict optimization, or nothing
//!
//! Every failure in here ends at *fetch it from the server* (FR-048). A
//! declining peer, a silent peer, a peer that hangs up halfway, a peer that
//! sends bytes hashing to something else — all of them produce the same
//! observable outcome as never having had a peer at all, only sooner or
//! later in wall-clock time. There is deliberately no error type: a caller
//! that has to handle a peer failure is a caller that can be *made worse* by
//! one, and SC-013 forbids that.
//!
//! # Shape
//!
//! Pure first, as everywhere in this crate, so the rules that matter are
//! testable without a browser:
//!
//! - [`PeerMessage`] — the `REQUEST / OFFER / CHUNK / DONE / DECLINE`
//!   framing (T088).
//! - [`PlanScope`] / [`PeerRequest`] — you may only ask for what your own
//!   plan lists, made *unexpressible* rather than checked (T089).
//! - [`PeerDownload`] — the receive machine whose only byte-yielding exit is
//!   behind [`thunderforge_cache_core::fingerprint::verify`] (T090).
//! - [`PeerServer`] — what this client is willing to serve, and how often
//!   (T091).
//! - [`Adjudication`] — the `PROPOSE / ADJUDICATE / APPLY` protocol that runs
//!   only while server-isolated, ordered by a session-agreed nonce sequence
//!   and never by a clock, scoped to token position/rotation/scale by a type
//!   that cannot hold anything else (T098, T100, T101).
//!
//! A `#[cfg(target_arch = "wasm32")]` block below holds the `RTCPeerConnection`
//! and `RTCDataChannel` glue that drives them, and nothing else.

use std::collections::{BTreeMap, BTreeSet};

use thunderforge_cache_core::delta::SyncPlan;
use thunderforge_cache_core::{Fingerprint, fingerprint};
use uuid::Uuid;

#[path = "peer/transfer.rs"]
mod transfer;
pub use transfer::*;

#[path = "peer/trust.rs"]
mod trust;
pub use trust::*;

#[path = "peer/adjudication.rs"]
mod adjudication;
pub use adjudication::*;

#[cfg(target_arch = "wasm32")]
#[path = "peer/wasm.rs"]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    BlobProvider, activity, adjudication_active, adjudication_end, adjudication_propose,
    adjudication_server_returned, adjudication_submissions, begin_adjudication, connect_to,
    disable, enable, is_active, membership_lost, note_stored, on_signal, peer_count, set_held,
    set_plan, set_provider, try_fetch,
};
