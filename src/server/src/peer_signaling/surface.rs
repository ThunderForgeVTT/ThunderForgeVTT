//! The GraphQL surface: one mutation, one query, one subscription.
//!
//! The resolvers and the `_impl` functions behind them. They apply the gates
//! in [`super::admission`] and then do the only two things this feature does
//! — hand one opaque string to one session, and say who is reachable.
//!
//! # Why the roster is a query and not a push
//!
//! [`peer_sessions`] answers "who else is here right now" on demand. A
//! newcomer asks once and initiates to everybody it finds; nobody needs a
//! join notification, because the newcomer is always the initiator, and a
//! departure is noticed when the data channel closes. Keeping it a pull keeps
//! the server a post box instead of quietly becoming a presence service with
//! a second, divergent notion of who is online.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use uuid::Uuid;

use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;

use super::admission::{
    CLAIM_GRACE, PeerSignalingError, is_member, register_if_play_field, to_graphql_error,
};
use super::registry::{GraphQLPeerSignal, PeerRegistry, registry, valid_session_id};

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

    // And spec 036 FR-038: reachability belongs to the play field. A companion
    // surface never enters the registry, so it is never addressable.
    let registration = register_if_play_field(
        registry(),
        crate::play_field::registry(),
        auth_user.user_id,
        world_id,
        session_id,
        CLAIM_GRACE,
    )
    .await;
    let (guard, rx) = match registration {
        Ok(pair) => pair,
        Err(e) => return failure(&e.to_string()),
    };

    Box::pin(futures_util::stream::unfold(
        (rx, guard),
        |(mut rx, guard)| async move { rx.recv().await.map(|signal| (Ok(signal), (rx, guard))) },
    ))
}

#[cfg(test)]
mod tests {
    //! These need a real Postgres (`DATABASE_URL`): "membership is re-checked
    //! per signal" is a claim about the database, and mocking it would test
    //! the mock.

    use std::sync::Arc;

    use super::super::admission::NOT_A_MEMBER;
    use super::super::registry::MAX_SESSION_ID_LEN;
    use super::*;
    use crate::test_support::*;

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
