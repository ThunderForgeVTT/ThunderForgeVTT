//! Spec 081 T011: a GM only roll does not reach a player's live stream, not
//! even as an id; the GM's stream carries all three, and every other event
//! passes untouched.
//!
//! Driven through `execute_stream` on the real schema, with events published
//! straight into the world's channel, so what is proven is what a subscriber
//! receives rather than what a helper returns.

use std::time::Duration;

use async_graphql::Request;
use futures_util::StreamExt as _;
use serde_json::Value;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::auth::ClientDescription;
use crate::auth::sessions::issue_session_cookie;
use crate::auth_middleware::AuthenticatedUser;
use crate::models::WorldEvent;
use crate::rolls::visibility::Visibility;
use crate::state::AppState;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};
use crate::world_events::{
    EVENT_CODE_ROLL_MADE, EVENT_CODE_ROLL_REVEALED, EVENT_CODE_ROLLS_CLEARED, roll_event_payload,
};

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

fn event(world_id: Uuid, by: Uuid, code: i32, payload: Option<Value>) -> WorldEvent {
    let now = chrono::Utc::now().naive_utc();
    WorldEvent {
        id: 0,
        world_id,
        event_code: code,
        token_event: payload,
        created_at: now,
        schema_version: 1,
        updated_at: now,
        created_by: by,
        updated_by: by,
    }
}

/// The event codes and visibilities one subscriber receives after the
/// sequence below has been published, ending at the marker.
async fn received_by(as_gm: bool) -> Vec<(i64, Option<String>)> {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, player, "Player");
    drop(conn);

    let watcher = signed_in(&state, if as_gm { owner } else { player }).await;
    let schema = schema(state.clone());
    let mut stream = schema.execute_stream(
        Request::new(format!(
            r#"subscription {{ worldEventsCreated(worldId: "{world}") {{ eventCode tokenEvent }} }}"#
        ))
        .data(watcher),
    );

    // The subscription registers its receiver when first polled.
    let opened = async {
        while state.world_events.subscriber_count(world) == 0 {
            tokio::task::yield_now().await;
            let _ = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), opened)
        .await
        .expect("the stream subscribes");

    let roll = |v: Visibility| Some(roll_event_payload(Uuid::now_v7(), v));
    for e in [
        event(
            world,
            player,
            EVENT_CODE_ROLL_MADE,
            roll(Visibility::Everyone),
        ),
        event(
            world,
            player,
            EVENT_CODE_ROLL_MADE,
            roll(Visibility::GmEyes),
        ),
        event(world, owner, EVENT_CODE_ROLL_MADE, roll(Visibility::GmOnly)),
        event(
            world,
            owner,
            EVENT_CODE_ROLL_REVEALED,
            roll(Visibility::GmOnly),
        ),
        event(world, owner, 29, None),
        event(world, owner, 999, None),
    ] {
        state.world_events.publish(world, e);
    }

    let mut seen = Vec::new();
    while let Ok(Some(response)) = tokio::time::timeout(Duration::from_secs(5), stream.next()).await
    {
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let ev = &data["worldEventsCreated"];
        let code = ev["eventCode"].as_i64().unwrap();
        let visibility = ev["tokenEvent"]["visibility"].as_str().map(str::to_string);
        seen.push((code, visibility));
        if code == 999 {
            break;
        }
    }
    seen
}

fn codes(seen: &[(i64, Option<String>)]) -> Vec<(i64, Option<&str>)> {
    seen.iter().map(|(c, v)| (*c, v.as_deref())).collect()
}

#[tokio::test]
async fn a_player_never_receives_a_gm_only_roll() {
    let seen = received_by(false).await;
    assert_eq!(
        codes(&seen),
        vec![
            (36, Some("everyone")),
            (36, Some("gm_eyes")),
            // The reveal is the moment the table may see it.
            (37, Some("gm_only")),
            (29, None),
            (999, None),
        ]
    );
}

#[tokio::test]
async fn the_gm_receives_every_roll() {
    let seen = received_by(true).await;
    assert_eq!(
        codes(&seen),
        vec![
            (36, Some("everyone")),
            (36, Some("gm_eyes")),
            (36, Some("gm_only")),
            (37, Some("gm_only")),
            (29, None),
            (999, None),
        ]
    );
}

/// Spec 088 T062 (FR-042): once the GM has cleared the feed, a roll event
/// recorded before the clear is not delivered, even replayed afterwards, to a
/// player or to the GM. One recorded after it is, and so is the clear itself.
async fn received_after_a_clear(as_gm: bool) -> Vec<(i64, Option<String>)> {
    use crate::schema::worlds;
    use diesel::prelude::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, player, "Player");
    // At the database's precision, so "the very instant" below is the same
    // instant once stored.
    let cleared_at = chrono::SubsecRound::trunc_subsecs(chrono::Utc::now(), 6);
    diesel::update(worlds::table.find(world))
        .set(worlds::rolls_cleared_at.eq(Some(cleared_at)))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let watcher = signed_in(&state, if as_gm { owner } else { player }).await;
    let schema = schema(state.clone());
    let mut stream = schema.execute_stream(
        Request::new(format!(
            r#"subscription {{ worldEventsCreated(worldId: "{world}") {{ eventCode tokenEvent }} }}"#
        ))
        .data(watcher),
    );
    let opened = async {
        while state.world_events.subscriber_count(world) == 0 {
            tokio::task::yield_now().await;
            let _ = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), opened)
        .await
        .expect("the stream subscribes");

    let at = |e: WorldEvent, when: chrono::DateTime<chrono::Utc>| WorldEvent {
        created_at: when.naive_utc(),
        ..e
    };
    let before = cleared_at - chrono::Duration::seconds(1);
    let later = cleared_at + chrono::Duration::seconds(1);
    let roll = |v: Visibility| Some(roll_event_payload(Uuid::now_v7(), v));
    let cleared = Some(serde_json::json!({ "clearedAt": cleared_at.to_rfc3339() }));
    for e in [
        // Replayed after the clear, recorded before it.
        at(
            event(
                world,
                player,
                EVENT_CODE_ROLL_MADE,
                roll(Visibility::Everyone),
            ),
            before,
        ),
        at(
            event(
                world,
                owner,
                EVENT_CODE_ROLL_REVEALED,
                roll(Visibility::GmOnly),
            ),
            before,
        ),
        // At the very instant of the clear: cleared too.
        at(
            event(
                world,
                player,
                EVENT_CODE_ROLL_MADE,
                roll(Visibility::Everyone),
            ),
            cleared_at,
        ),
        at(
            event(world, owner, EVENT_CODE_ROLLS_CLEARED, cleared),
            cleared_at,
        ),
        at(
            event(
                world,
                player,
                EVENT_CODE_ROLL_MADE,
                roll(Visibility::GmEyes),
            ),
            later,
        ),
        event(world, owner, 999, None),
    ] {
        state.world_events.publish(world, e);
    }

    let mut seen = Vec::new();
    while let Ok(Some(response)) = tokio::time::timeout(Duration::from_secs(5), stream.next()).await
    {
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let ev = &data["worldEventsCreated"];
        let code = ev["eventCode"].as_i64().unwrap();
        let visibility = ev["tokenEvent"]["visibility"].as_str().map(str::to_string);
        seen.push((code, visibility));
        if code == 999 {
            break;
        }
    }
    seen
}

#[tokio::test]
async fn no_one_receives_a_roll_event_from_before_the_clear() {
    for as_gm in [false, true] {
        let seen = received_after_a_clear(as_gm).await;
        assert_eq!(
            codes(&seen),
            vec![(39, None), (36, Some("gm_eyes")), (999, None)],
            "as the GM: {as_gm}"
        );
    }
}
