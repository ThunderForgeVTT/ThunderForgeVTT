//! Spec 051 T019: `pauseWorldPlay`, through the schema.

use async_graphql::Request;
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::schema::world_play_pauses;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn caller(user_id: Uuid, is_admin: bool) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin,
        role: if is_admin { "Admin" } else { "User" }.to_string(),
        disabled: false,
    }
}

fn pause(world: Uuid, grounds: &str) -> String {
    format!(
        r#"mutation {{ pauseWorldPlay(worldId: "{world}", grounds: "{grounds}") {{
            alreadyPaused
            pause {{ id worldId grounds pausedBy {{ id }} liftedAt triggers {{ kind note }} }}
        }} }}"#
    )
}

fn code(response: &async_graphql::Response) -> Value {
    serde_json::to_value(response).unwrap()["errors"][0]["extensions"]["code"].clone()
}

fn pauses_of(state: &crate::state::AppState, world: Uuid) -> i64 {
    let mut conn = state.db_pool.get().unwrap();
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// FR-001, FR-036: an operator pauses; a second operator's pause of the same
/// world adds to it rather than making another.
#[tokio::test]
async fn an_operator_pauses_a_world_and_a_second_pause_adds_to_it() {
    let state = test_app_state();
    let (first, second, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let first = insert_test_user(&mut conn);
        let second = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        (first, second, insert_test_world(&mut conn, owner))
    };
    let schema = schema(state.clone());

    let made = schema
        .execute(Request::new(pause(world, "Abuse at this table.")).data(caller(first, true)))
        .await;
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    let made = made.data.into_json().unwrap()["pauseWorldPlay"].clone();
    assert_eq!(made["alreadyPaused"], false);
    assert_eq!(made["pause"]["worldId"], world.to_string());
    assert_eq!(made["pause"]["grounds"], "Abuse at this table.");
    assert_eq!(made["pause"]["pausedBy"]["id"], first.to_string());
    assert!(made["pause"]["liftedAt"].is_null());
    assert_eq!(made["pause"]["triggers"][0]["kind"], "OPERATOR");

    let again = schema
        .execute(Request::new(pause(world, "Also reported.")).data(caller(second, true)))
        .await;
    assert!(again.errors.is_empty(), "{:?}", again.errors);
    let again = again.data.into_json().unwrap()["pauseWorldPlay"].clone();
    assert_eq!(again["alreadyPaused"], true);
    assert_eq!(again["pause"]["id"], made["pause"]["id"], "the same pause");
    assert_eq!(again["pause"]["triggers"][1]["note"], "Also reported.");
    assert_eq!(pauses_of(&state, world), 1);
}

/// FR-004 and the contract's refusal table.
#[tokio::test]
async fn blank_grounds_and_a_missing_world_are_refused_with_their_codes() {
    let state = test_app_state();
    let (operator, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let operator = insert_test_user(&mut conn);
        (operator, insert_test_world(&mut conn, operator))
    };
    let schema = schema(state.clone());

    let blank = schema
        .execute(Request::new(pause(world, "   ")).data(caller(operator, true)))
        .await;
    assert_eq!(code(&blank), "GROUNDS_REQUIRED");
    assert_eq!(pauses_of(&state, world), 0);

    let missing = schema
        .execute(Request::new(pause(Uuid::now_v7(), "Grounds.")).data(caller(operator, true)))
        .await;
    assert_eq!(code(&missing), "WORLD_NOT_FOUND");
}

/// FR-006, FR-040: the world's own Owner is not an operator.
#[tokio::test]
async fn a_worlds_owner_cannot_pause_it() {
    let state = test_app_state();
    let (owner, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        (owner, insert_test_world(&mut conn, owner))
    };

    let refused = schema(state.clone())
        .execute(Request::new(pause(world, "Mine to stop.")).data(caller(owner, false)))
        .await;
    assert!(!refused.errors.is_empty());
    assert_eq!(pauses_of(&state, world), 0, "and nothing was written");
}
