//! Hotfix: an open subscription is counted while it is open, and released —
//! its count and its broadcast receiver both — however it ends.
//!
//! vtt-dev reported `subs_open=168` with six sockets attached. The number was
//! the cumulative count of subscriptions *ever opened*, printed under a label
//! that said "open". These tests pin the live gauge that replaces it to the
//! thing it claims to measure: every way a subscription can end must bring it
//! back down, and must give back the world channel's receiver, which is the
//! resource a leak would actually hold.
//!
//! The gauge is process-wide and other tests open subscriptions in parallel,
//! so the assertions read the per-thread mirror the guard keeps under
//! `cfg(test)`. A `#[tokio::test]` runs on one thread, so every subscription
//! a test opens is counted there and nowhere else.

use std::time::Duration;

use async_graphql::Request;
use async_graphql::http::{WebSocket, WebSocketProtocols as Protocols, WsMessage};
use futures_util::{Stream as _, StreamExt as _};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::auth::ClientDescription;
use crate::auth::sessions::issue_session_cookie;
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::subscription_metrics::open_on_this_thread;
use crate::models::WorldEvent;
use crate::state::AppState;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn schema(state: AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

async fn signed_in(state: &AppState, user_id: Uuid) -> AuthenticatedUser {
    let session = issue_session_cookie(
        state,
        &Cookies::default(),
        user_id,
        ClientDescription::unknown(),
    )
    .await
    .expect("a session");
    AuthenticatedUser {
        user_id,
        session_id: session.id,
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

/// A world with its owner signed in.
async fn world_with_owner() -> (AppState, Uuid, AuthenticatedUser) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    drop(conn);
    let caller = signed_in(&state, owner).await;
    (state, world, caller)
}

fn event(world_id: Uuid, by: Uuid, code: i32) -> WorldEvent {
    let now = chrono::Utc::now().naive_utc();
    WorldEvent {
        id: 0,
        world_id,
        event_code: code,
        token_event: None,
        created_at: now,
        schema_version: 1,
        updated_at: now,
        created_by: by,
        updated_by: by,
    }
}

fn world_events_query(world: Uuid) -> String {
    format!(r#"subscription {{ worldEventsCreated(worldId: "{world}") {{ eventCode }} }}"#)
}

/// Poll `stream` until the world's channel has `want` receivers.
async fn until_subscribed<S>(state: &AppState, world: Uuid, want: usize, stream: &mut S)
where
    S: futures_util::Stream + Unpin,
{
    let opened = async {
        while state.world_events.subscriber_count(world) < want {
            tokio::task::yield_now().await;
            let _ = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), opened)
        .await
        .expect("the subscription opens");
}

/// The socket going away: the stream is dropped with the connection that
/// owned it. Covers presence as well, the other counted subscription.
#[tokio::test]
async fn a_dropped_stream_is_no_longer_counted_open() {
    let (state, world, caller) = world_with_owner().await;
    let schema = schema(state.clone());
    let baseline = open_on_this_thread();

    let mut events =
        schema.execute_stream(Request::new(world_events_query(world)).data(caller.clone()));
    until_subscribed(&state, world, 1, &mut events).await;

    let mut presence = schema.execute_stream(
        Request::new(format!(
            r#"subscription {{ playersOnline(worldId: "{world}") {{ worldId }} }}"#
        ))
        .data(caller),
    );
    let opened = async {
        while open_on_this_thread() < baseline + 2 {
            let _ = tokio::time::timeout(Duration::from_millis(20), presence.next()).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), opened)
        .await
        .expect("the presence subscription opens");
    assert_eq!(open_on_this_thread(), baseline + 2);

    drop(events);
    drop(presence);

    assert_eq!(
        open_on_this_thread(),
        baseline,
        "a dropped subscription must not stay counted open"
    );
    assert_eq!(
        state.world_events.subscriber_count(world),
        0,
        "a dropped subscription must give its world channel's receiver back"
    );
}

/// A refused subscription is never counted open.
#[tokio::test]
async fn a_refused_subscription_is_not_counted_open() {
    let (state, _world, caller) = world_with_owner().await;
    let schema = schema(state.clone());
    let baseline = open_on_this_thread();

    let mut refused =
        schema.execute_stream(Request::new(world_events_query(Uuid::now_v7())).data(caller));
    let first = tokio::time::timeout(Duration::from_secs(5), refused.next())
        .await
        .expect("a refusal arrives")
        .expect("a refusal is an item");
    assert!(!first.errors.is_empty(), "a non-member is refused");
    assert_eq!(open_on_this_thread(), baseline);
}

/// A subscriber that falls behind the channel loses events but keeps its
/// subscription — and when it does end, it is released like any other.
#[tokio::test]
async fn a_lagged_subscription_stays_open_and_is_released_when_dropped() {
    let (state, world, caller) = world_with_owner().await;
    let schema = schema(state.clone());
    let baseline = open_on_this_thread();

    let mut events =
        schema.execute_stream(Request::new(world_events_query(world)).data(caller.clone()));
    until_subscribed(&state, world, 1, &mut events).await;

    // Far more than the channel holds, without reading: the receiver lags.
    let overflow = thunderforge_pg_sockets::WORLD_CHANNEL_CAPACITY * 2;
    for _ in 0..overflow {
        state
            .world_events
            .publish(world, event(world, caller.user_id, 999));
    }
    state
        .world_events
        .publish(world, event(world, caller.user_id, 1));

    let mut last = None;
    while let Ok(Some(response)) = tokio::time::timeout(Duration::from_secs(5), events.next()).await
    {
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let code = response.data.into_json().unwrap()["worldEventsCreated"]["eventCode"]
            .as_i64()
            .unwrap();
        last = Some(code);
        if code == 1 {
            break;
        }
    }
    assert_eq!(last, Some(1), "the stream carries on past the lag");
    assert_eq!(
        open_on_this_thread(),
        baseline + 1,
        "a lag does not close it"
    );

    drop(events);
    assert_eq!(open_on_this_thread(), baseline);
    assert_eq!(state.world_events.subscriber_count(world), 0);
}

/// Feed the protocol the way a browser's `graphql-ws` client does, through
/// the same `async_graphql::http::WebSocket` the axum handler serves.
struct Client {
    tx: Option<mpsc::UnboundedSender<String>>,
}

impl Client {
    fn send(&self, message: serde_json::Value) {
        self.tx
            .as_ref()
            .expect("the socket is open")
            .send(message.to_string())
            .expect("the server is reading");
    }
}

fn connect(
    schema: crate::graphql::AppSchema,
    caller: AuthenticatedUser,
) -> (Client, impl futures_util::Stream<Item = WsMessage> + Unpin) {
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    let socket = WebSocket::new(
        schema,
        UnboundedReceiverStream::new(rx),
        Protocols::GraphQLWS,
    )
    .on_connection_init(move |_| async move {
        let mut data = async_graphql::Data::default();
        data.insert(caller);
        Ok(data)
    });
    (Client { tx: Some(tx) }, Box::pin(socket))
}

/// Read server messages until one satisfies `want`.
async fn until_message<S>(socket: &mut S, want: impl Fn(&WsMessage) -> bool)
where
    S: futures_util::Stream<Item = WsMessage> + Unpin,
{
    let read = async {
        while let Some(message) = socket.next().await {
            if want(&message) {
                return;
            }
        }
        panic!("the socket ended before the expected message");
    };
    tokio::time::timeout(Duration::from_secs(5), read)
        .await
        .expect("the expected message arrives");
}

fn is_text_containing(needle: &'static str) -> impl Fn(&WsMessage) -> bool {
    move |message| matches!(message, WsMessage::Text(text) if text.contains(needle))
}

/// Open two subscriptions over one socket, then `complete` one of them: that
/// one is released at once, while the socket and the other stay up.
#[tokio::test]
async fn a_completed_subscription_is_released_while_its_socket_stays_open() {
    let (state, world, caller) = world_with_owner().await;
    let baseline = open_on_this_thread();
    let (client, mut socket) = connect(schema(state.clone()), caller.clone());

    client.send(serde_json::json!({ "type": "connection_init" }));
    until_message(&mut socket, is_text_containing("connection_ack")).await;

    for id in ["a", "b"] {
        client.send(serde_json::json!({
            "id": id,
            "type": "subscribe",
            "payload": { "query": world_events_query(world) },
        }));
    }
    until_subscribed(&state, world, 2, &mut socket).await;
    assert_eq!(open_on_this_thread(), baseline + 2);

    client.send(serde_json::json!({ "id": "a", "type": "complete" }));
    until_message(&mut socket, is_text_containing(r#""complete""#)).await;

    assert_eq!(
        open_on_this_thread(),
        baseline + 1,
        "a completed subscription must not stay counted open"
    );
    assert_eq!(state.world_events.subscriber_count(world), 1);

    // The other is still live on the same socket.
    state
        .world_events
        .publish(world, event(world, caller.user_id, 7));
    until_message(&mut socket, is_text_containing(r#""eventCode":7"#)).await;

    drop(client);
    drop(socket);
    assert_eq!(open_on_this_thread(), baseline);
    assert_eq!(state.world_events.subscriber_count(world), 0);
}

/// The client goes away without completing anything: the socket's input
/// ends, the protocol stream ends, and everything it held is released.
#[tokio::test]
async fn a_closed_socket_releases_every_subscription_on_it() {
    let (state, world, caller) = world_with_owner().await;
    let baseline = open_on_this_thread();
    let (mut client, mut socket) = connect(schema(state.clone()), caller);

    client.send(serde_json::json!({ "type": "connection_init" }));
    until_message(&mut socket, is_text_containing("connection_ack")).await;
    for id in ["a", "b", "c"] {
        client.send(serde_json::json!({
            "id": id,
            "type": "subscribe",
            "payload": { "query": world_events_query(world) },
        }));
    }
    until_subscribed(&state, world, 3, &mut socket).await;
    assert_eq!(open_on_this_thread(), baseline + 3);

    // The socket closes. What the axum handler does next is run the protocol
    // stream to its end and drop it, which is exactly this.
    client.tx = None;
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        while socket.next().await.is_some() {}
    })
    .await;
    assert!(ended.is_ok(), "a closed socket ends the protocol stream");
    drop(socket);

    assert_eq!(open_on_this_thread(), baseline);
    assert_eq!(state.world_events.subscriber_count(world), 0);
}

/// A subscription that panics while being polled is still released: the
/// guard lives in the stream, and unwinding drops the stream.
#[test]
fn a_panicking_subscription_is_still_released() {
    use crate::graphql::subscription_metrics::OpenSubscription;

    let baseline = open_on_this_thread();
    let held = OpenSubscription::begin();
    let mut stream = held.hold(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<()>> { panic!("a resolver bug") },
    ));
    assert_eq!(open_on_this_thread(), baseline + 1);

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let waker = futures_util::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        let _ = std::pin::Pin::new(&mut stream).poll_next(&mut cx);
    }));
    assert!(panicked.is_err());
    assert_eq!(open_on_this_thread(), baseline);
}
