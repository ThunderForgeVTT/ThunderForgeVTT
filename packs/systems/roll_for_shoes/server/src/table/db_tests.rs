//! The standing difficulty against a real Postgres.
//!
//! The choice between a named number, a fixed band and a rolled one is pure
//! and is tested beside `plan` in `mod.rs`. What needs a database is who may
//! write, what a player reads back, that the Game Master's dice are rolled
//! and kept rather than rolled again on every read, and that the row goes
//! when its world does.

use rand::SeedableRng;
use thunderforge_server::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};
use uuid::Uuid;

use super::graphql::{
    clear_difficulty_impl, read_difficulty_impl, set_difficulty_impl,
    SetRollForShoesTableDifficultyInput,
};
use super::*;
use crate::settings::{upsert, UpsertSettings};

fn seeded() -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(62)
}

/// A world, its Game Master, and one player who has joined it.
fn a_table() -> (thunderforge_server::state::AppState, Uuid, Uuid, Uuid) {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");
    let gm_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);
    (state, world_id, gm_id, player_id)
}

fn use_mode(state: &thunderforge_server::state::AppState, world_id: Uuid, mode: DifficultyMode) {
    let mut conn = state.db_pool.get().expect("a test database connection");
    upsert(
        &mut conn,
        UpsertSettings {
            world_id,
            difficulty_mode: mode.as_str().to_string(),
            tie_succeeds: false,
            statuses_enabled: false,
            skill_slots_enabled: false,
            starting_skills: serde_json::json!([]),
            updated_by: None,
        },
    )
    .expect("the mode stores");
}

fn named(world_id: Uuid, target: i32) -> SetRollForShoesTableDifficultyInput {
    SetRollForShoesTableDifficultyInput {
        world_id,
        target: Some(target),
        band: None,
    }
}

fn banded(world_id: Uuid, band: &str) -> SetRollForShoesTableDifficultyInput {
    SetRollForShoesTableDifficultyInput {
        world_id,
        target: None,
        band: Some(band.to_string()),
    }
}

/// The invariant that keeps every existing table playing as it did: a world
/// nobody has set a difficulty for reads as "none", not as an error and not
/// as zero.
#[tokio::test]
async fn a_world_with_no_difficulty_reads_as_none_set() {
    let (state, world_id, gm_id, player_id) = a_table();

    let for_the_player = read_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect("a member may read");
    assert_eq!(for_the_player.target, None);
    assert_eq!(for_the_player.band, None);
    assert_eq!(for_the_player.gm_dice, None);
    assert!(!for_the_player.can_set, "a player may not set it");

    let for_the_gm = read_difficulty_impl(&state, gm_id, false, world_id)
        .await
        .expect("the Game Master may read");
    assert!(for_the_gm.can_set, "the Game Master may");
}

#[tokio::test]
async fn the_game_master_names_a_number_and_the_player_reads_it() {
    let (state, world_id, gm_id, player_id) = a_table();

    let set = set_difficulty_impl(&state, gm_id, false, named(world_id, 7), &mut seeded())
        .await
        .expect("the Game Master sets it");
    assert_eq!(set.target, Some(7));
    assert_eq!(set.band, None);
    assert_eq!(set.gm_dice, None);

    let read = read_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect("the player reads it");
    assert_eq!(read.target, Some(7));
    assert!(!read.can_set);

    // A second set replaces the first rather than failing on the key.
    set_difficulty_impl(&state, gm_id, false, named(world_id, 4), &mut seeded())
        .await
        .expect("the second set replaces");
    let read = read_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect("reads");
    assert_eq!(read.target, Some(4));
}

/// The point of the feature. A player's sheet must not be able to move the
/// number, by this route or by clearing it and typing their own.
#[tokio::test]
async fn a_player_can_neither_set_nor_clear_it() {
    let (state, world_id, gm_id, player_id) = a_table();
    set_difficulty_impl(&state, gm_id, false, named(world_id, 9), &mut seeded())
        .await
        .expect("the Game Master sets it");

    let refused = set_difficulty_impl(&state, player_id, false, named(world_id, 1), &mut seeded())
        .await
        .expect_err("a player's set must be refused");
    assert!(
        refused.message.contains("Game Master"),
        "{}",
        refused.message
    );

    let refused = clear_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect_err("a player's clear must be refused");
    assert!(
        refused.message.contains("Game Master"),
        "{}",
        refused.message
    );

    let read = read_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect("reads");
    assert_eq!(read.target, Some(9), "the number did not move");
}

#[tokio::test]
async fn somebody_outside_the_world_cannot_read_it() {
    let (state, world_id, gm_id, _) = a_table();
    set_difficulty_impl(&state, gm_id, false, named(world_id, 5), &mut seeded())
        .await
        .expect("sets");

    let outsider_id = {
        let mut conn = state.db_pool.get().expect("a connection");
        insert_test_user(&mut conn)
    };
    read_difficulty_impl(&state, outsider_id, false, world_id)
        .await
        .expect_err("a non-member must not learn the number");
}

#[tokio::test]
async fn a_band_in_a_target_world_stores_its_fixed_number() {
    let (state, world_id, gm_id, _) = a_table();
    use_mode(&state, world_id, DifficultyMode::Target);

    let set = set_difficulty_impl(
        &state,
        gm_id,
        false,
        banded(world_id, "hard"),
        &mut seeded(),
    )
    .await
    .expect("sets");
    assert_eq!(set.target, Some(9));
    assert_eq!(set.band.as_deref(), Some("hard"));
    assert_eq!(set.gm_dice, None, "nothing is rolled for a fixed target");
}

/// In a rolled world the Game Master's dice are rolled by the server, once.
/// Reading twice must show the same faces: a difficulty that changed each
/// time somebody looked would be the re-roll this feature exists to remove.
#[tokio::test]
async fn a_band_in_a_rolled_world_rolls_once_and_keeps_the_dice() {
    let (state, world_id, gm_id, player_id) = a_table();
    use_mode(&state, world_id, DifficultyMode::Rolled);

    let set = set_difficulty_impl(
        &state,
        gm_id,
        false,
        banded(world_id, "veryHard"),
        &mut seeded(),
    )
    .await
    .expect("sets");

    let faces = set.gm_dice.clone().expect("the faces are stored");
    assert_eq!(faces.len(), 4, "very hard is four dice");
    assert!(faces.iter().all(|face| (1..=6).contains(face)), "{faces:?}");
    assert_eq!(set.target, Some(faces.iter().sum::<i32>()));
    assert_eq!(set.band.as_deref(), Some("veryHard"));

    for _ in 0..2 {
        let read = read_difficulty_impl(&state, player_id, false, world_id)
            .await
            .expect("reads");
        assert_eq!(read.gm_dice.as_ref(), Some(&faces));
        assert_eq!(read.target, set.target);
    }
}

#[tokio::test]
async fn a_band_in_a_free_world_is_refused_and_nothing_is_stored() {
    let (state, world_id, gm_id, _) = a_table();

    set_difficulty_impl(
        &state,
        gm_id,
        false,
        banded(world_id, "easy"),
        &mut seeded(),
    )
    .await
    .expect_err("a free world has no bands");

    let read = read_difficulty_impl(&state, gm_id, false, world_id)
        .await
        .expect("reads");
    assert_eq!(read.target, None);
}

#[tokio::test]
async fn clearing_hands_the_sheets_back_their_own_entry() {
    let (state, world_id, gm_id, player_id) = a_table();
    set_difficulty_impl(&state, gm_id, false, named(world_id, 6), &mut seeded())
        .await
        .expect("sets");

    let cleared = clear_difficulty_impl(&state, gm_id, false, world_id)
        .await
        .expect("the Game Master clears it");
    assert_eq!(cleared.target, None);

    let read = read_difficulty_impl(&state, player_id, false, world_id)
        .await
        .expect("reads");
    assert_eq!(read.target, None);

    // Clearing what is already clear is not an error.
    clear_difficulty_impl(&state, gm_id, false, world_id)
        .await
        .expect("clearing nothing is fine");
}

/// Each write tells the table, under the pack's own code, and says only that
/// something changed — never the number.
#[tokio::test]
async fn setting_and_clearing_are_announced_without_the_number() {
    use diesel::sql_query;
    use diesel::sql_types::{Integer, Nullable, Text, Uuid as SqlUuid};

    #[derive(diesel::QueryableByName)]
    struct Announced {
        #[diesel(sql_type = Nullable<Text>)]
        payload: Option<String>,
    }

    let (state, world_id, gm_id, _) = a_table();
    set_difficulty_impl(&state, gm_id, false, named(world_id, 11), &mut seeded())
        .await
        .expect("sets");
    clear_difficulty_impl(&state, gm_id, false, world_id)
        .await
        .expect("clears");

    let mut conn = state.db_pool.get().expect("a connection");
    let announced: Vec<Announced> = sql_query(
        "SELECT token_event::text AS payload FROM world_events \
         WHERE world_id = $1 AND event_code = $2 ORDER BY id",
    )
    .bind::<SqlUuid, _>(world_id)
    .bind::<Integer, _>(EVENT_CODE_TABLE_DIFFICULTY)
    .load(&mut conn)
    .expect("the events read");

    let payloads: Vec<String> = announced.into_iter().filter_map(|a| a.payload).collect();
    assert_eq!(payloads.len(), 2, "{payloads:?}");
    assert!(payloads[0].contains("set"), "{payloads:?}");
    assert!(payloads[1].contains("cleared"), "{payloads:?}");
    assert!(
        payloads.iter().all(|p| !p.contains("11")),
        "the number is read, never broadcast: {payloads:?}"
    );
}

/// The row is keyed on the world and means nothing without it. Account
/// deletion removes worlds, so this path runs for real.
#[test]
fn deleting_a_world_deletes_its_difficulty() {
    use diesel::sql_query;

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    set(
        &mut conn,
        SetDifficulty {
            world_id,
            target: 6,
            band: Some("moderate".into()),
            gm_dice: Some(serde_json::json!([2, 4])),
            set_by: Some(owner_id),
        },
    )
    .expect("stores");
    let stored = read(&mut conn, world_id).expect("reads").expect("is set");
    assert_eq!(stored.band, Some(Band::Moderate));
    assert_eq!(stored.gm_dice, Some(vec![2, 4]));

    sql_query("DELETE FROM worlds WHERE id = $1")
        .bind::<diesel::sql_types::Uuid, _>(world_id)
        .execute(&mut conn)
        .expect("the world deletes");

    assert_eq!(read(&mut conn, world_id).expect("reads"), None);
}
