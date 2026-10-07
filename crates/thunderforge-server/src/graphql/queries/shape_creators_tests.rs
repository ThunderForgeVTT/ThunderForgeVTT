//! `shapeCreators` (spec 082 US4): who drew on a scene, for the Game
//! Master's "clear a player's shapes" picker.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::graphql::mutations_shapes::tests::players_draw::{Table, data, draw, run, table};
use crate::schema::{users, world_members};

const CREATORS: &str = r#"query ($scene: UUID!) {
    shapeCreators(sceneId: $scene) { userId displayName isMember shapeCount }
}"#;

fn username(t: &Table, user: Uuid) -> String {
    users::table
        .find(user)
        .select(users::username)
        .first(&mut t.state.db_pool.get().unwrap())
        .unwrap()
}

#[tokio::test]
async fn lists_each_player_who_drew_with_a_count_and_leaves_out_the_gm() {
    let t = table();
    draw(&t, t.owner, false).await;
    draw(&t, t.a, true).await;
    draw(&t, t.a, true).await;
    draw(&t, t.b, true).await;
    // B leaves the world; their drawing stays, and so do they in the list.
    diesel::delete(
        world_members::table
            .filter(world_members::world_id.eq(t.world))
            .filter(world_members::user_id.eq(t.b)),
    )
    .execute(&mut t.state.db_pool.get().unwrap())
    .unwrap();

    let answer = data(
        run(
            &t.state,
            t.owner,
            false,
            CREATORS,
            json!({ "scene": t.scene }),
        )
        .await,
    );
    let mut expected = vec![
        json!({ "userId": t.a, "displayName": username(&t, t.a), "isMember": true, "shapeCount": 2 }),
        json!({ "userId": t.b, "displayName": username(&t, t.b), "isMember": false, "shapeCount": 1 }),
    ];
    expected.sort_by_key(|creator| creator["displayName"].as_str().unwrap().to_owned());
    assert_eq!(answer["shapeCreators"], Value::Array(expected));
}

#[tokio::test]
async fn a_player_is_refused() {
    let t = table();
    draw(&t, t.a, true).await;

    let refused = run(&t.state, t.a, false, CREATORS, json!({ "scene": t.scene })).await;
    assert!(!refused.errors.is_empty(), "a player read the creators");
}
