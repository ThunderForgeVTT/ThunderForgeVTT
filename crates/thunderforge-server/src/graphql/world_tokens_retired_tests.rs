//! The world-scoped token routes are gone, so nobody outside a world can place
//! a token in it through them.
//!
//! # Why this exists
//!
//! `createWorldToken` and `upsertWorldToken` checked that the caller was signed
//! in and that the world was not paused — and nothing else. Any account could
//! write a token into any world, and `upsertWorldToken` then broadcast the
//! token's label to every seat, past the Game Master's hidden-name rule. The
//! table behind them had been retired by ADR-040 and no client called them, so
//! the routes and the table were removed rather than repaired.
//!
//! These tests hold that shut: the fields are absent from the schema, a
//! signed-in non-member who sends the old documents is refused at validation
//! and writes nothing, and the live route that replaced them (`createToken`)
//! refuses the same person.

use async_graphql::Request;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, test_app_state};

/// Every root field the world-scoped token table was reached through.
const RETIRED: &[(&str, &str)] = &[
    (
        "createWorldToken",
        r#"mutation { createWorldToken(input: { worldId: "{world}", label: "Intruder" }) { id } }"#,
    ),
    (
        "upsertWorldToken",
        r#"mutation { upsertWorldToken(input: { worldId: "{world}", label: "Intruder", x: 1 }) { id } }"#,
    ),
    (
        "moveToken",
        r#"mutation { moveToken(input: { tokenId: "t", x: 1, y: 1 }) { id } }"#,
    ),
    (
        "deleteWorldToken",
        r#"mutation { deleteWorldToken(tokenId: "t") }"#,
    ),
    ("myWorldTokens", r#"query { myWorldTokens { id } }"#),
    ("worldToken", r#"query { worldToken(tokenId: "t") { id } }"#),
];

fn schema(state: AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn signed_in(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

fn world_writes(conn: &mut PgConnection, world: Uuid) -> (i64, i64) {
    use crate::schema::{scenes, tokens, world_events};
    let events = world_events::table
        .filter(world_events::world_id.eq(world))
        .count()
        .get_result::<i64>(conn)
        .unwrap();
    let placed = tokens::table
        .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
        .filter(scenes::world_id.eq(world))
        .count()
        .get_result::<i64>(conn)
        .unwrap();
    (events, placed)
}

#[test]
fn the_world_token_fields_are_not_in_the_schema() {
    let sdl = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl();
    for (field, _) in RETIRED {
        assert!(
            !sdl.contains(&format!("\t{field}(")) && !sdl.contains(&format!("\t{field}:")),
            "`{field}` is back in the schema"
        );
    }
    assert!(
        !sdl.contains("GraphQLWorldToken"),
        "the retired type is back"
    );
}

#[tokio::test]
async fn a_non_member_cannot_reach_a_world_through_the_retired_routes() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let outsider = insert_test_user(&mut conn);
    let before = world_writes(&mut conn, world);

    let schema = schema(state.clone());
    for (field, template) in RETIRED {
        let document = template.replace("{world}", &world.to_string());
        let response = schema
            .execute(Request::new(document).data(signed_in(outsider)))
            .await;
        let json = serde_json::to_value(&response).unwrap();
        let refused_at_validation = response
            .errors
            .iter()
            .any(|error| error.message.contains("Unknown field") && error.message.contains(field));
        assert!(
            refused_at_validation,
            "`{field}` must not resolve for anyone: {json}"
        );
        assert!(
            json["data"].is_null() || json.get("data").is_none(),
            "`{field}` answered data: {json}"
        );
    }

    assert_eq!(
        world_writes(&mut conn, world),
        before,
        "a refused request wrote into the world"
    );
}

/// The live route is the one the retired ones were replaced by; it must refuse
/// the same outsider, or removing the old ones closed nothing.
#[tokio::test]
async fn a_non_member_cannot_place_a_token_through_the_live_route() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);
    let outsider = insert_test_user(&mut conn);
    let before = world_writes(&mut conn, world);

    let document = format!(
        r#"mutation {{ createToken(input: {{ sceneId: "{scene}", x: 1, y: 1 }}) {{ tokenId }} }}"#
    );
    let response = schema(state.clone())
        .execute(Request::new(document).data(signed_in(outsider)))
        .await;
    assert!(
        !response.errors.is_empty(),
        "an outsider placed a token: {:?}",
        response.data
    );
    assert_eq!(world_writes(&mut conn, world), before);
}
