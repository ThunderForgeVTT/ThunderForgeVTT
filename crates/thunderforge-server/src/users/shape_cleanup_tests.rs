//! Deleting the account of a player who drew in somebody else's world
//! (spec 082 R9): their drawings go with them, and the board is told.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::graphql::mutations_shapes::tests::players_draw::{draw, table};
use crate::schema::{shapes, world_events};
use crate::users::delete_user_data_on;
use crate::world_events::EVENT_CODE_SHAPE_CHANGED;

#[tokio::test]
async fn a_player_who_drew_deletes_their_account_and_their_drawings_go() {
    let t = table();
    let gm_shape = draw(&t, t.owner, true).await;
    let mut theirs = vec![draw(&t, t.a, true).await, draw(&t, t.a, true).await];
    let b_shape = draw(&t, t.b, true).await;
    let mut conn = t.state.db_pool.get().unwrap();
    // A shape they only edited, back when somebody could.
    diesel::update(shapes::table.find(gm_shape))
        .set(shapes::updated_by.eq(t.a))
        .execute(&mut conn)
        .unwrap();

    let summary = delete_user_data_on(&mut conn, t.a).expect("the deletion succeeds");
    assert_eq!(summary.users_deleted, 1);
    assert_eq!(summary.shapes_deleted, 2);

    let left: Vec<(Uuid, Uuid, Uuid)> = shapes::table
        .filter(shapes::scene_id.eq(t.scene))
        .select((shapes::shape_id, shapes::created_by, shapes::updated_by))
        .order(shapes::shape_id)
        .load(&mut conn)
        .unwrap();
    let mut expected = vec![(gm_shape, t.owner, t.owner), (b_shape, t.b, t.b)];
    expected.sort();
    assert_eq!(
        left, expected,
        "the edited shape keeps its creator as editor"
    );

    let told: Vec<(Option<Value>, Uuid)> = world_events::table
        .filter(world_events::world_id.eq(t.world))
        .filter(world_events::event_code.eq(EVENT_CODE_SHAPE_CHANGED))
        .select((world_events::token_event, world_events::created_by))
        .order(world_events::id)
        .load(&mut conn)
        .unwrap();
    let mut deleted: Vec<Uuid> = told
        .iter()
        .filter(|(payload, _)| payload.as_ref().unwrap()["action"] == json!("deleted"))
        .map(|(payload, by)| {
            assert_eq!(*by, t.owner, "told by the world's owner");
            payload.as_ref().unwrap()["shape_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    deleted.sort();
    theirs.sort();
    assert_eq!(deleted, theirs, "one deleted event per drawing");
}

#[tokio::test]
async fn an_account_that_never_drew_deletes_nothing_of_anyone_elses() {
    let t = table();
    let gm_shape = draw(&t, t.owner, true).await;
    let mut conn = t.state.db_pool.get().unwrap();

    let summary = delete_user_data_on(&mut conn, t.b).unwrap();
    assert_eq!(summary.shapes_deleted, 0);
    let left: Vec<Uuid> = shapes::table
        .filter(shapes::scene_id.eq(t.scene))
        .select(shapes::shape_id)
        .load(&mut conn)
        .unwrap();
    assert_eq!(left, vec![gm_shape]);
}
