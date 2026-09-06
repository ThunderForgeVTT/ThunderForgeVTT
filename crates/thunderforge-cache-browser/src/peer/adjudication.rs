//! T098 / T100 / T101 — peer adjudication, in the server-isolated state only.

use super::*;

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
                // Answered in kind when it is news, and only then.
                //
                // `HELLO` is broadcast once, when a client begins. Two
                // clients do not begin at the same instant, so the one that
                // begins first announces itself to peers that are not
                // listening yet and the frame is dropped — it has no
                // adjudication to be delivered to. The later starter's own
                // `HELLO` then lands fine, which left the pair in the exact
                // asymmetry this closes: the Game Master knew the player,
                // the player never learned the Game Master, and a player's
                // client will not adjudicate until it has. Measured, with
                // both clients severed and peers connected: the GM sat at
                // `server-isolated` while the player sat at `reconnecting`
                // indefinitely.
                //
                // Replying only on new information bounds it: two crossing
                // greetings cost one reply each and then stop, because the
                // second reply teaches nobody anything.
                if self.users.insert(from.to_string(), user_id).is_none() {
                    return AdjudicationStep::Broadcast {
                        frames: vec![self.hello()],
                        applied: None,
                    };
                }
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
