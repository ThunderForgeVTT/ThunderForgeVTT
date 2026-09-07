//! Who is reachable right now, and for exactly how long.
//!
//! The map, its guard, and the one process-wide instance of it. Nothing here
//! decides *whether* a client may be in the map — [`super::admission`] does
//! that — and nothing here touches GraphQL beyond the message type the
//! channels carry.
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

use async_graphql::SimpleObject;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use uuid::Uuid;

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

    pub(super) fn unregister(&self, world_id: Uuid, session_id: &str, token: u64) {
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

/// Is this a session id the server is willing to hold as a map key?
pub(super) fn valid_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_SESSION_ID_LEN
}

#[cfg(test)]
mod tests {
    //! Pure — no database, no schema, the way `WorldRouter`'s are.

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
}
