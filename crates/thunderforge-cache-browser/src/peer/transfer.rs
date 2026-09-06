//! The framing, the entitlement scope, and the two state machines: what this
//! client may ask for, and what it is willing to serve.

use super::*;

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
