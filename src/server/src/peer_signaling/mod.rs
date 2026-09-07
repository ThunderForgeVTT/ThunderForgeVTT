//! Spec 028 (T086): peer signaling — the server as a post box.
//!
//! See `specs/028-client-world-cache/contracts/peer-protocol.md`. Clients
//! that want to trade content-addressed bytes over WebRTC need to exchange
//! SDP offers, answers and ICE candidates before a data channel exists. That
//! exchange rides the `graphql-ws` connection everybody already holds, so
//! there is no second service and no second auth surface.
//!
//! The server relays opaque strings between live sessions in one world and
//! **never interprets them** (FR-044). It does not vouch for a peer, does not
//! promise reachability, and does not participate in the transfer. Every
//! authorization decision that matters was already made by `worldSyncPlan`:
//! a client may only ask a peer for a fingerprint the server put in its own
//! `fetch` list (FR-047), and it verifies the bytes against that fingerprint
//! before storing them (FR-046). A malicious peer can waste bandwidth and
//! nothing else.
//!
//! # What is in this directory
//!
//! Split three ways, along the seams the arguments already fell on rather
//! than by size:
//!
//! - [`registry`] — who is reachable right now, and for exactly how long.
//!   Carries the lifetime argument the play-field claim was later modelled
//!   on (`play_field.rs` cites it by name).
//! - [`admission`] — who may enter that registry at all: world membership,
//!   the bound on a session id, and the play-field claim (spec 036 FR-038).
//!   One place, checked once, so no message path has to know the rule.
//! - [`surface`] — the GraphQL mutation, query and subscription that clients
//!   actually call.
//!
//! The order matters when reading: the registry says what a registration
//! *is*, admission says who gets one, and the surface is only the plumbing
//! that connects the two to a socket.

mod admission;
mod registry;
mod surface;

pub use admission::{PeerSignalingError, to_graphql_error};
pub use registry::{
    GraphQLPeerSignal, MAX_SESSION_ID_LEN, PeerRegistry, PeerSessionGuard, registry,
};
pub use surface::{
    GraphQLPeerSignalInput, PeerSignalingMutation, PeerSignalingQuery, peer_sessions_impl,
    peer_signals_stream, send_peer_signal_impl,
};
