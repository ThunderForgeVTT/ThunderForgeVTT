//! Who may become reachable at all.
//!
//! Every gate on entering [`super::registry`] lives here: world membership,
//! the bound on a session id, and the play-field claim. They belong together
//! because they are all answered at the same moment — once, when the stream
//! is established — and because each one's refusal is written against the
//! others: they must not, between them, tell a caller anything it could not
//! already ask for directly.
//!
//! # Why only the play field may register (spec 036 FR-038)
//!
//! Peer-to-peer continuation belongs to the play field and to nothing else. A
//! companion surface — a character sheet on the other screen, the compendium
//! — acts through the server or not at all.
//!
//! That is enforced *here*, at registration, and nowhere else. A client that
//! does not hold its account's claim for this world never enters the registry,
//! so it cannot be found by `peerSessions`, cannot be addressed by
//! [`PeerRegistry::deliver`], and cannot speak either: `sendPeerSignal`
//! already requires `fromSessionId` to name a session registered to the
//! caller, so an unregistered client's sends fall out as `Ok(false)` with no
//! second check written for them. FR-038 is then a consequence of who is in
//! the map rather than a rule applied to messages.
//!
//! It could not be enforced on the messages in any case: the payloads are
//! opaque strings the server never interprets (spec 028 FR-044), and looking
//! inside one to see whether it carries a roll would undo that deliberately.
//!
//! ## And only at registration
//!
//! A claim that moves away mid-session does **not** retroactively end an open
//! peer stream (research R5: "enforced at registration, not at each signal").
//! Three reasons, in order of weight:
//!
//! 1. The window that is displaced stops being the play field on its own
//!    account — the `playField` stream tells it so, and it tears its peer
//!    transfer down with the rest of what it was doing at the table. The
//!    server killing the stream underneath it adds nothing it does not
//!    already do.
//! 2. What would be interrupted is a content transfer of bytes the client is
//!    *entitled to* — `worldSyncPlan` already put those fingerprints in its
//!    fetch list, and it verifies them before storing (FR-046/FR-047). Peer
//!    transfer is an optimisation over fetching the same bytes from the
//!    server, so aborting one mid-flight costs bandwidth and buys no
//!    authority back.
//! 3. Re-checking per signal is the design R5 rejected, and it would put a
//!    lock acquisition on the hot path of every ICE candidate for a rule
//!    whose whole point is that it is decided once.
//!
//! The residue is bounded and honest: a client that *was* the play field can
//! finish a transfer it started, on a channel it opened while it was. It
//! cannot open a new one — a fresh `peerSignals` is gated again — and a
//! surface that was never the play field is never reachable for a moment.

use std::sync::Arc;
use std::time::Duration;

use async_graphql::{Error, ErrorExtensions};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::play_field::PlayFieldRegistry;
use crate::state::AppState;

use super::registry::{
    GraphQLPeerSignal, MAX_SESSION_ID_LEN, PeerRegistry, PeerSessionGuard, valid_session_id,
};

/// The message a non-member sees, character for character identical to the
/// one `worldSyncPlan` and the canvas asset resolvers produce. A caller must
/// not be able to tell "no such world" from "not your world" by comparing
/// error text between two endpoints.
pub(super) const NOT_A_MEMBER: &str = "user is not a member of this world";

/// How long registration will wait for the caller's play-field claim to
/// arrive before refusing.
///
/// # The race this exists for
///
/// A client opens `playField` and `peerSignals` at nearly the same moment,
/// and the two are separate subscriptions on one socket — nothing orders
/// them. If `peerSignals` is resolved first, the play field's own claim does
/// not exist yet, and a bare check would refuse the very client the claim is
/// about to name. The refusal is recoverable (peer transfer is advisory; the
/// client falls back to fetching from the server, FR-048) but it would be
/// *order-dependent*, which is the kind of defect that shows up as a flaky
/// suite and a feature that works on one machine.
///
/// So the check subscribes to the account's claim changes *before* reading
/// the current one and, finding none, waits this long for one to appear. A
/// claim taken between the read and the subscribe cannot be missed, because
/// the subscribe happens first.
///
/// The wait costs a genuine companion a delayed refusal and nothing else: it
/// is never registered, whatever the order. Only an account with no claim on
/// this world waits at all — a companion of an account that *is* at this
/// world's table passes the first read immediately.
pub(super) const CLAIM_GRACE: Duration = Duration::from_secs(2);

/// Why a signal could not be accepted.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PeerSignalingError {
    #[error("{NOT_A_MEMBER}")]
    Forbidden,
    #[error("session id must be 1..={MAX_SESSION_ID_LEN} characters")]
    InvalidSessionId,
    /// Spec 036 FR-038: the caller is a companion surface, not the play field.
    ///
    /// Says only what the caller already knows about their own account — the
    /// `playFieldClaim` query answers the same question for the same caller —
    /// and nothing about the world, its membership, or anybody else's
    /// sessions. It discloses strictly less than [`Self::Forbidden`], which is
    /// deliberately identical across endpoints so world existence cannot be
    /// probed; there is nothing here to probe.
    #[error("peer transfer belongs to the play field for this world")]
    NotThePlayField,
    #[error("database error: {0}")]
    Database(String),
}

/// Mirrors `world_sync_plan::to_graphql_error`: async-graphql's blanket
/// `From<T: Display>` rules out a second `From` impl, so the `FORBIDDEN`
/// extension is attached here instead.
pub fn to_graphql_error(e: PeerSignalingError) -> Error {
    let msg = e.to_string();
    if matches!(e, PeerSignalingError::Forbidden) {
        Error::new(msg).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
    } else {
        Error::new(msg)
    }
}

/// Is `user_id` a current member of `world_id`, right now, from the database?
pub(super) async fn is_member(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> Result<bool, PeerSignalingError> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| PeerSignalingError::Database(e.to_string()))?;

    tokio::task::spawn_blocking(
        move || match require_world_member(&mut conn, user_id, world_id) {
            Ok(_) => Ok(true),
            Err(WorldMembershipError::NotAMember) => Ok(false),
            Err(WorldMembershipError::Database(msg)) => Err(PeerSignalingError::Database(msg)),
        },
    )
    .await
    .map_err(|e| PeerSignalingError::Database(e.to_string()))?
}

/// Does `user_id` hold the play-field claim for `world_id` right now, or take
/// it within `grace`?
///
/// # Why the claim must name this world
///
/// A claim is not "this account is playing somewhere". It is *this client is
/// at this world's table*, which is why [`crate::play_field::Claim`] carries a
/// `world_id` at all: the table it names is a particular world's table, and a
/// person at one is not thereby at another.
///
/// Peer reachability is world-scoped for the same reason — the registry is
/// keyed by world so that membership means something for signaling. Accepting
/// a claim on some other world would join those two scopes at the seam: an
/// account holding the table in a scratch world would gain peer reachability
/// in every world it opened a companion window on, and the capability would
/// follow the *account* instead of the table. That is precisely the leak
/// FR-038 names, arriving by the back door.
///
/// The narrower check also costs nothing real. Only one client of an account
/// can be at a table at all (FR-027), so an account with a companion open on
/// world B and the play field on world A has no play field on world B — which
/// is the true answer, not an inconvenient one.
///
/// The claim is checked per *account*, not against this connection's
/// `sessionId`: the peer session id is generated per peer-transfer start and
/// is unrelated to the play field's `clientId`, so comparing them would refuse
/// everybody. The account-level answer is the one FR-038 needs — a companion's
/// account may well hold the table in another window, and that window is the
/// one doing the peering.
async fn holds_play_field(
    claims: &PlayFieldRegistry,
    user_id: Uuid,
    world_id: Uuid,
    grace: Duration,
) -> bool {
    let at_this_table = |claim: Option<crate::play_field::Claim>| {
        claim.is_some_and(|claim| claim.world_id == world_id)
    };

    // Watch first, then read: a claim taken in between arrives on the watch
    // rather than being lost between the two.
    let mut changes = claims.watch(user_id);
    if at_this_table(claims.current(user_id)) {
        return true;
    }
    if grace.is_zero() {
        return false;
    }

    let deadline = tokio::time::sleep(grace);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => return false,
            change = changes.recv() => match change {
                Ok(claim) => {
                    if at_this_table(claim) {
                        return true;
                    }
                }
                // Fell behind this account's own claim moving. The registry is
                // the truth; re-read it rather than guessing from a gap.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if at_this_table(claims.current(user_id)) {
                        return true;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
            },
        }
    }
}

/// Register one live connection for peer signaling, if it may be reachable.
///
/// Factored out of [`peer_signals_stream`] so the rule can be tested without a
/// GraphQL context, and so the two gates it applies read in one place.
///
/// **Membership is checked by the caller** (the stream), because it needs the
/// database; what is decided here is the play-field question.
pub(super) async fn register_if_play_field(
    peers: &Arc<PeerRegistry>,
    claims: &PlayFieldRegistry,
    user_id: Uuid,
    world_id: Uuid,
    session_id: String,
    grace: Duration,
) -> Result<(PeerSessionGuard, UnboundedReceiver<GraphQLPeerSignal>), PeerSignalingError> {
    if !valid_session_id(&session_id) {
        return Err(PeerSignalingError::InvalidSessionId);
    }
    if !holds_play_field(claims, user_id, world_id, grace).await {
        return Err(PeerSignalingError::NotThePlayField);
    }
    Ok(peers.register(world_id, session_id, user_id))
}

#[cfg(test)]
mod tests {
    //! Also pure: the play-field claim is an in-process registry, so the rule
    //! it enforces can be tested without a database or a GraphQL context.

    use super::*;

    fn world(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn user(n: u128) -> Uuid {
        Uuid::from_u128(1_000 + n)
    }

    fn signal(from: &str, payload: &str) -> GraphQLPeerSignal {
        GraphQLPeerSignal {
            from_session_id: from.to_string(),
            payload: payload.to_string(),
        }
    }

    fn claims_for_test() -> Arc<PlayFieldRegistry> {
        Arc::new(PlayFieldRegistry::new())
    }

    /// The requirement itself, at the only place it is enforced. A companion
    /// surface that never enters the registry cannot be listed, cannot be
    /// addressed, and cannot answer — so FR-038 holds without anybody ever
    /// looking inside a payload.
    #[tokio::test]
    async fn a_companion_surface_is_refused_and_so_is_never_reachable() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();

        let err = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "sess-companion".into(),
            Duration::ZERO,
        )
        .await
        .expect_err("a surface holding no claim must not become reachable");

        assert_eq!(err, PeerSignalingError::NotThePlayField);
        assert_eq!(peers.session_count(world(1)), 0);
        assert_eq!(peers.session_user(world(1), "sess-companion"), None);
        assert!(
            peers.sessions_excluding_user(world(1), user(2)).is_empty(),
            "a refused surface must not appear in anybody's roster"
        );
    }

    /// The refusal says only what the caller can already ask about their own
    /// account, and in particular does not become a second way to learn
    /// whether a world exists or who belongs to it.
    #[tokio::test]
    async fn the_refusal_discloses_nothing_about_the_world() {
        let msg = PeerSignalingError::NotThePlayField.to_string();
        assert!(!msg.contains("member"), "{msg}");
        assert_ne!(msg, NOT_A_MEMBER);
        assert!(
            !msg.contains(&world(1).to_string()) && !msg.contains(&user(1).to_string()),
            "a refusal must not echo identifiers back"
        );
    }

    /// The other half: the client that *is* at the table registers exactly as
    /// it always did.
    #[tokio::test]
    async fn the_play_field_registers_and_is_reachable() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();
        let (_claim, _held) = claims.claim(user(1), "window-a".into(), world(1));

        let (_guard, mut rx) = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "sess-a".into(),
            Duration::ZERO,
        )
        .await
        .expect("the play field must be reachable");

        assert_eq!(peers.session_user(world(1), "sess-a"), Some(user(1)));
        assert!(peers.deliver(world(1), "sess-a", signal("b", "offer")));
        assert_eq!(rx.recv().await.unwrap().payload, "offer");
    }

    /// The world-scoping decision, made explicit as a test because it is the
    /// one that could be quietly loosened later: a claim is a seat at *a*
    /// table, and holding one elsewhere is not holding this one.
    #[tokio::test]
    async fn a_claim_on_another_world_does_not_make_this_one_peerable() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();
        let (_claim, _held) = claims.claim(user(1), "window-a".into(), world(2));

        let err = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "sess-a".into(),
            Duration::ZERO,
        )
        .await
        .expect_err("a table in another world is not this world's table");

        assert_eq!(err, PeerSignalingError::NotThePlayField);
        assert_eq!(peers.session_count(world(1)), 0);
    }

    /// The race. `playField` and `peerSignals` are two subscriptions on one
    /// socket and nothing orders them, so the claim may land a moment after
    /// the peer stream opens. Without the grace window this is a refusal that
    /// depends on scheduling — which is the shape of a flaky suite.
    #[tokio::test]
    async fn a_claim_arriving_a_moment_late_still_registers() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();

        let claiming = Arc::clone(&claims);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let (_claim, _held) = claiming.claim(user(1), "window-a".into(), world(1));
            // Hold it past the end of the test, as a live client would.
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        let (_guard, _rx) = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "sess-a".into(),
            Duration::from_secs(2),
        )
        .await
        .expect("a claim taken just after the peer stream opened must still count");

        assert_eq!(peers.session_user(world(1), "sess-a"), Some(user(1)));
    }

    /// And the grace window is a wait for *this* account's table, not a wait
    /// that any claim anywhere can end.
    #[tokio::test]
    async fn the_grace_window_ends_in_a_refusal_when_no_claim_arrives() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();
        let (_other, _held) = claims.claim(user(2), "their-window".into(), world(1));

        let err = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "sess-a".into(),
            Duration::from_millis(50),
        )
        .await
        .expect_err("somebody else's table is not this caller's");

        assert_eq!(err, PeerSignalingError::NotThePlayField);
    }

    /// The decision recorded in the module docs, kept honest here: the gate is
    /// at registration, so a claim that moves to another window does not tear
    /// down a transfer already in flight on a channel that was opened
    /// legitimately. The displaced client ends its own stream; when it does,
    /// the guard removes it, and a fresh registration is gated again.
    #[tokio::test]
    async fn a_claim_released_mid_session_does_not_break_an_open_peer_stream() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();

        let (guard, mut rx) = {
            let (_claim, held) = claims.claim(user(1), "window-a".into(), world(1));
            let registered = register_if_play_field(
                &peers,
                &claims,
                user(1),
                world(1),
                "sess-a".into(),
                Duration::ZERO,
            )
            .await
            .expect("the play field registers");
            drop(held);
            registered
        };

        assert_eq!(claims.current(user(1)), None, "the claim is gone");
        assert!(
            peers.deliver(world(1), "sess-a", signal("b", "answer")),
            "an open channel keeps working: the bytes in flight were already \
             in this client's fetch list, and the server is not the thing \
             holding the connection open"
        );
        assert_eq!(rx.recv().await.unwrap().payload, "answer");

        // But nothing new. Ending the stream unregisters, and asking again is
        // asking as a companion.
        drop(guard);
        assert_eq!(peers.session_count(world(1)), 0);
        assert_eq!(
            register_if_play_field(
                &peers,
                &claims,
                user(1),
                world(1),
                "sess-a".into(),
                Duration::ZERO,
            )
            .await
            .expect_err("re-registering without the claim must be refused"),
            PeerSignalingError::NotThePlayField
        );
    }

    /// An invalid session id is still refused before the claim is consulted —
    /// the cheap check stays first.
    #[tokio::test]
    async fn an_oversized_session_id_is_refused_before_the_claim_is_consulted() {
        let peers = Arc::new(PeerRegistry::new());
        let claims = claims_for_test();

        let err = register_if_play_field(
            &peers,
            &claims,
            user(1),
            world(1),
            "a".repeat(MAX_SESSION_ID_LEN + 1),
            Duration::from_secs(30),
        )
        .await
        .expect_err("an oversized session id must be refused");

        assert_eq!(err, PeerSignalingError::InvalidSessionId);
    }
}
