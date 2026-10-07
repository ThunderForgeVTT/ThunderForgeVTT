//! `clearShapes` (spec 082 US3/US4): the Game Master clears a scene's
//! drawings, all of them or only some players'.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::graphql::mutations_shapes::tests::players_draw::{Table, data, draw, run, table};
use crate::schema::{shapes, world_events};
use crate::test_support::insert_test_scene_named;
use crate::world_events::EVENT_CODE_SHAPE_CHANGED;

const CLEAR: &str = r#"mutation ($scene: UUID!, $createdBy: [UUID!]) {
    clearShapes(sceneId: $scene, createdBy: $createdBy)
}"#;

/// Draws a rectangle on `scene`, on `level` when given, as `user`.
async fn draw_on(t: &Table, user: Uuid, scene: Uuid, level: Option<Uuid>) -> Uuid {
    let answer = data(
        run(
            &t.state,
            user,
            false,
            r#"mutation ($input: GraphQLCreateShapeInput!) {
                createShape(input: $input) { shapeId }
            }"#,
            json!({ "input": {
                "sceneId": scene, "levelId": level, "kind": "RECT",
                "geometry": { "x": 1, "y": 2, "w": 3, "h": 4 },
                "visibleToPlayers": true,
            }}),
        )
        .await,
    );
    answer["createShape"]["shapeId"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// Every shape on `scene`, as stored, in a stable order.
fn rows(t: &Table, scene: Uuid) -> Vec<(Uuid, Value, bool, Uuid, Uuid)> {
    shapes::table
        .filter(shapes::scene_id.eq(scene))
        .select((
            shapes::shape_id,
            shapes::geometry,
            shapes::visible_to_players,
            shapes::created_by,
            shapes::updated_by,
        ))
        .order(shapes::shape_id)
        .load(&mut t.state.db_pool.get().unwrap())
        .unwrap()
}

/// The shapes the world was told were deleted, in the order it was told.
fn deleted_events(t: &Table) -> Vec<Uuid> {
    world_events::table
        .filter(world_events::world_id.eq(t.world))
        .filter(world_events::event_code.eq(EVENT_CODE_SHAPE_CHANGED))
        .select(world_events::token_event)
        .order(world_events::id)
        .load::<Option<Value>>(&mut t.state.db_pool.get().unwrap())
        .unwrap()
        .into_iter()
        .flatten()
        .filter(|payload| payload["action"] == json!("deleted"))
        .map(|payload| payload["shape_id"].as_str().unwrap().parse().unwrap())
        .collect()
}

async fn clear(t: &Table, user: Uuid, created_by: Option<Vec<Uuid>>) -> async_graphql::Response {
    run(
        &t.state,
        user,
        false,
        CLEAR,
        json!({ "scene": t.scene, "createdBy": created_by }),
    )
    .await
}

#[tokio::test]
async fn the_gm_clears_every_shape_on_every_level_and_nothing_else() {
    let t = table();
    let upstairs = data(
        run(
            &t.state,
            t.owner,
            false,
            r#"mutation ($scene: UUID!) {
                createSceneLevel(input: { sceneId: $scene, name: "Upstairs" }) { levelId }
            }"#,
            json!({ "scene": t.scene }),
        )
        .await,
    )["createSceneLevel"]["levelId"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let mut drawn = vec![
        draw(&t, t.owner, false).await,
        draw(&t, t.a, true).await,
        draw_on(&t, t.b, t.scene, Some(upstairs)).await,
    ];
    let elsewhere = insert_test_scene_named(
        &mut t.state.db_pool.get().unwrap(),
        t.world,
        t.owner,
        "Elsewhere",
    );
    let kept = draw_on(&t, t.owner, elsewhere, None).await;

    let cleared = data(clear(&t, t.owner, None).await);
    assert_eq!(cleared["clearShapes"], json!(3));
    assert!(rows(&t, t.scene).is_empty(), "every level is cleared");
    assert_eq!(
        rows(&t, elsewhere)
            .iter()
            .map(|row| row.0)
            .collect::<Vec<_>>(),
        vec![kept],
        "another scene is untouched"
    );

    let mut told = deleted_events(&t);
    told.sort();
    drawn.sort();
    assert_eq!(told, drawn, "one deleted event per shape");
}

#[tokio::test]
async fn a_player_cannot_clear_and_nothing_is_deleted() {
    let t = table();
    draw(&t, t.owner, true).await;
    draw(&t, t.a, true).await;
    let before = rows(&t, t.scene);

    for created_by in [None, Some(vec![t.a])] {
        let refused = clear(&t, t.a, created_by).await;
        assert!(!refused.errors.is_empty(), "a player cleared the scene");
    }
    assert_eq!(rows(&t, t.scene), before);
    assert!(deleted_events(&t).is_empty());
}

#[tokio::test]
async fn the_gm_clears_one_players_shapes_and_leaves_the_rest_byte_for_byte() {
    let t = table();
    let gm_shape = draw(&t, t.owner, false).await;
    let a_shapes = vec![draw(&t, t.a, true).await, draw(&t, t.a, true).await];
    let b_shape = draw(&t, t.b, true).await;
    let kept_before: Vec<_> = rows(&t, t.scene)
        .into_iter()
        .filter(|row| row.0 == gm_shape || row.0 == b_shape)
        .collect();

    let cleared = data(clear(&t, t.owner, Some(vec![t.a])).await);
    assert_eq!(cleared["clearShapes"], json!(2));
    assert_eq!(
        rows(&t, t.scene),
        kept_before,
        "SC-004: the rest are untouched"
    );

    let mut told = deleted_events(&t);
    told.sort();
    let mut expected = a_shapes;
    expected.sort();
    assert_eq!(told, expected, "an event for A's shapes only");
}

#[tokio::test]
async fn an_empty_creator_list_clears_nothing() {
    let t = table();
    draw(&t, t.owner, false).await;
    draw(&t, t.a, true).await;
    let before = rows(&t, t.scene);

    let cleared = data(clear(&t, t.owner, Some(vec![])).await);
    assert_eq!(cleared["clearShapes"], json!(0));
    assert_eq!(rows(&t, t.scene), before);
    assert!(deleted_events(&t).is_empty());
}
