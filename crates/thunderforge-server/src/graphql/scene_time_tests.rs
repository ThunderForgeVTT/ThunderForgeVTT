use super::set_scene_time_of_day_impl;
use crate::schema::world_events;
use crate::test_support::*;
use diesel::prelude::*;

#[tokio::test]
async fn only_the_dm_sets_the_time_and_every_client_is_told() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let set =
        |user, hour, over| set_scene_time_of_day_impl(&state, user, false, scene_id, hour, over);
    assert!(
        set(player_id, Some(22.0), None).await.is_err(),
        "a Player may not set the time"
    );
    assert!(
        set(owner_id, Some(25.0), None).await.is_err(),
        "an hour is 0 to 24"
    );
    assert!(
        set(owner_id, Some(22.0), Some(-1.0)).await.is_err(),
        "time passes forwards"
    );

    let scene = set(owner_id, Some(22.0), Some(2.0))
        .await
        .expect("the DM may set the time");
    assert_eq!(scene.time_of_day, Some(22.0));
    let midnight = set(owner_id, Some(24.0), None)
        .await
        .expect("24 is midnight");
    assert_eq!(midnight.time_of_day, Some(0.0));
    let stopped = set(owner_id, None, None).await.expect("the clock stops");
    assert_eq!(stopped.time_of_day, None);

    let mut conn = state.db_pool.get().unwrap();
    let payloads: Vec<Option<serde_json::Value>> = world_events::table
        .filter(world_events::world_id.eq(world_id))
        .filter(world_events::event_code.eq(crate::world_events::EVENT_CODE_SCENE_TIME_CHANGED))
        .order(world_events::id.asc())
        .select(world_events::token_event)
        .load(&mut conn)
        .expect("world events should load");
    assert_eq!(
        payloads.len(),
        3,
        "one announcement each, none for the refusals"
    );
    let first = payloads[0]
        .clone()
        .expect("the announcement carries the hour");
    assert_eq!(first["sceneId"], scene_id.to_string());
    assert_eq!(first["hour"], 22.0);
    assert_eq!(first["overSecs"], 2.0);
    assert!(payloads[2].clone().unwrap()["hour"].is_null());
}
