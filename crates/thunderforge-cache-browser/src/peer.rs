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

/// How much of one transfer travels in a single data-channel message.
///
/// 16 KiB is the largest payload every SCTP implementation the browsers ship
/// accepts without negotiating message fragmentation. Larger frames work in
/// Chrome and fail in Safari, and a transfer that fails only on one browser
/// is worse than a slightly chattier one that works everywhere.
pub const CHUNK_BYTES: usize = 16 * 1024;

/// The largest single item this client will accept from a peer.
///
/// A ceiling exists at all because an `OFFER` is a number chosen by the peer
/// and a requester that trusts it will happily allocate whatever it is told.
/// Only ever consulted when the plan did not carry a size; when it did, the
/// server's figure is the ceiling and this is not reached.
pub const MAX_TRANSFER_BYTES: u64 = 64 * 1024 * 1024;

/// How long a transfer may make no progress before the peer is abandoned.
///
/// FR-048's teeth. A peer that has gone quiet is indistinguishable from a
/// peer that is merely slow, and the protocol does not need to tell them
/// apart: both are worse than the server, and both are abandoned for it.
pub const STALL_MS: u64 = 3_000;

/// How long one transfer may take in total, however steadily it progresses.
///
/// A peer trickling one chunk every two seconds never trips [`STALL_MS`] and
/// is still slower than the server. Progress is not the same as usefulness.
pub const DEADLINE_MS: u64 = 20_000;

/// The window every rate limit below is measured over.
pub const RATE_WINDOW_MS: u64 = 10_000;

/// Requests one peer may make per [`RATE_WINDOW_MS`] before being told `BUSY`.
///
/// "A peer is a participant in a game, not a CDN." A player opening a scene
/// asks for a handful of assets; nothing legitimate asks for dozens a second.
pub const MAX_REQUESTS_PER_WINDOW: u32 = 32;

/// Requests in one window past which the channel is dropped rather than
/// declined.
///
/// `DECLINE` still costs a read and a write per request, so a peer that
/// ignores it is not rate-limited by it. At some point the answer has to be
/// to stop listening (peer-protocol.md, "Peer floods requests").
pub const FLOOD_DROP_REQUESTS: u32 = 256;

/// Bytes one peer may be served per [`RATE_WINDOW_MS`].
pub const MAX_BYTES_PER_WINDOW: u64 = 16 * 1024 * 1024;

/// Transfers to one peer that may be in flight at once.
pub const MAX_CONCURRENT_SERVES: u32 = 2;

// ---------------------------------------------------------------------------
// T088 — framing
// ---------------------------------------------------------------------------

/// Why a peer will not serve a fingerprint.
///
/// **Never information about whether content exists.** A requester that read
/// `NOT_HELD` as "there is no such content" would be taking a stranger's word
/// over the server's plan, which is the one thing this protocol never does.
/// The variants exist for the *sender's* diagnostics; the receiver treats all
/// three identically, and [`Fallback::Declined`] is how that is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    /// This client does not hold that fingerprint, or holds it unverified.
    NotHeld,
    /// This client is no longer a member of the world (FR-050).
    NotPermitted,
    /// Rate-limited.
    Busy,
}

impl DeclineReason {
    const fn tag(self) -> u8 {
        match self {
            Self::NotHeld => 0,
            Self::NotPermitted => 1,
            Self::Busy => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::NotHeld),
            1 => Some(Self::NotPermitted),
            2 => Some(Self::Busy),
            _ => None,
        }
    }
}

/// One frame on a peer data channel.
///
/// Binary rather than JSON, and one frame shape rather than two, because a
/// `CHUNK` carries arbitrary bytes: a text protocol would have to base64 them
/// (a third more bandwidth for the single largest thing this protocol moves),
/// and a mixed text/binary protocol would give a peer two parsers to confuse
/// against each other.
///
/// Layout: one tag byte, then the 32 raw fingerprint bytes every message
/// carries, then the per-variant tail. The fingerprint is first and always
/// present so that "is this about something I asked for?" is answerable
/// before anything else is parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerMessage {
    /// "Do you have these bytes?" The only message a requester sends.
    Request { fingerprint: Fingerprint },
    /// "I do, and it is this many bytes." Sizes are checked against the
    /// server's figure before a single chunk is accepted.
    Offer {
        fingerprint: Fingerprint,
        byte_size: u64,
    },
    /// Part of the content, in strict sequence from zero.
    Chunk {
        fingerprint: Fingerprint,
        seq: u32,
        bytes: Vec<u8>,
    },
    /// "That is all of it." Verification happens here, never before.
    Done { fingerprint: Fingerprint },
    /// "No." See [`DeclineReason`] for what this never means.
    Decline {
        fingerprint: Fingerprint,
        reason: DeclineReason,
    },
}

const TAG_REQUEST: u8 = 1;
const TAG_OFFER: u8 = 2;
const TAG_CHUNK: u8 = 3;
const TAG_DONE: u8 = 4;
const TAG_DECLINE: u8 = 5;

/// Tag byte plus the fingerprint every frame carries.
const HEADER_BYTES: usize = 1 + 32;

impl PeerMessage {
    /// The fingerprint this frame is about. Every variant has one, which is
    /// what makes "not something I asked for" a total answer.
    pub fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::Request { fingerprint }
            | Self::Offer { fingerprint, .. }
            | Self::Chunk { fingerprint, .. }
            | Self::Done { fingerprint }
            | Self::Decline { fingerprint, .. } => *fingerprint,
        }
    }

    /// Render for the wire.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_BYTES + 12);
        let (tag, fingerprint) = match self {
            Self::Request { fingerprint } => (TAG_REQUEST, fingerprint),
            Self::Offer { fingerprint, .. } => (TAG_OFFER, fingerprint),
            Self::Chunk { fingerprint, .. } => (TAG_CHUNK, fingerprint),
            Self::Done { fingerprint } => (TAG_DONE, fingerprint),
            Self::Decline { fingerprint, .. } => (TAG_DECLINE, fingerprint),
        };
        out.push(tag);
        out.extend_from_slice(fingerprint.as_bytes());
        match self {
            Self::Request { .. } | Self::Done { .. } => {}
            Self::Offer { byte_size, .. } => out.extend_from_slice(&byte_size.to_be_bytes()),
            Self::Chunk { seq, bytes, .. } => {
                out.extend_from_slice(&seq.to_be_bytes());
                out.extend_from_slice(bytes);
            }
            Self::Decline { reason, .. } => out.push(reason.tag()),
        }
        out
    }

    /// Read a frame, or `None`.
    ///
    /// Total and silent, for the same reason [`crate::signal::parse`] is: this
    /// reads a channel an untrusted party writes to, so a malformed frame is
    /// not an error condition, it is simply not a message. The one thing that
    /// must never happen is a partially-parsed frame being acted on, which
    /// the `Option` makes unexpressible.
    pub fn decode(frame: &[u8]) -> Option<Self> {
        if frame.len() < HEADER_BYTES {
            return None;
        }
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&frame[1..HEADER_BYTES]);
        // Round-tripping through hex is how a `Fingerprint` is constructible
        // from raw bytes without widening its API; `to_hex`/`from_hex` are
        // exact inverses, so this cannot fail.
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let fingerprint = Fingerprint::from_hex(&hex).ok()?;
        let tail = &frame[HEADER_BYTES..];

        match frame[0] {
            TAG_REQUEST if tail.is_empty() => Some(Self::Request { fingerprint }),
            TAG_DONE if tail.is_empty() => Some(Self::Done { fingerprint }),
            TAG_OFFER if tail.len() == 8 => Some(Self::Offer {
                fingerprint,
                byte_size: u64::from_be_bytes(tail.try_into().ok()?),
            }),
            TAG_CHUNK if tail.len() >= 4 => Some(Self::Chunk {
                fingerprint,
                seq: u32::from_be_bytes(tail[..4].try_into().ok()?),
                bytes: tail[4..].to_vec(),
            }),
            TAG_DECLINE if tail.len() == 1 => Some(Self::Decline {
                fingerprint,
                reason: DeclineReason::from_tag(tail[0])?,
            }),
            _ => None,
        }
    }
}

/// Split content into the `OFFER`/`CHUNK`…/`DONE` frames that carry it.
///
/// Kept pure and next to the decoder so the two halves of one transfer are
/// read together, and so the sequence numbering the receiver insists on is
/// produced by something a test can drive without a channel.
pub fn serve_frames(fingerprint: &Fingerprint, bytes: &[u8]) -> Vec<PeerMessage> {
    let mut frames = vec![PeerMessage::Offer {
        fingerprint: *fingerprint,
        byte_size: bytes.len() as u64,
    }];
    for (seq, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        frames.push(PeerMessage::Chunk {
            fingerprint: *fingerprint,
            seq: seq as u32,
            bytes: chunk.to_vec(),
        });
    }
    frames.push(PeerMessage::Done {
        fingerprint: *fingerprint,
    });
    frames
}

// ---------------------------------------------------------------------------
// T089 — you may only ask for what the server told you to fetch
// ---------------------------------------------------------------------------

/// Permission to ask a peer for exactly one fingerprint.
///
/// **The enforcement point for FR-047**, and it enforces by construction
/// rather than by checking. Every field is private and there is no public
/// constructor, so the only way anywhere in the program to obtain one is
/// [`PlanScope::request`], which will only mint one for a fingerprint the
/// server itself put in this client's `SyncPlan.fetch`. Asking a peer for
/// anything else is not a rejected operation; it is not an expressible one.
///
/// Not `Clone` on purpose: one token, one transfer. A clonable token could be
/// parked and replayed against a later plan that no longer lists it.
#[derive(Debug, PartialEq, Eq)]
pub struct PeerRequest {
    world_id: Uuid,
    fingerprint: Fingerprint,
    /// The server's byte count, or zero when the plan did not carry one.
    /// An `OFFER` disagreeing with a non-zero figure ends the transfer.
    byte_size: u64,
}

impl PeerRequest {
    /// What is being asked for.
    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// The world this request belongs to. Peer connections are confined to
    /// one world (FR-050), so a request must never cross into another.
    pub fn world_id(&self) -> Uuid {
        self.world_id
    }

    /// The size the server promised, or zero if it did not say.
    pub fn expected_bytes(&self) -> u64 {
        self.byte_size
    }

    /// The frame that asks for it.
    pub fn frame(&self) -> Vec<u8> {
        PeerMessage::Request {
            fingerprint: self.fingerprint,
        }
        .encode()
    }
}

/// The set of fingerprints this client is currently entitled to fetch, taken
/// verbatim from the server's plan.
///
/// Built from a [`SyncPlan`] and from nothing else. There is no `insert`, no
/// `extend`, and no constructor that takes a bare fingerprint — widening this
/// type is what a future change to defeat FR-047 would look like, so the type
/// is shaped to make that a visible act rather than an easy one.
///
/// Replaced wholesale on every sync. A scope outliving the plan that produced
/// it would let a client keep asking for content the server has since stopped
/// listing, which is precisely the revocation hole FR-015 closes elsewhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanScope {
    world_id: Uuid,
    wanted: BTreeMap<Fingerprint, u64>,
}

impl PlanScope {
    /// The fingerprints in `plan.fetch`, and only those.
    pub fn from_plan(world_id: Uuid, plan: &SyncPlan) -> Self {
        let mut wanted = BTreeMap::new();
        for item in &plan.fetch {
            // Largest wins when one fingerprint appears twice with differing
            // sizes: the check downstream is "the offer must match", and the
            // smaller figure would reject content the server does list.
            let entry = wanted.entry(item.fingerprint).or_insert(item.byte_size);
            *entry = (*entry).max(item.byte_size);
        }
        Self { world_id, wanted }
    }

    /// An empty scope: ask no peer for anything.
    ///
    /// What a client holds between losing a plan and getting the next one,
    /// and what it holds forever if the plan never parses. Both mean
    /// server-only, which is the correct answer to not knowing.
    pub fn none(world_id: Uuid) -> Self {
        Self {
            world_id,
            wanted: BTreeMap::new(),
        }
    }

    pub fn world_id(&self) -> Uuid {
        self.world_id
    }

    pub fn len(&self) -> usize {
        self.wanted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.wanted.is_empty()
    }

    /// Whether the server's plan lists this fingerprint.
    pub fn contains(&self, fingerprint: &Fingerprint) -> bool {
        self.wanted.contains_key(fingerprint)
    }

    /// Mint permission to ask a peer for `fingerprint`, if and only if the
    /// server's plan lists it.
    ///
    /// The whole of FR-047's client side is this `Option`.
    pub fn request(&self, fingerprint: &Fingerprint) -> Option<PeerRequest> {
        let byte_size = *self.wanted.get(fingerprint)?;
        Some(PeerRequest {
            world_id: self.world_id,
            fingerprint: *fingerprint,
            byte_size,
        })
    }
}

// ---------------------------------------------------------------------------
// T090 — verify before storing, and there is no other way out
// ---------------------------------------------------------------------------

/// Why this transfer ended at the server instead.
///
/// Every variant is a fall-back, never a failure. The distinction that
/// actually matters to a caller is [`Fallback::distrusts_peer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// The peer said no. Carries no information about whether the content
    /// exists — see [`DeclineReason`].
    Declined,
    /// The bytes did not hash to the fingerprint they were asked for
    /// (FR-046). The one variant the user-visible indicator counts.
    VerificationFailed,
    /// The channel closed, or the peer went away, mid-transfer. Nothing is
    /// stored: the buffer only ever leaves this type verified.
    PeerGone,
    /// No progress for [`STALL_MS`], or [`DEADLINE_MS`] in total.
    Stalled,
    /// The peer offered a size the server did not promise, or sent more bytes
    /// than it offered. Either is a lie about the content before any of it
    /// arrives, so the transfer ends without allocating for it.
    SizeMismatch,
    /// Frames arrived out of order, twice, or before the offer.
    Protocol,
}

impl Fallback {
    /// Whether this peer should be asked for anything again this session.
    ///
    /// `Declined` and `PeerGone` are ordinary: a peer that does not hold
    /// something, or whose laptop closed, has done nothing wrong and may be
    /// perfectly useful for the next item. `Stalled` is likewise a statement
    /// about a network, not about a peer's honesty — though the caller stops
    /// *this* transfer regardless.
    ///
    /// The rest are a peer failing to send what it was asked for, which the
    /// contract answers with "do not retry that peer".
    pub fn distrusts_peer(self) -> bool {
        matches!(
            self,
            Self::VerificationFailed | Self::SizeMismatch | Self::Protocol
        )
    }
}

/// What one received frame did to a transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStep {
    /// Still going.
    Continue,
    /// Not about this transfer, and therefore not about anything. Unrequested
    /// content is ignored *entirely* — it does not advance the state machine,
    /// does not reset the stall timer, and is never buffered.
    Ignore,
    /// Bytes that hashed to the fingerprint that was asked for. **The only
    /// way content leaves this type**, and it is reachable solely through
    /// [`fingerprint::verify`] returning `Ok`.
    Verified(Vec<u8>),
    /// Go to the server.
    FallBack(Fallback),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Asked, nothing back yet.
    Requested,
    /// Offered and receiving.
    Receiving,
    /// Over, one way or another. Terminal: nothing reopens a transfer.
    Done,
}

/// One in-flight fetch of one fingerprint from one peer.
///
/// Holds the partial buffer, and is the reason "no partial stores" is a
/// property rather than a rule: the buffer is private and the only public
/// path that yields it is [`DownloadStep::Verified`], produced after
/// [`fingerprint::verify`] and nowhere else. A peer vanishing mid-transfer
/// drops the whole thing on the floor because there is no expressible way to
/// get half of it out.
#[derive(Debug)]
pub struct PeerDownload {
    request: PeerRequest,
    phase: Phase,
    offered: u64,
    buffer: Vec<u8>,
    next_seq: u32,
    started_ms: u64,
    progress_ms: u64,
}

impl PeerDownload {
    /// Begin, and produce the frame to send.
    ///
    /// Takes the token by value: a [`PeerRequest`] is consumed by the
    /// transfer it authorises, so one plan entry cannot be spent twice.
    pub fn begin(request: PeerRequest, now_ms: u64) -> (Self, Vec<u8>) {
        let frame = request.frame();
        (
            Self {
                request,
                phase: Phase::Requested,
                offered: 0,
                buffer: Vec::new(),
                next_seq: 0,
                started_ms: now_ms,
                progress_ms: now_ms,
            },
            frame,
        )
    }

    /// What is being fetched.
    pub fn fingerprint(&self) -> Fingerprint {
        self.request.fingerprint()
    }

    /// Bytes received so far. For diagnostics only — they are not obtainable.
    pub fn received(&self) -> usize {
        self.buffer.len()
    }

    /// Feed a raw data-channel frame.
    ///
    /// A frame that does not parse is [`DownloadStep::Ignore`], not a
    /// protocol violation: the channel is shared with whatever the peer feels
    /// like sending, and treating noise as an attack would let anyone end a
    /// transfer that was going to succeed.
    pub fn on_frame(&mut self, frame: &[u8], now_ms: u64) -> DownloadStep {
        match PeerMessage::decode(frame) {
            Some(message) => self.on_message(message, now_ms),
            None => DownloadStep::Ignore,
        }
    }

    /// Feed a decoded message.
    pub fn on_message(&mut self, message: PeerMessage, now_ms: u64) -> DownloadStep {
        // Two gates before anything else, and in this order. Content for a
        // fingerprint we did not ask for is ignored *entirely* — including
        // when this transfer is already over, so a late frame from a finished
        // transfer cannot reopen one.
        if message.fingerprint() != self.request.fingerprint() || self.phase == Phase::Done {
            return DownloadStep::Ignore;
        }

        match message {
            // Never information about whether the content exists (FR-045):
            // the plan said it does, and a stranger does not overrule the
            // server. Only this transfer ends.
            PeerMessage::Decline { .. } => self.finish(Fallback::Declined),

            PeerMessage::Offer { byte_size, .. } => {
                if self.phase != Phase::Requested {
                    return self.finish(Fallback::Protocol);
                }
                // The server's figure is the authority when it gave one. A
                // peer offering a different size is offering different
                // content, which the hash check would reject at the end of a
                // transfer we can decline to start.
                let promised = self.request.expected_bytes();
                let acceptable = if promised > 0 {
                    byte_size == promised
                } else {
                    byte_size <= MAX_TRANSFER_BYTES
                };
                if !acceptable {
                    return self.finish(Fallback::SizeMismatch);
                }
                self.offered = byte_size;
                self.phase = Phase::Receiving;
                self.progress_ms = now_ms;
                // Reserving against a number the *server* published, not one
                // the peer chose, is what keeps a hostile offer from being an
                // allocation primitive.
                self.buffer
                    .reserve(byte_size.min(MAX_TRANSFER_BYTES) as usize);
                DownloadStep::Continue
            }

            PeerMessage::Chunk { seq, bytes, .. } => {
                if self.phase != Phase::Receiving {
                    return self.finish(Fallback::Protocol);
                }
                // Strict sequence. Out of order, repeated, or skipped are all
                // the same answer: this peer is not sending the content, so
                // stop rather than try to reassemble a stream someone else is
                // choosing the shape of.
                if seq != self.next_seq || bytes.len() > CHUNK_BYTES {
                    return self.finish(Fallback::Protocol);
                }
                let total = self.buffer.len() as u64 + bytes.len() as u64;
                if total > self.offered {
                    return self.finish(Fallback::SizeMismatch);
                }
                self.buffer.extend_from_slice(&bytes);
                self.next_seq += 1;
                self.progress_ms = now_ms;
                if now_ms.saturating_sub(self.started_ms) > DEADLINE_MS {
                    return self.finish(Fallback::Stalled);
                }
                DownloadStep::Continue
            }

            PeerMessage::Done { .. } => {
                if self.phase != Phase::Receiving || self.buffer.len() as u64 != self.offered {
                    return self.finish(Fallback::SizeMismatch);
                }
                // The single sanctioned trust choke point. Nothing in this
                // module compares a digest by hand, and this is the only
                // statement in it that hands bytes to a caller.
                let verified = fingerprint::verify(&self.buffer, &self.request.fingerprint());
                self.phase = Phase::Done;
                match verified {
                    Ok(()) => DownloadStep::Verified(std::mem::take(&mut self.buffer)),
                    Err(_) => {
                        // Dropped here rather than left for the caller: the
                        // bytes are known-bad and holding them is only an
                        // opportunity to use them by mistake.
                        self.buffer = Vec::new();
                        DownloadStep::FallBack(Fallback::VerificationFailed)
                    }
                }
            }

            // A peer asking us for something is not part of this transfer.
            // It is handled by `PeerServer`, and here it is noise.
            PeerMessage::Request { .. } => DownloadStep::Ignore,
        }
    }

    /// Give the clock a chance to end a transfer nobody is advancing.
    ///
    /// The other half of FR-048: a peer that never answers, or answers ever
    /// more slowly, must not hold up bytes the server would already have
    /// delivered. Silence is a fall-back, not a wait.
    pub fn tick(&mut self, now_ms: u64) -> DownloadStep {
        if self.phase == Phase::Done {
            return DownloadStep::Ignore;
        }
        if now_ms.saturating_sub(self.progress_ms) > STALL_MS
            || now_ms.saturating_sub(self.started_ms) > DEADLINE_MS
        {
            return self.finish(Fallback::Stalled);
        }
        DownloadStep::Continue
    }

    /// The channel closed. Whatever arrived is discarded.
    pub fn peer_gone(&mut self) -> DownloadStep {
        if self.phase == Phase::Done {
            return DownloadStep::Ignore;
        }
        self.finish(Fallback::PeerGone)
    }

    fn finish(&mut self, reason: Fallback) -> DownloadStep {
        self.phase = Phase::Done;
        self.buffer = Vec::new();
        DownloadStep::FallBack(reason)
    }
}

// ---------------------------------------------------------------------------
// T091 — what this client is willing to serve
// ---------------------------------------------------------------------------

/// The answer to one peer's `REQUEST`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeDecision {
    /// Read the blob and send it. The caller must still verify what it read
    /// before sending — `OpfsStore::read_blob` does, which is why a hit there
    /// is the only thing that ever reaches this path.
    Serve,
    /// Send a `DECLINE` with this reason.
    Decline(DeclineReason),
    /// Stop listening to this peer. `DECLINE` costs a read and a write, so a
    /// peer that ignores it is not being limited by it.
    DropChannel,
}

/// One peer's recent behaviour, in a sliding window that is really a
/// resetting one.
///
/// A true sliding window needs a timestamp per request; a resetting window
/// needs two numbers and lets a peer briefly do double the rate across a
/// boundary. For "do not be a CDN" that is the right trade — the limit is
/// about orders of magnitude, not about precision.
#[derive(Debug, Clone, Copy, Default)]
struct PeerMeter {
    window_started_ms: u64,
    requests: u32,
    bytes: u64,
    in_flight: u32,
}

impl PeerMeter {
    fn roll(&mut self, now_ms: u64) {
        if now_ms.saturating_sub(self.window_started_ms) >= RATE_WINDOW_MS {
            self.window_started_ms = now_ms;
            self.requests = 0;
            self.bytes = 0;
        }
    }
}

/// What this client will serve to peers, and to whom.
///
/// Deliberately not an authorization decision, and the shape says so: there
/// is nothing in here about *who* is asking beyond rate accounting. The
/// requester's entitlement came from the server's plan before it ever opened
/// a channel (`peer-protocol.md`, "Neither side makes an authorization
/// decision"). What this type decides is only what this client *can* honestly
/// answer with, and how much of it.
#[derive(Debug)]
pub struct PeerServer {
    world_id: Uuid,
    member: bool,
    /// Fingerprints this client holds *and* has verified. Populated from the
    /// index/store, never from a request.
    held: BTreeSet<Fingerprint>,
    meters: BTreeMap<String, PeerMeter>,
}

impl PeerServer {
    /// A server for one world, serving nothing until told what is held.
    pub fn new(world_id: Uuid) -> Self {
        Self {
            world_id,
            member: true,
            held: BTreeSet::new(),
            meters: BTreeMap::new(),
        }
    }

    pub fn world_id(&self) -> Uuid {
        self.world_id
    }

    /// Note that a fingerprint is on disk and verified.
    ///
    /// Called after a successful store, never before. The window between a
    /// blob being written and this being called costs a peer one `DECLINE`;
    /// the reverse ordering would cost them a transfer of bytes that are not
    /// there yet.
    pub fn holds(&mut self, fingerprint: Fingerprint) {
        self.held.insert(fingerprint);
    }

    /// Note that a fingerprint is no longer held — evicted, repaired away, or
    /// revoked.
    pub fn forgets(&mut self, fingerprint: &Fingerprint) {
        self.held.remove(fingerprint);
    }

    /// Replace the held set wholesale, e.g. after a sync.
    pub fn holds_only(&mut self, fingerprints: impl IntoIterator<Item = Fingerprint>) {
        self.held = fingerprints.into_iter().collect();
    }

    pub fn held_count(&self) -> usize {
        self.held.len()
    }

    /// Membership in this world ended (FR-050).
    ///
    /// **Immediate and irreversible for this instance.** Serving stops on the
    /// next request and every in-flight transfer is abandoned; regaining
    /// membership means a fresh sync and a fresh `PeerServer`, because a
    /// server that could be switched back on is one that can be switched back
    /// on by mistake.
    pub fn membership_lost(&mut self) {
        self.member = false;
        self.held.clear();
        for meter in self.meters.values_mut() {
            meter.in_flight = 0;
        }
    }

    /// Whether this client is serving at all.
    pub fn is_serving(&self) -> bool {
        self.member
    }

    /// Whether a transfer already under way must be abandoned now.
    ///
    /// Checked between chunks, so losing membership stops a send in the
    /// middle rather than at the end of it.
    pub fn must_abort(&self) -> bool {
        !self.member
    }

    /// Decide what to answer one peer's request with.
    pub fn on_request(
        &mut self,
        peer: &str,
        fingerprint: &Fingerprint,
        now_ms: u64,
    ) -> ServeDecision {
        // Membership first, before the rate meter and before the held set.
        // A client that has lost the world must not even reveal, by the shape
        // of its refusal, what it used to hold.
        if !self.member {
            return ServeDecision::Decline(DeclineReason::NotPermitted);
        }

        // A fresh meter's window began at time zero, which any real clock is
        // already past — so a peer's first request opens its own window
        // rather than inheriting one. Special-casing zero here is what made
        // an early version never reset a window at all.
        let meter = self.meters.entry(peer.to_string()).or_default();
        meter.roll(now_ms);
        meter.requests = meter.requests.saturating_add(1);

        if meter.requests > FLOOD_DROP_REQUESTS {
            return ServeDecision::DropChannel;
        }
        if meter.requests > MAX_REQUESTS_PER_WINDOW
            || meter.bytes > MAX_BYTES_PER_WINDOW
            || meter.in_flight >= MAX_CONCURRENT_SERVES
        {
            return ServeDecision::Decline(DeclineReason::Busy);
        }

        if !self.held.contains(fingerprint) {
            return ServeDecision::Decline(DeclineReason::NotHeld);
        }

        meter.in_flight += 1;
        ServeDecision::Serve
    }

    /// Account for bytes actually sent, and release the in-flight slot.
    ///
    /// Called once per served transfer, whether it completed or not: a
    /// transfer that was cut short still cost bandwidth, and a slot that is
    /// never released is a peer permanently `BUSY`.
    pub fn served(&mut self, peer: &str, bytes: u64) {
        if let Some(meter) = self.meters.get_mut(peer) {
            meter.bytes = meter.bytes.saturating_add(bytes);
            meter.in_flight = meter.in_flight.saturating_sub(1);
        }
    }

    /// Forget a peer that has disconnected.
    pub fn peer_gone(&mut self, peer: &str) {
        self.meters.remove(peer);
    }
}

// ---------------------------------------------------------------------------
// Who we will talk to, and what to tell the user about it
// ---------------------------------------------------------------------------

/// Peers this client has stopped asking, and why it stopped.
///
/// Session-lifetime and nothing more (FR-050). There is no persistence and no
/// reporting: a peer that sent bad bytes is dropped from *this* client's
/// consideration and no further conclusion is drawn, because a peer behind a
/// broken proxy and a peer being malicious are indistinguishable from here
/// and the response to both is identical anyway.
#[derive(Debug, Clone, Default)]
pub struct PeerTrust {
    distrusted: BTreeSet<String>,
}

impl PeerTrust {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this peer may still be asked for content.
    pub fn trusts(&self, peer: &str) -> bool {
        !self.distrusted.contains(peer)
    }

    /// Record the outcome of a transfer. Only the outcomes the contract calls
    /// out — mismatched or fabricated content — cost a peer its trust.
    pub fn record(&mut self, peer: &str, fallback: Fallback) {
        if fallback.distrusts_peer() {
            self.distrusted.insert(peer.to_string());
        }
    }

    pub fn distrusted_count(&self) -> usize {
        self.distrusted.len()
    }
}

/// What the FR-049 indicator shows.
///
/// Mirrors `PeerTransferState` in `apps/web/src/services/peerTransfer.ts`
/// minus `enabled`, which is the user's to set and never this side's to
/// report. Counters only: no peer identities, no addresses, no timings — the
/// panel exists to disclose that peer transfer is happening, not to profile
/// who is in the game (FR-052, FR-054).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeerActivity {
    pub connected_peers: usize,
    pub bytes_from_peers: u64,
    pub verification_failures: u32,
}

impl PeerActivity {
    /// The exact object `reportPeerTransferActivity` takes.
    pub fn to_json(self) -> String {
        serde_json::json!({
            "connectedPeers": self.connected_peers,
            "bytesFromPeers": self.bytes_from_peers,
            "verificationFailures": self.verification_failures,
        })
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// T098 / T100 / T101 — peer adjudication, in the server-isolated state only
// ---------------------------------------------------------------------------

/// One token movement, and the only shape a peer-adjudicated change has.
///
/// **T101 by construction** (FR-060). There is no field here for a creation,
/// a deletion, a permission, a name, a hit point, or anything else — so a
/// proposal outside position, rotation and scale is not a rejected message,
/// it is an unrepresentable one. That is the same choice [`PlanScope`] makes
/// for entitlement, and for the same reason: a check can be forgotten at one
/// call site, while a type that cannot hold the wrong thing cannot be.
///
/// At least one of the three must be present. An empty transform is not a
/// harmless no-op — it is a proposal that says nothing, and accepting one
/// would mean a nonce consumed and an `APPLY` broadcast for no change.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenTransform {
    position: Option<[f64; 2]>,
    rotation: Option<f64>,
    scale: Option<f64>,
}

/// Which fields a transform carries, on the wire.
const FIELD_POSITION: u8 = 0b001;
const FIELD_ROTATION: u8 = 0b010;
const FIELD_SCALE: u8 = 0b100;
/// Every bit that has a meaning. A frame setting any other bit is claiming a
/// field this protocol does not have, which is exactly what FR-060 forbids.
const FIELD_ALL: u8 = FIELD_POSITION | FIELD_ROTATION | FIELD_SCALE;

impl TokenTransform {
    /// Move a token.
    pub fn position(x: f64, y: f64) -> Self {
        Self {
            position: Some([x, y]),
            ..Self::default()
        }
    }

    /// Turn a token.
    pub fn rotation(radians: f64) -> Self {
        Self {
            rotation: Some(radians),
            ..Self::default()
        }
    }

    /// Resize a token.
    pub fn scale(factor: f64) -> Self {
        Self {
            scale: Some(factor),
            ..Self::default()
        }
    }

    pub fn with_position(mut self, x: f64, y: f64) -> Self {
        self.position = Some([x, y]);
        self
    }

    pub fn with_rotation(mut self, radians: f64) -> Self {
        self.rotation = Some(radians);
        self
    }

    pub fn with_scale(mut self, factor: f64) -> Self {
        self.scale = Some(factor);
        self
    }

    pub fn position_of(&self) -> Option<[f64; 2]> {
        self.position
    }

    pub fn rotation_of(&self) -> Option<f64> {
        self.rotation
    }

    pub fn scale_of(&self) -> Option<f64> {
        self.scale
    }

    /// Whether this proposes nothing at all.
    pub fn is_empty(&self) -> bool {
        self.position.is_none() && self.rotation.is_none() && self.scale.is_none()
    }

    fn mask(&self) -> u8 {
        let mut mask = 0;
        if self.position.is_some() {
            mask |= FIELD_POSITION;
        }
        if self.rotation.is_some() {
            mask |= FIELD_ROTATION;
        }
        if self.scale.is_some() {
            mask |= FIELD_SCALE;
        }
        mask
    }

    /// The JSON the engine's own token mutation takes, so an adjudicated
    /// change replays through the ordinary path rather than a second one.
    pub fn to_json(&self) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        if let Some([x, y]) = self.position {
            object.insert("x".into(), serde_json::json!(x));
            object.insert("y".into(), serde_json::json!(y));
        }
        if let Some(rotation) = self.rotation {
            object.insert("rotation".into(), serde_json::json!(rotation));
        }
        if let Some(scale) = self.scale {
            object.insert("scale".into(), serde_json::json!(scale));
        }
        serde_json::Value::Object(object)
    }
}

/// Where one proposal sits in the session's agreed order.
///
/// **A counter and an origin, and deliberately not a clock.** A wall-clock
/// timestamp is chosen by the machine that sends it: a client with a skewed
/// clock silently wins every conflict it takes part in, and a client that
/// wants to win only has to lie. That is the same reasoning that keeps
/// timestamps out of `thunderforge_cache_core::conflict`, and it applies here
/// with more force, because offline there is no server to notice.
///
/// `seq` is a Lamport counter: every client raises its own past every nonce
/// it sees, so a proposal made in response to another is always ordered after
/// it, whatever either machine believes the time is. Ties — two clients
/// proposing at the same logical instant, having seen the same history — are
/// broken by `origin`, which is an opaque per-page-load session id. The
/// result is a total order every participant computes identically from the
/// messages alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nonce {
    /// The logical counter. Ordered on first, which is what makes causality
    /// win over identity.
    pub seq: u64,
    /// The session that issued it. Tie-break only.
    pub origin: String,
}

/// This client's place in the session-agreed sequence.
#[derive(Debug, Clone)]
pub struct NonceSequence {
    session: String,
    next: u64,
}

impl NonceSequence {
    /// Start at the value every participant starts at. "Agreed at session
    /// start" is exactly this: a shared origin of zero and a rule for raising
    /// it, rather than a negotiation nobody could complete while offline.
    pub fn new(session: impl Into<String>) -> Self {
        Self {
            session: session.into(),
            next: 0,
        }
    }

    /// Take the next nonce for something this client is proposing.
    pub fn issue(&mut self) -> Nonce {
        let nonce = Nonce {
            seq: self.next,
            origin: self.session.clone(),
        };
        self.next = self.next.saturating_add(1);
        nonce
    }

    /// Note a nonce seen from someone else, so anything issued afterwards is
    /// ordered after it.
    pub fn observe(&mut self, nonce: &Nonce) {
        self.next = self.next.max(nonce.seq.saturating_add(1));
    }
}

/// The Game Master's answer to one proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Reject,
}

impl Verdict {
    const fn tag(self) -> u8 {
        match self {
            Self::Accept => 0,
            Self::Reject => 1,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Accept),
            1 => Some(Self::Reject),
            _ => None,
        }
    }
}

/// One frame of the adjudication protocol.
///
/// Shares the data channel and the framing style of [`PeerMessage`] — one tag
/// byte, a fixed head, a per-variant tail, total-and-silent decoding — and
/// shares no tag with it, so a frame is unambiguously one protocol or the
/// other and neither decoder can be confused into reading the other's
/// messages.
///
/// # No per-message signatures
///
/// ADR-052 ("The trust model, stated plainly") considered and rejected them.
/// A signature would have let the server verify that a change attributed to
/// player A, arriving over the Game Master's connection, was genuinely A's —
/// which defends against the Game Master, who is the trusted party here and
/// not an adversary. The server's check on reconnection is instead the role
/// check it already has: *does the submitter hold the GM role in this world?*
/// So there is no keypair, no signature format, and no new trust root in this
/// protocol, and adding one later would be undoing a decision rather than
/// hardening an oversight.
#[derive(Debug, Clone, PartialEq)]
pub enum AdjudicationMessage {
    /// "This channel belongs to this user." Sent once when a channel opens.
    ///
    /// A claim, never proof — see [`Adjudication::on_frame`] for what is and
    /// is not concluded from it.
    Hello { user_id: Uuid },
    /// "I would like to move this token."
    Propose {
        nonce: Nonce,
        origin_user: Uuid,
        entity_id: Uuid,
        transform: TokenTransform,
    },
    /// The Game Master's verdict, and only ever the Game Master's (FR-059).
    Adjudicate { nonce: Nonce, verdict: Verdict },
    /// "Everyone apply that one."
    Apply { nonce: Nonce },
}

const TAG_HELLO: u8 = 6;
const TAG_PROPOSE: u8 = 7;
const TAG_ADJUDICATE: u8 = 8;
const TAG_APPLY: u8 = 9;

/// Read a nonce from the head of `tail`, returning it and the rest.
fn take_nonce(tail: &[u8]) -> Option<(Nonce, &[u8])> {
    if tail.len() < 9 {
        return None;
    }
    let seq = u64::from_be_bytes(tail[..8].try_into().ok()?);
    let len = tail[8] as usize;
    let rest = &tail[9..];
    if rest.len() < len {
        return None;
    }
    let origin = std::str::from_utf8(&rest[..len]).ok()?.to_string();
    // An empty origin cannot be a session id, and would make two different
    // clients' nonces compare equal.
    if origin.is_empty() {
        return None;
    }
    Some((Nonce { seq, origin }, &rest[len..]))
}

fn put_nonce(out: &mut Vec<u8>, nonce: &Nonce) {
    out.extend_from_slice(&nonce.seq.to_be_bytes());
    let origin = nonce.origin.as_bytes();
    // Truncated rather than refused: a session id is minted by this client as
    // a uuid, so the length is known and small, and a `u8` length keeps the
    // decoder total.
    let len = origin.len().min(u8::MAX as usize);
    out.push(len as u8);
    out.extend_from_slice(&origin[..len]);
}

fn take_uuid(tail: &[u8]) -> Option<(Uuid, &[u8])> {
    if tail.len() < 16 {
        return None;
    }
    Some((Uuid::from_slice(&tail[..16]).ok()?, &tail[16..]))
}

fn take_f64(tail: &[u8]) -> Option<(f64, &[u8])> {
    if tail.len() < 8 {
        return None;
    }
    Some((f64::from_be_bytes(tail[..8].try_into().ok()?), &tail[8..]))
}

impl AdjudicationMessage {
    /// Render for the wire.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64);
        match self {
            Self::Hello { user_id } => {
                out.push(TAG_HELLO);
                out.extend_from_slice(user_id.as_bytes());
            }
            Self::Propose {
                nonce,
                origin_user,
                entity_id,
                transform,
            } => {
                out.push(TAG_PROPOSE);
                put_nonce(&mut out, nonce);
                out.extend_from_slice(origin_user.as_bytes());
                out.extend_from_slice(entity_id.as_bytes());
                out.push(transform.mask());
                if let Some([x, y]) = transform.position {
                    out.extend_from_slice(&x.to_be_bytes());
                    out.extend_from_slice(&y.to_be_bytes());
                }
                if let Some(rotation) = transform.rotation {
                    out.extend_from_slice(&rotation.to_be_bytes());
                }
                if let Some(scale) = transform.scale {
                    out.extend_from_slice(&scale.to_be_bytes());
                }
            }
            Self::Adjudicate { nonce, verdict } => {
                out.push(TAG_ADJUDICATE);
                put_nonce(&mut out, nonce);
                out.push(verdict.tag());
            }
            Self::Apply { nonce } => {
                out.push(TAG_APPLY);
                put_nonce(&mut out, nonce);
            }
        }
        out
    }

    /// Read a frame, or `None`.
    ///
    /// Total and silent, exactly as [`PeerMessage::decode`] is, and for the
    /// same reason: the channel is written by a party this client does not
    /// trust, so a malformed frame is not an error condition — it is simply
    /// not a message.
    ///
    /// **This is where FR-060 is enforced on the wire.** A `PROPOSE` whose
    /// field mask names anything but position, rotation or scale does not
    /// decode, and neither does one that names nothing. A peer therefore
    /// cannot express a creation, a deletion, or a permission change at all;
    /// there is no branch that could be persuaded to accept one.
    pub fn decode(frame: &[u8]) -> Option<Self> {
        let (tag, tail) = frame.split_first()?;
        match *tag {
            TAG_HELLO => {
                let (user_id, rest) = take_uuid(tail)?;
                rest.is_empty().then_some(Self::Hello { user_id })
            }
            TAG_PROPOSE => {
                let (nonce, tail) = take_nonce(tail)?;
                let (origin_user, tail) = take_uuid(tail)?;
                let (entity_id, tail) = take_uuid(tail)?;
                let (mask, mut tail) = tail.split_first()?;
                // Out of scope, and not by a hair: any bit outside the three
                // fields is a field this protocol does not have, and a mask
                // of zero is a proposal that proposes nothing.
                if *mask == 0 || mask & !FIELD_ALL != 0 {
                    return None;
                }
                let mut transform = TokenTransform::default();
                if mask & FIELD_POSITION != 0 {
                    let (x, rest) = take_f64(tail)?;
                    let (y, rest) = take_f64(rest)?;
                    transform.position = Some([x, y]);
                    tail = rest;
                }
                if mask & FIELD_ROTATION != 0 {
                    let (rotation, rest) = take_f64(tail)?;
                    transform.rotation = Some(rotation);
                    tail = rest;
                }
                if mask & FIELD_SCALE != 0 {
                    let (scale, rest) = take_f64(tail)?;
                    transform.scale = Some(scale);
                    tail = rest;
                }
                tail.is_empty().then_some(Self::Propose {
                    nonce,
                    origin_user,
                    entity_id,
                    transform,
                })
            }
            TAG_ADJUDICATE => {
                let (nonce, tail) = take_nonce(tail)?;
                let (verdict, rest) = tail.split_first()?;
                rest.is_empty()
                    .then_some(())
                    .and(Verdict::from_tag(*verdict))
                    .map(|verdict| Self::Adjudicate { nonce, verdict })
            }
            TAG_APPLY => {
                let (nonce, tail) = take_nonce(tail)?;
                tail.is_empty().then_some(Self::Apply { nonce })
            }
            _ => None,
        }
    }
}

/// Why peer-adjudicated play stopped.
///
/// Every variant means the same thing to the user — back to plain offline,
/// with edits queued in the outbox and replayed on reconnection — and they
/// are kept apart only so the indicator can say something true about which
/// it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjudicationEnd {
    /// A participant became unreachable (FR-058). **Both halves of a
    /// partition see this**, which is the point: neither side wins, because a
    /// side that kept playing would be a second history to reconcile.
    PeerLost,
    /// The Game Master became unreachable (FR-059). No election follows —
    /// promoting a replacement would mean two adjudicators across one
    /// session and no single chain of authority.
    GameMasterLost,
    /// The server came back. Everything adjudicated is now submitted for
    /// confirmation and may be rejected (FR-062).
    ServerReturned,
    /// The world closed, or peer transfer was turned off.
    Ended,
}

/// Why one frame changed nothing.
///
/// Returned rather than logged, so a test can name the refusal it is checking
/// for instead of asserting on an absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Adjudicated play is not running: it never started, every participant
    /// is not reachable, the Game Master is not known, or it has stopped.
    NotAdjudicating,
    /// An `ADJUDICATE` from somebody who is not the Game Master (FR-059).
    NotTheGameMaster,
    /// A proposal attributed to a user other than the one whose channel it
    /// arrived on, from a peer who is not the Game Master. The peer-side
    /// mirror of FR-061a; the server checks it again on submission, which is
    /// the check that actually binds.
    NotYours,
    /// A verdict or an apply for a nonce this client never saw proposed.
    Unknown,
    /// Ordered before something already applied to that token. Not an error:
    /// it is the nonce sequence doing its job.
    Superseded,
    /// A transform that changes nothing. Reachable only through
    /// [`TokenTransform::default`]; a nonce spent and an `APPLY` broadcast for
    /// no change is worse than a refusal.
    NothingProposed,
}

/// One change this client applied while server-isolated.
///
/// **Provisional, and the type says so by carrying everything the server
/// needs to re-authorize it** — who it is attributed to, what it touched,
/// and where it sits in the order — and nothing that could be mistaken for a
/// confirmation. On reconnection the Game Master's client submits these over
/// its own authenticated session and the server confirms or rejects each one
/// (FR-062). Nothing here is the record of anything.
#[derive(Debug, Clone, PartialEq)]
pub struct AdjudicatedChange {
    pub nonce: Nonce,
    pub origin_user: Uuid,
    pub entity_id: Uuid,
    pub transform: TokenTransform,
}

impl AdjudicatedChange {
    /// The submission payload, for the reconcile mutation.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "nonceSeq": self.nonce.seq,
            "nonceOrigin": self.nonce.origin,
            "originUser": self.origin_user.to_string(),
            "entityId": self.entity_id.to_string(),
            "transform": self.transform.to_json(),
        })
    }
}

/// What one frame did to the session.
#[derive(Debug, Clone, PartialEq)]
pub enum AdjudicationStep {
    /// Nothing to do, and nothing to say.
    Ignore,
    /// Frames to send to every peer, and the change the sender applied
    /// locally at the same time. The Game Master both decides and plays, so
    /// its own handling of a `PROPOSE` produces both at once.
    Broadcast {
        frames: Vec<Vec<u8>>,
        applied: Option<Box<AdjudicatedChange>>,
    },
    /// Apply this to the local scene. Provisional.
    Applied(Box<AdjudicatedChange>),
    /// Deliberately did nothing, for a reason worth naming.
    Refused(Refusal),
}

/// Peer-adjudicated play, for as long as the conditions hold.
///
/// # The three conditions, and why they are all of them
///
/// This runs only while the server is unreachable, **every** participant is
/// reachable, and the Game Master is among them (FR-057 to FR-059). The
/// second is the one that looks excessive and is not.
///
/// A quorum would let two subsets each satisfy a majority and both make
/// progress — two irreconcilable histories, and no rule that could merge them
/// afterwards without destroying somebody's work. Requiring everyone means at
/// most one group is ever playing, so there is never a second history to
/// merge. It stops play more often; it never produces state that cannot be
/// reconciled. Two disjoint halves both stop, and neither wins.
///
/// The third has the same shape. If the Game Master's client is not there,
/// play stops rather than electing a replacement, because a replacement is a
/// second adjudicator in one session and the end of a single chain of
/// authority.
///
/// # What "unreachable" means here
///
/// Nothing in this type decides whether the server is up. That is the
/// heartbeat's answer (`apps/web/src/engine/world/sync/heartbeat.ts`), the
/// one liveness signal this feature has, and it is passed in. A second
/// opinion about connectivity is exactly how a client ends up queueing edits
/// during an idle moment while reporting a healthy connection it does not
/// have.
#[derive(Debug)]
pub struct Adjudication {
    self_session: String,
    self_user: Uuid,
    /// Who holds the Game Master role, according to the **server**, learned
    /// while still connected. The one piece of authority in here that a peer
    /// did not supply.
    gm_user: Uuid,
    /// Everyone who must stay reachable. Fixed when play began: a session
    /// that joins afterwards cannot join adjudicated play, because the
    /// participants agreed an order at the start and a newcomer has not seen
    /// it.
    roster: BTreeSet<String>,
    reachable: BTreeSet<String>,
    /// Which session each peer says it belongs to. A claim; see `on_frame`.
    users: BTreeMap<String, Uuid>,
    nonces: NonceSequence,
    /// Proposals seen and not yet applied, by nonce.
    pending: BTreeMap<Nonce, AdjudicatedChange>,
    /// The last nonce applied to each token. What makes ordering *mean*
    /// something rather than merely exist.
    applied: BTreeMap<Uuid, Nonce>,
    log: Vec<AdjudicatedChange>,
    stopped: Option<AdjudicationEnd>,
    server_unreachable: bool,
}

impl Adjudication {
    /// Begin, given who is here.
    ///
    /// `roster` is every other participant that was reachable at the moment
    /// the server was lost — the session's membership, agreed while there was
    /// still a server to agree it with. `None` when there is nobody to
    /// adjudicate with: a client alone is not server-isolated, it is offline,
    /// and the outbox already handles that case correctly.
    pub fn begin(
        self_session: impl Into<String>,
        self_user: Uuid,
        gm_user: Uuid,
        roster: impl IntoIterator<Item = String>,
    ) -> Option<Self> {
        let self_session = self_session.into();
        let roster: BTreeSet<String> = roster
            .into_iter()
            .filter(|session| *session != self_session)
            .collect();
        if roster.is_empty() {
            return None;
        }
        let mut users = BTreeMap::new();
        users.insert(self_session.clone(), self_user);
        Some(Self {
            nonces: NonceSequence::new(self_session.clone()),
            self_session,
            self_user,
            gm_user,
            reachable: roster.clone(),
            roster,
            users,
            pending: BTreeMap::new(),
            applied: BTreeMap::new(),
            log: Vec::new(),
            stopped: None,
            server_unreachable: true,
        })
    }

    /// Whether this client is the adjudicating authority.
    pub fn is_game_master(&self) -> bool {
        self.self_user == self.gm_user
    }

    /// The session the Game Master is speaking on, if one is known.
    ///
    /// Known immediately when this client *is* the Game Master, and otherwise
    /// only once a peer has said which user it belongs to and that user is
    /// the one the server named.
    pub fn gm_session(&self) -> Option<&str> {
        if self.is_game_master() {
            return Some(&self.self_session);
        }
        self.users
            .iter()
            .find(|(session, user)| **user == self.gm_user && self.reachable.contains(*session))
            .map(|(session, _)| session.as_str())
    }

    /// Whether peer-adjudicated play is running right now.
    ///
    /// All three conditions, asked every time rather than cached, because the
    /// cost of getting this wrong is a change adjudicated by a table that is
    /// no longer all there.
    pub fn is_adjudicating(&self) -> bool {
        self.stopped.is_none()
            && self.server_unreachable
            && self.roster.iter().all(|s| self.reachable.contains(s))
            && self.gm_session().is_some()
    }

    /// Why it stopped, if it has.
    pub fn ended(&self) -> Option<AdjudicationEnd> {
        self.stopped
    }

    /// The frame announcing who this client is, to send when a channel opens.
    pub fn hello(&self) -> Vec<u8> {
        AdjudicationMessage::Hello {
            user_id: self.self_user,
        }
        .encode()
    }

    /// A participant became unreachable (FR-058, FR-059).
    ///
    /// **Immediate, and with no grace period.** A peer that has gone quiet
    /// might come back in a second, and waiting to see is how a partition
    /// gets a window in which both halves are still playing. Losing anyone
    /// ends adjudicated play at once and drops this client to plain offline,
    /// where the outbox takes over — which is a path that already works.
    pub fn peer_lost(&mut self, session: &str) -> Option<AdjudicationEnd> {
        let was_gm = self.gm_session() == Some(session);
        self.reachable.remove(session);
        if !self.roster.contains(session) {
            // Somebody who was never part of adjudicated play leaving is not
            // an event: they could not have been proposing.
            return None;
        }
        let end = if was_gm {
            AdjudicationEnd::GameMasterLost
        } else {
            AdjudicationEnd::PeerLost
        };
        self.stop(end)
    }

    /// The server is reachable again (FR-062). Everything adjudicated now
    /// goes to it for confirmation.
    pub fn server_returned(&mut self) -> Option<AdjudicationEnd> {
        self.server_unreachable = false;
        self.stop(AdjudicationEnd::ServerReturned)
    }

    /// The world closed, or peer transfer was turned off.
    pub fn end(&mut self) -> Option<AdjudicationEnd> {
        self.stop(AdjudicationEnd::Ended)
    }

    fn stop(&mut self, end: AdjudicationEnd) -> Option<AdjudicationEnd> {
        if self.stopped.is_some() {
            return None;
        }
        // Pending proposals are dropped, and applied ones are not. A proposal
        // nobody adjudicated never happened; a change that was applied is on
        // screens and owed a submission.
        self.pending.clear();
        self.stopped = Some(end);
        Some(end)
    }

    /// Propose a move of this client's own.
    ///
    /// [`AdjudicationStep::Refused`] when adjudicated play is not running,
    /// which is the same answer as "queue it in the outbox instead" — the
    /// caller has one fall-back and does not need to know which condition
    /// failed.
    ///
    /// On the Game Master's client this both proposes and decides, because
    /// there is nobody else to ask; the frames it returns are the same ones a
    /// player's proposal would have produced from the Game Master, so the
    /// other participants cannot tell the two paths apart and neither can
    /// this file's tests.
    pub fn propose(&mut self, entity_id: Uuid, transform: TokenTransform) -> AdjudicationStep {
        if !self.is_adjudicating() {
            return AdjudicationStep::Refused(Refusal::NotAdjudicating);
        }
        // Not reachable through the three constructors, which each set a
        // field; here for the day someone builds one from `default()`.
        if transform.is_empty() {
            return AdjudicationStep::Refused(Refusal::NothingProposed);
        }
        let nonce = self.nonces.issue();
        let change = AdjudicatedChange {
            nonce: nonce.clone(),
            origin_user: self.self_user,
            entity_id,
            transform,
        };
        let propose = AdjudicationMessage::Propose {
            nonce: nonce.clone(),
            origin_user: change.origin_user,
            entity_id,
            transform,
        }
        .encode();
        self.pending.insert(nonce.clone(), change);

        if !self.is_game_master() {
            return AdjudicationStep::Broadcast {
                frames: vec![propose],
                applied: None,
            };
        }
        let mut frames = vec![propose];
        frames.push(
            AdjudicationMessage::Adjudicate {
                nonce: nonce.clone(),
                verdict: Verdict::Accept,
            }
            .encode(),
        );
        frames.push(
            AdjudicationMessage::Apply {
                nonce: nonce.clone(),
            }
            .encode(),
        );
        let applied = match self.apply(&nonce) {
            AdjudicationStep::Applied(change) => Some(change),
            _ => None,
        };
        AdjudicationStep::Broadcast { frames, applied }
    }

    /// Feed one raw frame from `from`.
    ///
    /// A frame that does not decode is [`AdjudicationStep::Ignore`] and not a
    /// refusal: the channel carries content transfer as well, and everything
    /// else a peer feels like sending.
    pub fn on_frame(&mut self, from: &str, frame: &[u8]) -> AdjudicationStep {
        match AdjudicationMessage::decode(frame) {
            Some(message) => self.on_message(from, message),
            None => AdjudicationStep::Ignore,
        }
    }

    /// Feed one decoded message from `from`.
    pub fn on_message(&mut self, from: &str, message: AdjudicationMessage) -> AdjudicationStep {
        // A `HELLO` is accepted before the adjudicating check, because it is
        // what *makes* adjudication possible: until a peer says who it is,
        // nobody but the Game Master's own client knows the Game Master is
        // here.
        if let AdjudicationMessage::Hello { user_id } = message {
            if self.roster.contains(from) {
                // A claim, and treated as one. The only thing concluded from
                // it is which session to accept verdicts on, and the claim is
                // only believed where it matches the user the **server**
                // named as Game Master while this client was still connected.
                // A peer that lies here can adjudicate a session whose every
                // outcome is provisional, and still cannot submit one: the
                // server checks the submitter's role, and that check is the
                // one that binds (FR-061, ADR-052's trust model).
                self.users.insert(from.to_string(), user_id);
            }
            return AdjudicationStep::Ignore;
        }

        if !self.is_adjudicating() || !self.roster.contains(from) {
            return AdjudicationStep::Refused(Refusal::NotAdjudicating);
        }

        match message {
            AdjudicationMessage::Hello { .. } => AdjudicationStep::Ignore,

            AdjudicationMessage::Propose {
                nonce,
                origin_user,
                entity_id,
                transform,
            } => {
                // Every nonce seen raises this client's own, so anything it
                // proposes next is ordered after what it has already heard.
                // Done for every participant, not only the Game Master, so
                // the order is one everybody computes the same way.
                self.nonces.observe(&nonce);
                if nonce.origin != from {
                    // A nonce claiming to come from another session would let
                    // one peer spend another's sequence and order its own
                    // proposals ahead of theirs.
                    return AdjudicationStep::Refused(Refusal::NotYours);
                }
                let claimed = self.users.get(from).copied();
                let speaking_for_self = claimed == Some(origin_user);
                let is_gm = self.gm_session() == Some(from);
                if !speaking_for_self && !is_gm {
                    // The peer-side mirror of FR-061a. Only the Game Master
                    // may act on another user's behalf, which is a legitimate
                    // exercise of table authority (FR-061b) and not something
                    // this code tries to prevent.
                    return AdjudicationStep::Refused(Refusal::NotYours);
                }
                let change = AdjudicatedChange {
                    nonce: nonce.clone(),
                    origin_user,
                    entity_id,
                    transform,
                };
                self.pending.insert(nonce.clone(), change);

                if !self.is_game_master() {
                    // A player records the proposal and waits. Applying it
                    // now — optimistically, before the verdict — is how two
                    // clients end up in different states over a proposal the
                    // Game Master rejected.
                    return AdjudicationStep::Ignore;
                }
                // The Game Master's client is the only one that decides
                // (FR-059). The verdict is a table decision and not a
                // security one: the scope was already fixed by the type, and
                // authorization is the server's on reconnection.
                let frames = vec![
                    AdjudicationMessage::Adjudicate {
                        nonce: nonce.clone(),
                        verdict: Verdict::Accept,
                    }
                    .encode(),
                    AdjudicationMessage::Apply {
                        nonce: nonce.clone(),
                    }
                    .encode(),
                ];
                match self.apply(&nonce) {
                    AdjudicationStep::Applied(change) => AdjudicationStep::Broadcast {
                        frames,
                        applied: Some(change),
                    },
                    // Ordered behind something already applied to that token:
                    // there is nothing to broadcast, because there is nothing
                    // for anybody to apply.
                    other => other,
                }
            }

            AdjudicationMessage::Adjudicate { nonce, verdict } => {
                self.nonces.observe(&nonce);
                if self.gm_session() != Some(from) {
                    // FR-059. A peer is not promoted when the Game Master
                    // leaves, and it certainly is not promoted while they are
                    // still here.
                    return AdjudicationStep::Refused(Refusal::NotTheGameMaster);
                }
                if verdict == Verdict::Reject {
                    self.pending.remove(&nonce);
                    return AdjudicationStep::Ignore;
                }
                if self.pending.contains_key(&nonce) {
                    AdjudicationStep::Ignore
                } else {
                    AdjudicationStep::Refused(Refusal::Unknown)
                }
            }

            AdjudicationMessage::Apply { nonce } => {
                self.nonces.observe(&nonce);
                if self.gm_session() != Some(from) {
                    return AdjudicationStep::Refused(Refusal::NotTheGameMaster);
                }
                self.apply(&nonce)
            }
        }
    }

    /// Apply one accepted proposal, if the order allows.
    fn apply(&mut self, nonce: &Nonce) -> AdjudicationStep {
        let Some(change) = self.pending.remove(nonce) else {
            return AdjudicationStep::Refused(Refusal::Unknown);
        };
        // **The whole point of the nonce.** Two proposals for one token are
        // ordered by the sequence, not by which frame happened to arrive
        // first — so a slow link cannot decide the outcome, and neither can a
        // fast one. Nothing consults a clock, here or anywhere else in this
        // file.
        if let Some(previous) = self.applied.get(&change.entity_id)
            && previous >= nonce
        {
            return AdjudicationStep::Refused(Refusal::Superseded);
        }
        self.applied.insert(change.entity_id, nonce.clone());
        self.log.push(change.clone());
        AdjudicationStep::Applied(Box::new(change))
    }

    /// Everything applied while server-isolated, oldest first.
    pub fn submissions(&self) -> &[AdjudicatedChange] {
        &self.log
    }

    /// Take the log for submission, leaving it empty.
    ///
    /// Called on the Game Master's client when the server comes back: it
    /// submits over its own authenticated session, and the server verifies
    /// the submitter's **role** rather than any attestation travelling with
    /// the changes (FR-061).
    pub fn take_submissions(&mut self) -> Vec<AdjudicatedChange> {
        std::mem::take(&mut self.log)
    }

    /// The payload for the reconcile mutation.
    pub fn submissions_json(&self) -> String {
        serde_json::Value::Array(self.log.iter().map(AdjudicatedChange::to_json).collect())
            .to_string()
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    BlobProvider, activity, adjudication_active, adjudication_end, adjudication_propose,
    adjudication_server_returned, adjudication_submissions, begin_adjudication, connect_to,
    disable, enable, is_active, membership_lost, note_stored, on_signal, peer_count, set_held,
    set_plan, set_provider, try_fetch,
};

/// The `RTCPeerConnection`/`RTCDataChannel` glue, and nothing else.
///
/// Every rule this module has is above; this half only moves frames. That
/// split is why "verify before storing" is checkable under `cargo test` on a
/// machine with no browser on it, and it is the same split the rest of the
/// crate uses.
///
/// # No STUN, no TURN
///
/// Deliberately. Every participant reaches the same server over the same
/// network path already, and host ICE candidates are enough for the cases
/// this feature exists for — several players in one household, or on one
/// office network, pulling the same map. A STUN server would be a third
/// party learning who is playing with whom, for a marginal increase in the
/// number of peer pairs that connect, and FR-052/FR-054 rule out paying that
/// price. A pair that cannot connect falls back to the server, like every
/// other failure here.
#[cfg(target_arch = "wasm32")]
mod wasm {
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
    pub type BlobProvider =
        Rc<dyn Fn(Fingerprint) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>>>>>;

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

    // -----------------------------------------------------------------
    // Signaling
    // -----------------------------------------------------------------

    fn signal(to: &str, payload: serde_json::Value) {
        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow().as_ref() {
                let _ = fabric.send_signal.call2(
                    &JsValue::NULL,
                    &JsValue::from_str(to),
                    &JsValue::from_str(&payload.to_string()),
                );
            }
        });
    }

    /// Offer a connection to one peer.
    ///
    /// **The newcomer always initiates.** A client joining queries the roster
    /// and offers to each name on it; nobody offers to a newcomer. That makes
    /// glare — two peers offering each other at once — structurally
    /// impossible rather than something to resolve, which is worth more than
    /// the connection it occasionally costs when two clients join together.
    pub async fn connect_to(peer: String) {
        let already = FABRIC.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_some_and(|f| f.links.contains_key(&peer))
        });
        if already {
            return;
        }
        let Some(link) = new_link(&peer, true) else {
            return;
        };

        let offer = match JsFuture::from(link.connection.create_offer()).await {
            Ok(offer) => offer,
            Err(_) => return,
        };
        let Some(sdp) = sdp_of(&offer) else { return };
        let init = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Offer);
        init.set_sdp(&sdp);
        if JsFuture::from(link.connection.set_local_description(&init))
            .await
            .is_err()
        {
            return;
        }
        signal(&peer, serde_json::json!({ "kind": "offer", "sdp": sdp }));
    }

    /// A signal arrived for us, relayed by the server.
    ///
    /// The server never interprets these and neither does anything above this
    /// function: an unparseable payload is dropped in silence, exactly as an
    /// unparseable frame is, because the sender is not trusted and a malformed
    /// message is not an error condition.
    pub async fn on_signal(from: String, payload: String) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) else {
            return;
        };
        match value.get("kind").and_then(serde_json::Value::as_str) {
            Some("offer") => {
                let Some(sdp) = value.get("sdp").and_then(serde_json::Value::as_str) else {
                    return;
                };
                // The answerer never opens a channel of its own; it waits for
                // `ondatachannel`. Two channels on one connection would each
                // work and would double every count the indicator shows.
                let Some(link) = new_link(&from, false) else {
                    return;
                };
                let remote = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Offer);
                remote.set_sdp(sdp);
                if JsFuture::from(link.connection.set_remote_description(&remote))
                    .await
                    .is_err()
                {
                    return;
                }
                let Ok(answer) = JsFuture::from(link.connection.create_answer()).await else {
                    return;
                };
                let Some(answer_sdp) = sdp_of(&answer) else {
                    return;
                };
                let local = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Answer);
                local.set_sdp(&answer_sdp);
                if JsFuture::from(link.connection.set_local_description(&local))
                    .await
                    .is_err()
                {
                    return;
                }
                signal(
                    &from,
                    serde_json::json!({ "kind": "answer", "sdp": answer_sdp }),
                );
            }
            Some("answer") => {
                let (Some(link), Some(sdp)) = (
                    link_for(&from),
                    value.get("sdp").and_then(serde_json::Value::as_str),
                ) else {
                    return;
                };
                let remote = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Answer);
                remote.set_sdp(sdp);
                let _ = JsFuture::from(link.connection.set_remote_description(&remote)).await;
            }
            Some("candidate") => {
                let (Some(link), Some(candidate)) = (
                    link_for(&from),
                    value.get("candidate").and_then(serde_json::Value::as_str),
                ) else {
                    return;
                };
                let init = web_sys::RtcIceCandidateInit::new(candidate);
                init.set_sdp_mid(value.get("sdpMid").and_then(serde_json::Value::as_str));
                init.set_sdp_m_line_index(
                    value
                        .get("sdpMLineIndex")
                        .and_then(serde_json::Value::as_u64)
                        .map(|i| i as u16),
                );
                if let Ok(candidate) = web_sys::RtcIceCandidate::new(&init) {
                    let _ = JsFuture::from(
                        link.connection
                            .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)),
                    )
                    .await;
                }
            }
            _ => {}
        }
    }

    fn sdp_of(description: &JsValue) -> Option<String> {
        Reflect::get(description, &JsValue::from_str("sdp"))
            .ok()?
            .as_string()
    }

    fn link_for(peer: &str) -> Option<Rc<PeerLink>> {
        FABRIC.with(|slot| slot.borrow().as_ref()?.links.get(peer).cloned())
    }

    fn new_link(peer: &str, initiator: bool) -> Option<Rc<PeerLink>> {
        let connection = web_sys::RtcPeerConnection::new().ok()?;
        let link = Rc::new(PeerLink {
            session: peer.to_string(),
            connection,
            channel: RefCell::new(None),
            download: RefCell::new(None),
            waiter: RefCell::new(None),
            outcome: RefCell::new(None),
            ticker: RefCell::new(None),
            retained: RefCell::new(Vec::new()),
        });

        let to = peer.to_string();
        let on_ice = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            let Ok(candidate) = Reflect::get(&event, &JsValue::from_str("candidate")) else {
                return;
            };
            // A null candidate is "gathering finished", not a candidate.
            if candidate.is_null() || candidate.is_undefined() {
                return;
            }
            let text = Reflect::get(&candidate, &JsValue::from_str("candidate"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let mid = Reflect::get(&candidate, &JsValue::from_str("sdpMid"))
                .ok()
                .and_then(|v| v.as_string());
            let index = Reflect::get(&candidate, &JsValue::from_str("sdpMLineIndex"))
                .ok()
                .and_then(|v| v.as_f64());
            signal(
                &to,
                serde_json::json!({
                    "kind": "candidate",
                    "candidate": text,
                    "sdpMid": mid,
                    "sdpMLineIndex": index,
                }),
            );
        });
        link.connection
            .set_onicecandidate(Some(on_ice.as_ref().unchecked_ref()));
        link.retained.borrow_mut().push(on_ice);

        if initiator {
            let channel = link.connection.create_data_channel(CHANNEL_LABEL);
            attach_channel(&link, channel);
        } else {
            let waiting = link.clone();
            let on_channel = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let Ok(channel) = Reflect::get(&event, &JsValue::from_str("channel")) else {
                    return;
                };
                if let Ok(channel) = channel.dyn_into::<web_sys::RtcDataChannel>() {
                    attach_channel(&waiting, channel);
                }
            });
            link.connection
                .set_ondatachannel(Some(on_channel.as_ref().unchecked_ref()));
            link.retained.borrow_mut().push(on_channel);
        }

        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.links.insert(peer.to_string(), link.clone());
            }
        });
        Some(link)
    }

    fn attach_channel(link: &Rc<PeerLink>, channel: web_sys::RtcDataChannel) {
        channel.set_binary_type(web_sys::RtcDataChannelType::Arraybuffer);

        let receiving = link.clone();
        let on_message = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            let Ok(data) = Reflect::get(&event, &JsValue::from_str("data")) else {
                return;
            };
            if !data.is_object() {
                return;
            }
            let frame = Uint8Array::new(&data).to_vec();
            on_frame(&receiving, &frame);
        });
        channel.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        // Departures are noticed here and nowhere else — there is no
        // join/leave push in the signaling contract, by design.
        let closing = link.clone();
        let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
            peer_departed(&closing);
        });
        channel.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let erroring = link.clone();
        let on_error = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
            peer_departed(&erroring);
        });
        channel.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        link.retained.borrow_mut().push(on_message);
        link.retained.borrow_mut().push(on_close);
        link.retained.borrow_mut().push(on_error);
        *link.channel.borrow_mut() = Some(channel);
    }

    /// A peer went away. Any transfer in progress is abandoned whole.
    fn peer_departed(link: &Rc<PeerLink>) {
        let step = link
            .download
            .borrow_mut()
            .as_mut()
            .map(PeerDownload::peer_gone);
        if let Some(DownloadStep::FallBack(reason)) = step {
            settle(link, None, Some(reason));
        }
        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.server.peer_gone(&link.session);
                fabric.links.remove(&link.session);
                // FR-058/FR-059, and this is the only place a departure is
                // noticed: there is no join/leave push in the signaling
                // contract, so a closed channel *is* the event. Ending here
                // rather than on a timer is what makes "immediately" true.
                if let Some(adjudication) = fabric.adjudication.as_mut() {
                    adjudication.peer_lost(&link.session);
                }
            }
        });
    }

    fn on_frame(link: &Rc<PeerLink>, frame: &[u8]) {
        // The two protocols share one channel and share no tag, so this is a
        // sort rather than a guess: an adjudication frame cannot decode as a
        // transfer frame, and a transfer frame cannot decode as an
        // adjudication one.
        if let Some(message) = AdjudicationMessage::decode(frame) {
            let step = FABRIC.with(|slot| {
                slot.borrow_mut()
                    .as_mut()
                    .and_then(|fabric| fabric.adjudication.as_mut())
                    .map(|adjudication| adjudication.on_message(&link.session, message))
            });
            if let Some(step) = step {
                handle_adjudication(step);
            }
            return;
        }

        let Some(message) = PeerMessage::decode(frame) else {
            return;
        };

        // A request is the serving side and has nothing to do with any
        // download in flight; keeping them apart here is what stops a peer
        // from steering our own transfer by answering it with a question.
        if let PeerMessage::Request { fingerprint } = message {
            let serving = link.clone();
            wasm_bindgen_futures::spawn_local(async move { serve(serving, fingerprint).await });
            return;
        }

        let step = link
            .download
            .borrow_mut()
            .as_mut()
            .map_or(DownloadStep::Ignore, |download| {
                download.on_message(message, now_ms())
            });

        match step {
            DownloadStep::Verified(bytes) => settle(link, Some(bytes), None),
            DownloadStep::FallBack(reason) => settle(link, None, Some(reason)),
            DownloadStep::Continue | DownloadStep::Ignore => {}
        }
    }

    /// End a transfer: record it, hand the result to whoever is awaiting.
    ///
    /// `bytes` is `Some` only for [`DownloadStep::Verified`], so this is the
    /// last place the "no unverified bytes" property has to hold and the only
    /// way any bytes reach a caller.
    fn settle(link: &Rc<PeerLink>, bytes: Option<Vec<u8>>, reason: Option<Fallback>) {
        link.download.borrow_mut().take();
        link.stop_ticking();

        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                if let Some(reason) = reason {
                    fabric.trust.record(&link.session, reason);
                    if reason == Fallback::VerificationFailed {
                        fabric.activity.verification_failures =
                            fabric.activity.verification_failures.saturating_add(1);
                    }
                }
                if let Some(bytes) = bytes.as_ref() {
                    fabric.activity.bytes_from_peers = fabric
                        .activity
                        .bytes_from_peers
                        .saturating_add(bytes.len() as u64);
                }
            }
        });

        // A peer that failed verification is not asked again this session,
        // and the channel goes with it: there is nothing else we want from
        // someone who does not send what they were asked for.
        if reason.is_some_and(Fallback::distrusts_peer) {
            link.shut_down();
            FABRIC.with(|slot| {
                if let Some(fabric) = slot.borrow_mut().as_mut() {
                    fabric.links.remove(&link.session);
                }
            });
        }

        *link.outcome.borrow_mut() = bytes;
        let waiter = link.waiter.borrow_mut().take();
        if let Some(waiter) = waiter {
            let _ = waiter.call0(&JsValue::NULL);
        }
    }

    // -----------------------------------------------------------------
    // The requester
    // -----------------------------------------------------------------

    /// Try to get one fingerprint from a peer.
    ///
    /// `None` means "ask the server", and it is the answer to every one of:
    /// peer transfer is off, the fingerprint is not in this client's plan, no
    /// peer is connected, every peer is busy or distrusted, the peer declined,
    /// the peer stalled, the peer hung up, the peer sent something that did
    /// not verify. **The caller cannot tell those apart, and must not need
    /// to** — that indistinguishability is SC-013.
    pub async fn try_fetch(fingerprint: Fingerprint) -> Option<Vec<u8>> {
        // T089's gate. `request` is the only constructor of a `PeerRequest`
        // anywhere, and it answers `None` for anything the server's plan does
        // not list.
        let request = FABRIC.with(|slot| {
            let borrowed = slot.borrow();
            let fabric = borrowed.as_ref()?;
            fabric.scope.request(&fingerprint)
        })?;

        let link = pick_peer()?;
        let (download, frame) = PeerDownload::begin(request, now_ms());
        *link.download.borrow_mut() = Some(download);
        *link.outcome.borrow_mut() = None;

        // The promise is armed *before* the request goes out, so a peer that
        // answers synchronously cannot resolve into a slot that is not there.
        let waiting = link.clone();
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            *waiting.waiter.borrow_mut() = Some(resolve);
        });
        link.start_ticking();

        if link.send(&frame).is_err() {
            settle(&link, None, Some(Fallback::PeerGone));
            return None;
        }

        let _ = JsFuture::from(promise).await;
        link.outcome.borrow_mut().take()
    }

    /// The peer to ask next.
    ///
    /// Trusted, connected, and not already carrying a transfer. Round-robin
    /// would be better with many peers and is not worth the state at the
    /// scale this runs at — a table is a handful of people, not a swarm.
    fn pick_peer() -> Option<Rc<PeerLink>> {
        FABRIC.with(|slot| {
            let borrowed = slot.borrow();
            let fabric = borrowed.as_ref()?;
            fabric
                .links
                .values()
                .find(|link| {
                    link.is_open()
                        && link.download.borrow().is_none()
                        && fabric.trust.trusts(&link.session)
                })
                .cloned()
        })
    }

    // -----------------------------------------------------------------
    // The server
    // -----------------------------------------------------------------

    async fn serve(link: Rc<PeerLink>, fingerprint: Fingerprint) {
        let decision = FABRIC.with(|slot| {
            slot.borrow_mut().as_mut().map(|fabric| {
                fabric
                    .server
                    .on_request(&link.session, &fingerprint, now_ms())
            })
        });

        let decision = match decision {
            Some(decision) => decision,
            // Peer transfer was switched off between the request arriving and
            // this task running. Say nothing and close: an answer would be a
            // statement about content we are no longer entitled to discuss.
            None => {
                link.shut_down();
                return;
            }
        };

        match decision {
            ServeDecision::DropChannel => {
                link.shut_down();
                return;
            }
            ServeDecision::Decline(reason) => {
                link.decline(fingerprint, reason);
                return;
            }
            ServeDecision::Serve => {}
        }

        let provider = FABRIC.with(|slot| {
            slot.borrow()
                .as_ref()
                .and_then(|fabric| fabric.provider.clone())
        });
        let bytes = match provider {
            Some(provider) => provider(fingerprint).await,
            None => None,
        };

        // Held a moment ago, unreadable now — evicted underneath us, or the
        // key is gone. `DECLINE` rather than send something else: the
        // contract forbids sending bytes that do not hash to what was asked
        // for, and this is the branch where that temptation would live.
        let Some(bytes) = bytes else {
            FABRIC.with(|slot| {
                if let Some(fabric) = slot.borrow_mut().as_mut() {
                    fabric.server.served(&link.session, 0);
                    fabric.server.forgets(&fingerprint);
                }
            });
            link.decline(fingerprint, DeclineReason::NotHeld);
            return;
        };

        // Belt and braces over `read_blob`, which has already verified. The
        // cost is one hash of bytes we are about to spend far more bandwidth
        // on, and it makes "never send bytes that do not hash to the
        // requested fingerprint" true of this function in isolation rather
        // than true of a chain of callers.
        if fingerprint::verify(&bytes, &fingerprint).is_err() {
            FABRIC.with(|slot| {
                if let Some(fabric) = slot.borrow_mut().as_mut() {
                    fabric.server.served(&link.session, 0);
                    fabric.server.forgets(&fingerprint);
                }
            });
            link.decline(fingerprint, DeclineReason::NotHeld);
            return;
        }

        let mut sent = 0u64;
        for frame in serve_frames(&fingerprint, &bytes) {
            // Checked between every frame, not once at the start. FR-050
            // says serving stops on losing membership, and a large map is
            // seconds of frames — stopping only at the end of one would mean
            // finishing the delivery of content we have just lost.
            let stop = FABRIC.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .is_none_or(|fabric| fabric.server.must_abort())
            });
            if stop {
                break;
            }
            let encoded = frame.encode();
            if link.send(&encoded).is_err() {
                break;
            }
            sent = sent.saturating_add(encoded.len() as u64);
        }

        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.server.served(&link.session, sent);
            }
        });
    }

    impl PeerLink {
        fn is_open(&self) -> bool {
            self.channel
                .borrow()
                .as_ref()
                .is_some_and(|channel| channel.ready_state() == web_sys::RtcDataChannelState::Open)
        }

        fn send(&self, frame: &[u8]) -> Result<(), ()> {
            let channel = self.channel.borrow();
            let Some(channel) = channel.as_ref() else {
                return Err(());
            };
            if channel.ready_state() != web_sys::RtcDataChannelState::Open {
                return Err(());
            }
            channel.send_with_u8_array(frame).map_err(|_| ())
        }

        fn decline(&self, fingerprint: Fingerprint, reason: DeclineReason) {
            let _ = self.send(
                &PeerMessage::Decline {
                    fingerprint,
                    reason,
                }
                .encode(),
            );
        }

        /// Poll the download's own deadlines. The timer only exists while a
        /// transfer does, so an idle page schedules nothing.
        fn start_ticking(self: &Rc<Self>) {
            self.stop_ticking();
            let Some(set_interval) = global_fn("setInterval") else {
                return;
            };
            let ticking = self.clone();
            let tick = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
                let step = ticking
                    .download
                    .borrow_mut()
                    .as_mut()
                    .map(|download| download.tick(now_ms()));
                if let Some(DownloadStep::FallBack(reason)) = step {
                    settle(&ticking, None, Some(reason));
                }
            });
            let handle = set_interval.call2(
                &JsValue::NULL,
                tick.as_ref().unchecked_ref(),
                &JsValue::from_f64(f64::from(TICK_MS)),
            );
            if let Ok(handle) = handle
                && let Some(handle) = handle.as_f64()
            {
                *self.ticker.borrow_mut() = Some(handle as i32);
            }
            self.retained.borrow_mut().push(tick);
        }

        fn stop_ticking(&self) {
            if let Some(handle) = self.ticker.borrow_mut().take()
                && let Some(clear) = global_fn("clearInterval")
            {
                let _ = clear.call1(&JsValue::NULL, &JsValue::from_f64(f64::from(handle)));
            }
        }

        fn shut_down(&self) {
            self.stop_ticking();
            if let Some(channel) = self.channel.borrow_mut().take() {
                channel.set_onmessage(None);
                channel.set_onclose(None);
                channel.set_onerror(None);
                channel.close();
            }
            self.connection.set_onicecandidate(None);
            self.connection.set_ondatachannel(None);
            self.connection.close();
        }
    }

    impl Fabric {
        /// This client's own session id, for the roster query.
        #[allow(dead_code)]
        fn session(&self) -> &str {
            &self.session_id
        }
    }
}
