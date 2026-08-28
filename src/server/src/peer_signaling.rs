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
//! # Why a "session" is a connection, not a user
//!
//! Presence (`players_online`) is keyed by `(player_id, world_id)` and is the
//! wrong registry for this. One person may have two tabs open, and each tab
//! is a separate WebRTC endpoint with a separate data channel — a peer must
//! be able to address *one* of them. So a session here is one live client
//! connection, identified by an opaque id the client generates per page load.
//! The server never persists it, never links it to anything, and forgets it
//! when the socket drops.
//!
//! # Why the subscription is the registry
//!
//! Registration begins when `peerSignals` establishes its stream and ends
//! when that stream is dropped — the guard returned by
//! [`PeerRegistry::register`] is owned by the stream itself, so there is no
//! way for an entry to outlive the connection that created it. That is
//! FR-050 ("peer connections MUST NOT persist beyond the session") enforced
//! by construction rather than by a cleanup job that can be forgotten,
//! misconfigured, or skipped during a crash.
//!
//! This is why `peerSignals` takes a `sessionId` the contract's SDL does not
//! show: a client cannot be reachable without telling the server the address
//! it wants to be reachable at, and `PeerSignal` has no field that could
//! carry it in the other direction. A deliberate, minimal extension.
//!
//! # Why the roster is a query and not a push
//!
//! [`peer_sessions`] answers "who else is here right now" on demand. A
//! newcomer asks once and initiates to everybody it finds; nobody needs a
//! join notification, because the newcomer is always the initiator, and a
//! departure is noticed when the data channel closes. Keeping it a pull keeps
//! the server a post box instead of quietly becoming a presence service with
//! a second, divergent notion of who is online.
//!
//! # Why the registry is a process global
//!
//! Like the auth rate limiter in `auth_middleware`, this is live-connection
//! state belonging to one process — the sockets are here or they are nowhere.
//! It is a plain type with no global of its own, and [`registry`] is the one
//! place the static lives, so tests build their own instances and moving it
//! into `AppState` later is a mechanical change rather than a redesign.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use async_graphql::{
    Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult, SimpleObject,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;

/// The message a non-member sees, character for character identical to the
/// one `worldSyncPlan` and the canvas asset resolvers produce. A caller must
/// not be able to tell "no such world" from "not your world" by comparing
/// error text between two endpoints.
const NOT_A_MEMBER: &str = "user is not a member of this world";

/// Upper bound on a client-supplied session id.
///
/// Session ids are map keys the server holds for the life of a connection, so
/// an unbounded one is an unbounded allocation a client chooses. A UUID
/// string is 36 characters; 128 leaves room for a client that prefixes or
/// suffixes its own id without leaving room for abuse.
pub const MAX_SESSION_ID_LEN: usize = 128;

/// One relayed signal, exactly as the contract defines it.
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "PeerSignal")]
pub struct GraphQLPeerSignal {
    /// The session that sent it. Server-populated: it comes from the
    /// registry, never from a field the sender could set freely.
    pub from_session_id: String,
    /// Opaque SDP offer/answer or ICE candidate. Never interpreted, never
    /// stored, never logged.
    pub payload: String,
}

#[derive(InputObject, Debug, Clone)]
#[graphql(name = "PeerSignalInput")]
pub struct GraphQLPeerSignalInput {
    pub world_id: Uuid,
    /// The sender's own session id.
    ///
    /// Not in the contract's SDL, and required for the same reason
    /// `peerSignals` takes one: `PeerSignal.fromSessionId` is
    /// server-populated, so the server has to know which of the caller's
    /// connections is speaking — a user may have several. It is verified
    /// against the registry below, so it is an assertion the server checks,
    /// not a field the sender is trusted on.
    pub from_session_id: String,
    pub to_session_id: String,
    /// Opaque. The server is a post box.
    pub payload: String,
}

/// A live client connection registered for one world.
#[derive(Debug)]
struct RegisteredSession {
    /// Who this connection belongs to, so a signal can be dropped when that
    /// user's membership is checked and found gone. Storing the user id is
    /// what lets membership be re-checked per signal (the contract's explicit
    /// words) without a second lookup table.
    user_id: Uuid,
    /// Distinguishes this registration from a later one that reused the same
    /// session id — see [`PeerSessionGuard::drop`].
    token: u64,
    tx: mpsc::UnboundedSender<GraphQLPeerSignal>,
}

/// Who is reachable right now, per world.
///
/// Deliberately knows nothing about *content*: the server does not track
/// which bytes any client holds, and must not start. Nothing in the spec asks
/// for it, and a map of "who has what" would be a standing privacy cost paid
/// for an advisory hint.
#[derive(Debug, Default)]
pub struct PeerRegistry {
    worlds: Mutex<HashMap<Uuid, HashMap<String, RegisteredSession>>>,
    next_token: AtomicU64,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a live connection and hand back its inbox.
    ///
    /// The guard is the registration. Drop it — by dropping the subscription
    /// stream that owns it, which is what happens when the socket goes away —
    /// and the session is gone from the registry (FR-050).
    pub fn register(
        self: &Arc<Self>,
        world_id: Uuid,
        session_id: String,
        user_id: Uuid,
    ) -> (PeerSessionGuard, UnboundedReceiver<GraphQLPeerSignal>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);

        self.worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .entry(world_id)
            .or_default()
            .insert(session_id.clone(), RegisteredSession { user_id, token, tx });

        (
            PeerSessionGuard {
                registry: Arc::clone(self),
                world_id,
                session_id,
                token,
            },
            rx,
        )
    }

    /// Which user a session belongs to, or `None` if the session has ended.
    pub fn session_user(&self, world_id: Uuid, session_id: &str) -> Option<Uuid> {
        self.worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .get(&world_id)
            .and_then(|sessions| sessions.get(session_id))
            .map(|s| s.user_id)
    }

    /// Hand one signal to one session. `false` means it was not delivered.
    ///
    /// A session that has ended is a no-op, not an error, and nothing is
    /// queued for it: the contract says so, and queuing would mean holding
    /// opaque client payloads on the server, which is exactly what "the
    /// server is a post box" rules out.
    pub fn deliver(&self, world_id: Uuid, to_session_id: &str, signal: GraphQLPeerSignal) -> bool {
        self.worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .get(&world_id)
            .and_then(|sessions| sessions.get(to_session_id))
            .map(|s| s.tx.send(signal).is_ok())
            .unwrap_or(false)
    }

    /// Every live session in `world_id` that does not belong to `user_id`.
    ///
    /// The caller's own connections are excluded because they are not useful
    /// peers: two tabs of one browser share an origin, and therefore share
    /// the cache a peer transfer would be moving bytes into. Sorted so the
    /// answer is stable for a caller comparing two rosters.
    pub fn sessions_excluding_user(&self, world_id: Uuid, user_id: Uuid) -> Vec<String> {
        let mut ids: Vec<String> = self
            .worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .get(&world_id)
            .map(|sessions| {
                sessions
                    .iter()
                    .filter(|(_, s)| s.user_id != user_id)
                    .map(|(id, _)| id.clone())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Whether anybody other than `user_id` is reachable in this world.
    ///
    /// This is the whole of what `PlanItem.peerAvailable` reports (T087) —
    /// reachability, never holdings.
    pub fn has_peer_for(&self, world_id: Uuid, user_id: Uuid) -> bool {
        self.worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .get(&world_id)
            .is_some_and(|sessions| sessions.values().any(|s| s.user_id != user_id))
    }

    /// How many sessions a world holds.
    ///
    /// Only the tests read it today — it is the direct way to assert that a
    /// dropped guard really removed its entry, which is the FR-050 property
    /// and the one most likely to rot silently.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn session_count(&self, world_id: Uuid) -> usize {
        self.worlds
            .lock()
            .expect("peer registry mutex poisoned")
            .get(&world_id)
            .map_or(0, HashMap::len)
    }

    fn unregister(&self, world_id: Uuid, session_id: &str, token: u64) {
        let mut worlds = self.worlds.lock().expect("peer registry mutex poisoned");
        let Some(sessions) = worlds.get_mut(&world_id) else {
            return;
        };
        // Only if this is still *our* registration. A client that reconnects
        // with the same session id before the old stream finishes dropping
        // would otherwise have its new, live entry deleted by the old guard.
        if sessions.get(session_id).is_some_and(|s| s.token == token) {
            sessions.remove(session_id);
        }
        if sessions.is_empty() {
            // Otherwise the map grows by one entry per world ever opened and
            // never shrinks — the same slow leak `WorldRouter::reap` exists
            // to prevent, avoided here because registration is guarded and so
            // the last departure is observable.
            worlds.remove(&world_id);
        }
    }
}

/// A registration that ends when it is dropped.
#[derive(Debug)]
pub struct PeerSessionGuard {
    registry: Arc<PeerRegistry>,
    world_id: Uuid,
    session_id: String,
    token: u64,
}

impl Drop for PeerSessionGuard {
    fn drop(&mut self) {
        self.registry
            .unregister(self.world_id, &self.session_id, self.token);
    }
}

/// The process's registry of live peer sessions.
pub fn registry() -> &'static Arc<PeerRegistry> {
    static REGISTRY: LazyLock<Arc<PeerRegistry>> = LazyLock::new(|| Arc::new(PeerRegistry::new()));
    &REGISTRY
}

/// Why a signal could not be accepted.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PeerSignalingError {
    #[error("{NOT_A_MEMBER}")]
    Forbidden,
    #[error("session id must be 1..={MAX_SESSION_ID_LEN} characters")]
    InvalidSessionId,
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

fn valid_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_SESSION_ID_LEN
}

/// Is `user_id` a current member of `world_id`, right now, from the database?
async fn is_member(
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

/// Relay one signal. `Ok(false)` means it was dropped, which is an ordinary
/// outcome and not an error.
///
/// Membership is re-checked here, per signal, for **both** ends — the
/// contract's words, and the reason is that a subscription is long-lived: a
/// player removed from a world an hour into their session would otherwise
/// keep signaling on a check made when they still belonged.
pub async fn send_peer_signal_impl(
    state: &AppState,
    peers: &PeerRegistry,
    user_id: Uuid,
    input: GraphQLPeerSignalInput,
) -> Result<bool, PeerSignalingError> {
    if !valid_session_id(&input.from_session_id) || !valid_session_id(&input.to_session_id) {
        return Err(PeerSignalingError::InvalidSessionId);
    }

    // The sender's end. Refusing a non-member here is the same refusal every
    // other world-scoped resolver makes, in the same words.
    if !is_member(state, user_id, input.world_id).await? {
        return Err(PeerSignalingError::Forbidden);
    }

    // `fromSessionId` is an assertion, so check it. A caller may only speak as
    // a session that is registered, in this world, to them — otherwise a
    // member could forge `PeerSignal.fromSessionId` and impersonate another
    // participant to the peer receiving it.
    if peers.session_user(input.world_id, &input.from_session_id) != Some(user_id) {
        return Ok(false);
    }

    // The recipient's end. `None` means the session ended: drop it, do not
    // queue it, do not report it as a failure the caller could use to probe
    // who is online.
    let Some(recipient) = peers.session_user(input.world_id, &input.to_session_id) else {
        return Ok(false);
    };
    if !is_member(state, recipient, input.world_id).await? {
        return Ok(false);
    }

    Ok(peers.deliver(
        input.world_id,
        &input.to_session_id,
        GraphQLPeerSignal {
            from_session_id: input.from_session_id,
            payload: input.payload,
        },
    ))
}

#[derive(Default)]
pub struct PeerSignalingMutation;

#[async_graphql::Object]
impl PeerSignalingMutation {
    /// Relay one opaque signaling payload to one session in one world.
    ///
    /// `false` means it was not delivered — the addressed session has ended,
    /// or is no longer a member. Neither is retried and neither is queued.
    async fn send_peer_signal(
        &self,
        ctx: &Context<'_>,
        input: GraphQLPeerSignalInput,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        send_peer_signal_impl(state, registry(), auth_user.user_id, input)
            .await
            .map_err(to_graphql_error)
    }
}

#[derive(Default)]
pub struct PeerSignalingQuery;

#[async_graphql::Object]
impl PeerSignalingQuery {
    /// Who else is reachable in this world right now.
    ///
    /// Advisory and instantly stale by nature — a session may end between
    /// this answer and the first signal sent to it, which is why a signal to
    /// a departed session is a no-op rather than an error.
    async fn peer_sessions(&self, ctx: &Context<'_>, world_id: Uuid) -> GraphQLResult<Vec<String>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        peer_sessions_impl(state, registry(), auth_user.user_id, world_id)
            .await
            .map_err(to_graphql_error)
    }
}

pub async fn peer_sessions_impl(
    state: &AppState,
    peers: &PeerRegistry,
    user_id: Uuid,
    world_id: Uuid,
) -> Result<Vec<String>, PeerSignalingError> {
    if !is_member(state, user_id, world_id).await? {
        return Err(PeerSignalingError::Forbidden);
    }
    Ok(peers.sessions_excluding_user(world_id, user_id))
}

/// The `peerSignals` stream, factored out of `SubscriptionRoot` so the
/// registration lifetime lives beside the registry it belongs to.
///
/// The guard travels *inside* the stream state. That is the load-bearing
/// detail: async-graphql drops the stream when the client unsubscribes or the
/// socket dies, which drops the guard, which unregisters the session. There
/// is no path that leaves an entry behind, because there is no path that
/// keeps the stream alive without it.
pub async fn peer_signals_stream(
    ctx: &Context<'_>,
    world_id: Uuid,
    session_id: String,
) -> std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<GraphQLPeerSignal, Error>> + Send>> {
    use futures_util::StreamExt;

    let failure = |msg: &str| {
        Box::pin(tokio_stream::iter(vec![Err(Error::new(msg.to_string()))]).boxed())
            as std::pin::Pin<
                Box<dyn futures_util::Stream<Item = Result<GraphQLPeerSignal, Error>> + Send>,
            >
    };

    let Ok(state) = app_state(ctx) else {
        return failure("Application state unavailable");
    };
    let Ok(auth_user) = authenticated_user(ctx) else {
        return failure("Authentication required");
    };
    if !valid_session_id(&session_id) {
        return failure(&PeerSignalingError::InvalidSessionId.to_string());
    }
    // Registering is itself a grant of reachability, so it is gated the same
    // way every other world subscription is. Any failure to confirm — pool
    // error, database error — refuses, because a long-lived grant handed out
    // on an unconfirmed check is the wrong direction to be wrong in.
    match is_member(state, auth_user.user_id, world_id).await {
        Ok(true) => {}
        Ok(false) => return failure("You must be a member of this world"),
        Err(_) => return failure("You must be a member of this world"),
    }

    let (guard, rx) = registry().register(world_id, session_id, auth_user.user_id);

    Box::pin(futures_util::stream::unfold(
        (rx, guard),
        |(mut rx, guard)| async move { rx.recv().await.map(|signal| (Ok(signal), (rx, guard))) },
    ))
}

#[cfg(test)]
mod tests {
    //! The registry tests are pure — no database, no schema, the way
    //! `WorldRouter`'s are — because the lifetime rules are the part most
    //! likely to break silently. The relay tests need a real Postgres
    //! (`DATABASE_URL`), because "membership is re-checked per signal" is a
    //! claim about the database and mocking it would test the mock.

    use super::*;
    use crate::test_support::*;

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

    /// The core relay property: a signal is a letter to one address, not a
    /// broadcast. If this ever fans out, every peer in the world learns the
    /// SDP of every other, which is both wrong and a privacy leak.
    #[tokio::test]
    async fn a_signal_reaches_only_the_session_it_is_addressed_to() {
        let peers = Arc::new(PeerRegistry::new());
        let (_a, mut rx_a) = peers.register(world(1), "a".into(), user(1));
        let (_b, mut rx_b) = peers.register(world(1), "b".into(), user(2));
        let (_c, mut rx_c) = peers.register(world(1), "c".into(), user(3));

        assert!(peers.deliver(world(1), "b", signal("a", "offer")));

        assert_eq!(rx_b.recv().await.unwrap().payload, "offer");
        assert!(rx_a.try_recv().is_err(), "the sender must not hear itself");
        assert!(
            rx_c.try_recv().is_err(),
            "a session that was not addressed must receive nothing at all"
        );
    }

    /// Worlds are separate post boxes. A session id in one world must not be
    /// addressable from another, or world membership stops meaning anything
    /// for signaling.
    #[test]
    fn a_session_is_not_addressable_from_another_world() {
        let peers = Arc::new(PeerRegistry::new());
        let (_b, mut rx_b) = peers.register(world(1), "b".into(), user(2));

        assert!(!peers.deliver(world(2), "b", signal("a", "offer")));
        assert!(rx_b.try_recv().is_err());
    }

    /// FR-050, enforced by construction: the registration is the guard, so a
    /// dropped stream cannot leave a reachable ghost behind. A leak here is
    /// invisible in normal use — signals to the ghost just vanish — and shows
    /// up much later as a roster full of addresses nobody answers on.
    #[test]
    fn a_session_disappears_from_the_registry_when_its_stream_is_dropped() {
        let peers = Arc::new(PeerRegistry::new());
        let (guard, _rx) = peers.register(world(1), "a".into(), user(1));
        assert_eq!(peers.session_count(world(1)), 1);

        drop(guard);

        assert_eq!(peers.session_count(world(1)), 0);
        assert_eq!(peers.session_user(world(1), "a"), None);
        assert!(!peers.deliver(world(1), "a", signal("b", "offer")));
    }

    /// A reconnect that reuses its session id must not be unregistered by the
    /// old guard finishing its drop a moment later. Without the token check
    /// this races: the client is registered, then silently unreachable, and
    /// nothing anywhere reports an error.
    #[test]
    fn a_late_dropping_old_guard_does_not_unregister_a_reconnected_session() {
        let peers = Arc::new(PeerRegistry::new());
        let (old, _old_rx) = peers.register(world(1), "a".into(), user(1));
        let (_new, mut new_rx) = peers.register(world(1), "a".into(), user(1));

        drop(old);

        assert_eq!(peers.session_user(world(1), "a"), Some(user(1)));
        assert!(peers.deliver(world(1), "a", signal("b", "offer")));
        assert_eq!(new_rx.try_recv().unwrap().payload, "offer");
    }

    /// The roster is the newcomer's only way to find anyone, so its two
    /// properties both matter: it contains the live and only the live, and it
    /// leaves out the asker's own connections (which share a browser origin,
    /// and therefore share the cache a transfer would fill).
    #[test]
    fn the_roster_omits_the_callers_own_sessions_and_lists_only_live_ones() {
        let peers = Arc::new(PeerRegistry::new());
        let (_mine, _r1) = peers.register(world(1), "mine".into(), user(1));
        let (_other_tab, _r2) = peers.register(world(1), "mine-tab-2".into(), user(1));
        let (_theirs, _r3) = peers.register(world(1), "theirs".into(), user(2));
        let (departed, _r4) = peers.register(world(1), "departed".into(), user(3));

        drop(departed);

        assert_eq!(
            peers.sessions_excluding_user(world(1), user(1)),
            vec!["theirs".to_string()]
        );
        // And from the other side, symmetrically.
        assert_eq!(
            peers.sessions_excluding_user(world(1), user(2)),
            vec!["mine".to_string(), "mine-tab-2".to_string()]
        );
    }

    /// T087's whole question, at the registry level: "am I alone?".
    #[test]
    fn a_lone_session_has_no_peers_and_a_second_users_arrival_gives_it_one() {
        let peers = Arc::new(PeerRegistry::new());
        let (_alone, _rx) = peers.register(world(1), "alone".into(), user(1));

        assert!(
            !peers.has_peer_for(world(1), user(1)),
            "a user's own tabs are not peers to themselves"
        );

        let (theirs, _rx2) = peers.register(world(1), "theirs".into(), user(2));
        assert!(peers.has_peer_for(world(1), user(1)));

        drop(theirs);
        assert!(
            !peers.has_peer_for(world(1), user(1)),
            "and it goes false again the moment they leave"
        );
    }

    /// A world nobody is in must not keep an entry, or the map grows by one
    /// per world ever opened on a long-lived server.
    #[test]
    fn the_last_session_leaving_a_world_removes_the_world() {
        let peers = Arc::new(PeerRegistry::new());
        let (a, _rx) = peers.register(world(1), "a".into(), user(1));
        let (b, _rx2) = peers.register(world(1), "b".into(), user(2));

        drop(a);
        assert_eq!(peers.session_count(world(1)), 1);
        drop(b);
        assert_eq!(peers.session_count(world(1)), 0);
        assert!(peers.sessions_excluding_user(world(1), user(9)).is_empty());
    }

    /// T086 registration: the SDL must carry exactly the names the client
    /// half was written against, including the two deliberate extensions
    /// (`fromSessionId` on the input, `sessionId` on the subscription). The
    /// schema is only built in `main`, so a missing registration is a startup
    /// panic in production rather than a compile error here.
    #[test]
    fn the_signaling_surface_is_registered_under_the_contracts_names() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(
            sdl.contains("sendPeerSignal("),
            "mutation must be on the root"
        );
        assert!(
            sdl.contains("peerSessions("),
            "roster query must be on the root"
        );
        assert!(
            sdl.contains("peerSignals("),
            "subscription must be on the root"
        );
        assert!(sdl.contains("type PeerSignal {"));
        assert!(sdl.contains("input PeerSignalInput {"));
        assert!(sdl.contains("fromSessionId: String!"));
        assert!(sdl.contains("toSessionId: String!"));
    }

    /// Registers a world with two members and returns
    /// `(state, world_id, member_a, member_b)`.
    fn two_member_world() -> (AppState, Uuid, Uuid, Uuid) {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let other_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_id, "Player");
        drop(conn);
        (state, world_id, owner_id, other_id)
    }

    /// The happy path end to end: two members of one world, one signal, one
    /// recipient, payload untouched.
    #[tokio::test]
    async fn a_member_can_relay_a_signal_to_another_members_session() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, mut rx_a) = peers.register(world_id, "sess-a".into(), a);
        let (_gb, mut rx_b) = peers.register(world_id, "sess-b".into(), b);

        let delivered = send_peer_signal_impl(
            &state,
            &peers,
            a,
            GraphQLPeerSignalInput {
                world_id,
                from_session_id: "sess-a".into(),
                to_session_id: "sess-b".into(),
                payload: "v=0\r\no=- 1 1 IN IP4 0.0.0.0".into(),
            },
        )
        .await
        .expect("a member signaling another member must not error");

        assert!(delivered);
        let got = rx_b.recv().await.unwrap();
        assert_eq!(got.from_session_id, "sess-a");
        assert_eq!(
            got.payload, "v=0\r\no=- 1 1 IN IP4 0.0.0.0",
            "the payload must arrive byte-identical; the server never interprets it"
        );
        assert!(rx_a.try_recv().is_err());
    }

    /// A stranger holding a valid session id must not be able to use this
    /// world's post box, and must be refused in the same words as every other
    /// non-member refusal so the error cannot be used to probe world
    /// existence.
    #[tokio::test]
    async fn a_non_member_cannot_send_into_a_world() {
        let (state, world_id, _a, b) = two_member_world();
        let mut conn = state.db_pool.get().unwrap();
        let stranger = insert_test_user(&mut conn);
        drop(conn);

        let peers = Arc::new(PeerRegistry::new());
        let (_gs, _rx_s) = peers.register(world_id, "sess-x".into(), stranger);
        let (_gb, mut rx_b) = peers.register(world_id, "sess-b".into(), b);

        let err = send_peer_signal_impl(
            &state,
            &peers,
            stranger,
            GraphQLPeerSignalInput {
                world_id,
                from_session_id: "sess-x".into(),
                to_session_id: "sess-b".into(),
                payload: "offer".into(),
            },
        )
        .await
        .expect_err("a non-member must not be relayed");

        assert_eq!(err, PeerSignalingError::Forbidden);
        assert_eq!(err.to_string(), NOT_A_MEMBER);
        assert!(
            rx_b.try_recv().is_err(),
            "nothing may reach the recipient when the sender is refused"
        );
    }

    /// The contract's "membership is checked per signal, not once at connect":
    /// a subscription outlives the membership that authorized it, so a player
    /// removed mid-session must stop being relayed immediately — not at the
    /// next reconnect.
    #[tokio::test]
    async fn a_sender_who_loses_membership_mid_session_stops_being_relayed() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, _rx_a) = peers.register(world_id, "sess-a".into(), a);
        let (_gb, mut rx_b) = peers.register(world_id, "sess-b".into(), b);

        let input = || GraphQLPeerSignalInput {
            world_id,
            from_session_id: "sess-b".into(),
            to_session_id: "sess-a".into(),
            payload: "offer".into(),
        };

        // Registered and relayed while they still belong.
        assert!(
            send_peer_signal_impl(&state, &peers, b, input())
                .await
                .unwrap()
        );

        let mut conn = state.db_pool.get().unwrap();
        remove_test_world_member(&mut conn, world_id, b);
        drop(conn);

        let err = send_peer_signal_impl(&state, &peers, b, input())
            .await
            .expect_err("the same registered session must be refused once membership is gone");
        assert_eq!(err, PeerSignalingError::Forbidden);
        assert!(rx_b.try_recv().is_err());
    }

    /// The other end of the same rule. The recipient's membership is checked
    /// too, because a removed player's still-open subscription would
    /// otherwise keep receiving signaling from the world they were removed
    /// from.
    #[tokio::test]
    async fn a_signal_to_a_recipient_who_has_lost_membership_is_dropped() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, _rx_a) = peers.register(world_id, "sess-a".into(), a);
        let (_gb, mut rx_b) = peers.register(world_id, "sess-b".into(), b);

        let mut conn = state.db_pool.get().unwrap();
        remove_test_world_member(&mut conn, world_id, b);
        drop(conn);

        let delivered = send_peer_signal_impl(
            &state,
            &peers,
            a,
            GraphQLPeerSignalInput {
                world_id,
                from_session_id: "sess-a".into(),
                to_session_id: "sess-b".into(),
                payload: "offer".into(),
            },
        )
        .await
        .expect("dropping a signal is an ordinary outcome, not an error");

        assert!(!delivered);
        assert!(
            rx_b.try_recv().is_err(),
            "a session whose user lost membership must receive nothing"
        );
    }

    /// Sessions end constantly and a roster is stale the instant it is read,
    /// so addressing one that has gone is normal traffic. It must be a quiet
    /// `false`: an error would train clients to retry, and queuing would make
    /// the server hold opaque payloads it has no business holding.
    #[tokio::test]
    async fn signaling_a_session_that_has_ended_is_a_no_op_rather_than_an_error() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, _rx_a) = peers.register(world_id, "sess-a".into(), a);
        let (gone, _rx_b) = peers.register(world_id, "sess-b".into(), b);
        drop(gone);

        let delivered = send_peer_signal_impl(
            &state,
            &peers,
            a,
            GraphQLPeerSignalInput {
                world_id,
                from_session_id: "sess-a".into(),
                to_session_id: "sess-b".into(),
                payload: "offer".into(),
            },
        )
        .await
        .expect("a departed session must not produce an error");

        assert!(!delivered);
    }

    /// `fromSessionId` is a claim, and an unchecked one would let any member
    /// forge `PeerSignal.fromSessionId` — putting words in another
    /// participant's mouth on a channel the recipient is about to trust for
    /// SDP.
    #[tokio::test]
    async fn a_member_cannot_send_as_a_session_that_is_not_theirs() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, _rx_a) = peers.register(world_id, "sess-a".into(), a);
        let (_gb, mut rx_b) = peers.register(world_id, "sess-b".into(), b);

        let delivered = send_peer_signal_impl(
            &state,
            &peers,
            a,
            GraphQLPeerSignalInput {
                world_id,
                // b's session, claimed by a.
                from_session_id: "sess-b".into(),
                to_session_id: "sess-b".into(),
                payload: "offer".into(),
            },
        )
        .await
        .unwrap();

        assert!(!delivered);
        assert!(rx_b.try_recv().is_err());
    }

    /// Session ids are map keys the server holds for a whole connection, so
    /// an unbounded one is an allocation the client gets to choose the size
    /// of.
    #[tokio::test]
    async fn an_oversized_session_id_is_rejected_before_any_database_work() {
        let (state, world_id, a, _b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());

        let err = send_peer_signal_impl(
            &state,
            &peers,
            a,
            GraphQLPeerSignalInput {
                world_id,
                from_session_id: "a".repeat(MAX_SESSION_ID_LEN + 1),
                to_session_id: "b".into(),
                payload: "offer".into(),
            },
        )
        .await
        .expect_err("an oversized session id must be refused");

        assert_eq!(err, PeerSignalingError::InvalidSessionId);
    }

    /// The roster is world-scoped and member-only, for the same reason the
    /// subscription is: it is a list of who is at someone else's table.
    #[tokio::test]
    async fn the_roster_query_refuses_a_non_member() {
        let (state, world_id, _a, _b) = two_member_world();
        let mut conn = state.db_pool.get().unwrap();
        let stranger = insert_test_user(&mut conn);
        drop(conn);

        let peers = Arc::new(PeerRegistry::new());
        let err = peer_sessions_impl(&state, &peers, stranger, world_id)
            .await
            .expect_err("a non-member must not learn who is online");
        assert_eq!(err, PeerSignalingError::Forbidden);
    }

    /// A member's roster is the live registry minus themselves.
    #[tokio::test]
    async fn the_roster_query_answers_a_member_with_the_other_live_sessions() {
        let (state, world_id, a, b) = two_member_world();
        let peers = Arc::new(PeerRegistry::new());
        let (_ga, _rx_a) = peers.register(world_id, "sess-a".into(), a);

        assert!(
            peer_sessions_impl(&state, &peers, a, world_id)
                .await
                .unwrap()
                .is_empty(),
            "alone in a world, a member must be told there is nobody to dial"
        );

        let (_gb, _rx_b) = peers.register(world_id, "sess-b".into(), b);
        assert_eq!(
            peer_sessions_impl(&state, &peers, a, world_id)
                .await
                .unwrap(),
            vec!["sess-b".to_string()]
        );
    }
}
