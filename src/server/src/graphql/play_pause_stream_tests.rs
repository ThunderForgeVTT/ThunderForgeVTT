//! Spec 051 T021: the four world-scoped subscriptions, against a paused world.
//!
//! Driven through `execute_stream` on the real schema, because "a paused world
//! never opens a stream" is a claim about what a client gets back from
//! subscribing, and `session_lifetime`'s own tests already prove the wrapper
//! in isolation.

use std::time::Duration;

use async_graphql::Request;
use futures_util::StreamExt as _;
use serde_json::Value;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::auth::ClientDescription;
use crate::auth::sessions::issue_session_cookie;
use crate::auth_middleware::AuthenticatedUser;
use crate::play_pause::{TriggerDetail, pause_world};
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

fn member(user_id: Uuid, session_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id,
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

/// A world, its Owner signed in, and an operator.
struct Table {
    state: AppState,
    operator: Uuid,
    owner: Uuid,
    session: Uuid,
    world: Uuid,
}

async fn a_table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let operator = insert_test_user(&mut conn);
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    drop(conn);
    let session = issue_session_cookie(
        &state,
        &Cookies::default(),
        owner,
        ClientDescription::unknown(),
    )
    .await
    .expect("a session");
    Table {
        state,
        operator,
        owner,
        session: session.id,
        world,
    }
}

fn pause(t: &Table) {
    let mut conn = t.state.db_pool.get().unwrap();
    pause_world(
        &mut conn,
        t.operator,
        t.world,
        "Stopping play.",
        TriggerDetail::operator(),
    )
    .expect("paused");
}

/// Subscribe as the Owner of a paused world, and assert the first thing back
/// is the pause, and the stream ends there.
async fn refused_on_opening(document: impl FnOnce(Uuid) -> String) {
    let t = a_table().await;
    pause(&t);

    let schema = schema(t.state.clone());
    let mut stream =
        schema.execute_stream(Request::new(document(t.world)).data(member(t.owner, t.session)));

    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("a refusal arrives at once, not on a tick")
        .expect("one response");
    let first = serde_json::to_value(&first).unwrap();
    assert_eq!(
        first["errors"][0]["extensions"]["code"], "WORLD_PLAY_PAUSED",
        "a paused world refuses with its code: {first}"
    );
    assert_eq!(
        first["errors"][0]["extensions"]["worldId"],
        Value::String(t.world.to_string())
    );

    let rest = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("and the stream ends");
    assert!(rest.is_none(), "nothing follows the refusal");
}

#[tokio::test]
async fn world_events_created_refuses_to_open_on_a_paused_world() {
    refused_on_opening(|world| {
        format!(r#"subscription {{ worldEventsCreated(worldId: "{world}") {{ __typename }} }}"#)
    })
    .await;
}

#[tokio::test]
async fn players_online_refuses_to_open_on_a_paused_world() {
    refused_on_opening(|world| {
        format!(r#"subscription {{ playersOnline(worldId: "{world}") {{ __typename }} }}"#)
    })
    .await;
}

#[tokio::test]
async fn play_field_refuses_to_open_on_a_paused_world() {
    refused_on_opening(|world| {
        format!(
            r#"subscription {{ playField(worldId: "{world}", clientId: "window-a") {{ __typename }} }}"#
        )
    })
    .await;
}

#[tokio::test]
async fn peer_signals_refuses_to_open_on_a_paused_world() {
    refused_on_opening(|world| {
        format!(
            r#"subscription {{ peerSignals(worldId: "{world}", sessionId: "sess-a") {{ __typename }} }}"#
        )
    })
    .await;
}

/// FR-020, SC-001: a stream already open when the world is paused yields the
/// pause and ends, on its own. `playField` because it answers at once, so the
/// test can see the stream was open before the pause.
#[tokio::test]
async fn an_open_play_field_stream_ends_with_the_pause_error() {
    let t = a_table().await;
    let schema = schema(t.state.clone());
    let mut stream = schema.execute_stream(
        Request::new(format!(
            r#"subscription {{ playField(worldId: "{}", clientId: "window-a") {{ clientId isMine }} }}"#,
            t.world
        ))
        .data(member(t.owner, t.session)),
    );

    let opened = stream.next().await.expect("the claim as it stands");
    assert!(opened.errors.is_empty(), "{:?}", opened.errors);

    pause(&t);

    // One liveness tick, plus the moment its query takes.
    let ended = tokio::time::timeout(Duration::from_secs(8), async {
        let mut responses = Vec::new();
        while let Some(response) = stream.next().await {
            responses.push(serde_json::to_value(&response).unwrap());
        }
        responses
    })
    .await
    .expect("a paused world's stream ends on its own within a tick");

    let errors: Vec<&Value> = ended
        .iter()
        .filter(|response| response.get("errors").is_some())
        .collect();
    let [error] = errors.as_slice() else {
        panic!("exactly one error response expected, got {ended:?}");
    };
    assert_eq!(
        error["errors"][0]["extensions"]["code"],
        "WORLD_PLAY_PAUSED"
    );
}
