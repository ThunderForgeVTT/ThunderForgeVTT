use super::*;
use crate::test_support::*;

/// A scene, its Game Master, and two players at the table.
struct Table {
    state: crate::AppState,
    world_id: Uuid,
    scene_id: Uuid,
    gm: Uuid,
    aria: Uuid,
    brom: Uuid,
}

fn table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    let scene_id = insert_test_scene(&mut conn, world_id, gm);
    let aria = insert_test_user(&mut conn);
    let brom = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, aria, "Player");
    insert_test_world_member(&mut conn, world_id, brom, "Player");
    Table {
        state,
        world_id,
        scene_id,
        gm,
        aria,
        brom,
    }
}

fn state_of(table: &Table, user: Uuid) -> ExplorationState {
    let mut conn = table.state.db_pool.get().unwrap();
    state_for(&mut conn, table.scene_id, user).expect("readable")
}

#[tokio::test]
async fn a_scene_remembers_nothing_until_a_game_master_says_so() {
    // FR-070. A scene that has never heard of exploration must not start
    // hiding its map from the table on the strength of a migration.
    let table = table();
    let before = state_of(&table, table.aria);
    assert!(!before.enabled);
    assert_eq!(before.epoch, 0);

    set_enabled(&table.state, table.gm, false, table.scene_id, true)
        .await
        .expect("the Game Master may turn it on");
    assert!(state_of(&table, table.aria).enabled);
}

#[tokio::test]
async fn a_player_may_not_turn_exploration_on_or_reset_it() {
    let table = table();
    let refused = set_enabled(&table.state, table.aria, false, table.scene_id, true).await;
    assert!(refused.is_err(), "a player must not turn it on");

    let refused = reset(&table.state, table.aria, false, table.scene_id, None).await;
    assert!(refused.is_err(), "nor reset it");
}

#[tokio::test]
async fn a_reset_for_everyone_moves_the_scenes_own_epoch() {
    let table = table();
    assert_eq!(state_of(&table, table.aria).mine, 0);

    let epoch = reset(&table.state, table.gm, false, table.scene_id, None)
        .await
        .expect("reset");
    assert_eq!(epoch, 1);
    // Both players' stored maps are now stale, and neither needed a row.
    assert_eq!(state_of(&table, table.aria).mine, 1);
    assert_eq!(state_of(&table, table.brom).mine, 1);
}

#[tokio::test]
async fn a_reset_for_one_player_leaves_the_others_map_alone() {
    // The reason there are two epochs rather than one. Resetting a player who
    // wandered somewhere they should not have must not wipe the table.
    let table = table();
    reset(
        &table.state,
        table.gm,
        false,
        table.scene_id,
        Some(table.aria),
    )
    .await
    .expect("reset");

    assert_eq!(state_of(&table, table.aria).mine, 1, "Aria's map is stale");
    assert_eq!(state_of(&table, table.brom).mine, 0, "Brom's is not");
}

#[tokio::test]
async fn a_later_reset_for_everyone_still_reaches_a_player_reset_alone() {
    // The case taking the greater of the two is for. Aria's own row is
    // already 1; a reset for everyone takes the scene to 1 as well, and it
    // must still invalidate what she has stored since.
    let table = table();
    reset(
        &table.state,
        table.gm,
        false,
        table.scene_id,
        Some(table.aria),
    )
    .await
    .expect("reset");
    let after_hers = state_of(&table, table.aria).mine;

    reset(&table.state, table.gm, false, table.scene_id, None)
        .await
        .expect("reset");
    let after_everyone = state_of(&table, table.aria).mine;

    assert!(
        after_everyone > after_hers,
        "a reset for everyone must move Aria's number too: {after_hers} -> {after_everyone}"
    );
    // Brom is stale as well, and lands on the same number as Aria — a reset
    // for everyone leaves one epoch, not a scene epoch and a residue of
    // per-player rows. The exact value is not pinned: it has to clear the
    // highest row rather than simply increment, so it is 2 here and would be
    // higher on a scene that had seen more individual resets.
    let brom = state_of(&table, table.brom).mine;
    assert_eq!(brom, after_everyone);
    assert!(brom > 0);
}

#[tokio::test]
async fn resetting_one_player_twice_keeps_moving_their_number() {
    // A Game Master who resets the same player again must not find the second
    // reset silently does nothing — the row is updated, not inserted once.
    let table = table();
    let first = reset(
        &table.state,
        table.gm,
        false,
        table.scene_id,
        Some(table.aria),
    )
    .await
    .expect("reset");
    let second = reset(
        &table.state,
        table.gm,
        false,
        table.scene_id,
        Some(table.aria),
    )
    .await
    .expect("reset again");
    assert!(second > first, "{first} -> {second}");
    assert_eq!(state_of(&table, table.aria).mine, second);
}

#[tokio::test]
async fn a_reset_is_announced_so_a_player_watching_hears_at_once() {
    // The epoch is what makes a reset stick for a player who was away. This
    // is the fast path for one who is looking at the board.
    use crate::schema::world_events;
    use diesel::prelude::*;

    let table = table();
    let mut conn = table.state.db_pool.get().unwrap();
    // Scoped to this world. Counting every such event in the database counts
    // the ones other tests recorded, which are running at the same time.
    let before: i64 = world_events::table
        .filter(world_events::world_id.eq(table.world_id))
        .filter(
            world_events::event_code.eq(crate::world_events::EVENT_CODE_SCENE_EXPLORATION_RESET),
        )
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);
    drop(conn);

    reset(&table.state, table.gm, false, table.scene_id, None)
        .await
        .expect("reset");

    let mut conn = table.state.db_pool.get().unwrap();
    let after: i64 = world_events::table
        .filter(world_events::world_id.eq(table.world_id))
        .filter(
            world_events::event_code.eq(crate::world_events::EVENT_CODE_SCENE_EXPLORATION_RESET),
        )
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);
    assert_eq!(after, before + 1, "the reset was announced");
}
