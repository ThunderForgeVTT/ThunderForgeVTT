//! The settings row against a real Postgres.
//!
//! The pure rules — the defaults, mode parsing, the three validations — are
//! tested beside them in `mod.rs` with no database in sight. What needs a real
//! database is the pair of claims those tests cannot make: that a world with no
//! row reads as the defaults rather than failing, and that the upsert both
//! creates and replaces. Both are properties of the SQL, not of the Rust.

use thunderforge_server::test_support::{insert_test_user, insert_test_world, test_app_state};

use super::*;

/// The invariant the whole feature rests on: no row is not an error.
///
/// Every world that existed before this table, and every world whose Game
/// Master has never opened the settings panel, takes this path. If it ever
/// returns an error instead, Roll for Shoes stops working for every world that
/// plays the core game — which is all of them by default.
#[test]
fn a_world_with_no_row_reads_every_default() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    let settings =
        read_or_default(&mut conn, world_id).expect("a missing row must not be an error");

    assert_eq!(settings, WorldSettings::default());
    assert_eq!(settings.difficulty_mode, DifficultyMode::Free);
    assert!(!settings.tie_succeeds);
    assert!(!settings.statuses_enabled);
    assert!(!settings.skill_slots_enabled);
    assert!(settings.starting_skills.is_empty());
}

/// The upsert has to do both halves of its name, and the second half is the
/// one a plain `INSERT` would get wrong: a Game Master changing a setting
/// twice must not hit a primary-key violation on the second change.
#[test]
fn an_upsert_creates_a_row_and_then_replaces_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    let first = upsert(
        &mut conn,
        UpsertSettings {
            world_id,
            difficulty_mode: DifficultyMode::Target.as_str().to_string(),
            tie_succeeds: true,
            statuses_enabled: true,
            skill_slots_enabled: false,
            starting_skills: serde_json::to_value(vec![StartingSkill {
                name: "Scavenge".into(),
                level: 2,
            }])
            .expect("serialisable"),
            updated_by: Some(owner_id),
        },
    )
    .expect("the first write creates the row");

    assert_eq!(first.difficulty_mode, DifficultyMode::Target);
    assert!(first.tie_succeeds);
    assert!(first.statuses_enabled);
    assert!(!first.skill_slots_enabled);
    assert_eq!(first.starting_skills.len(), 1);
    assert_eq!(first.starting_skills[0].name, "Scavenge");
    assert_eq!(first.starting_skills[0].level, 2);

    // Read it back through the ordinary path: what the panel will see.
    let read_back = read_or_default(&mut conn, world_id).expect("a stored row reads back");
    assert_eq!(read_back, first);

    // The second write is the one that would fail on a bare INSERT.
    let second = upsert(
        &mut conn,
        UpsertSettings {
            world_id,
            difficulty_mode: DifficultyMode::Free.as_str().to_string(),
            tie_succeeds: false,
            statuses_enabled: false,
            skill_slots_enabled: true,
            starting_skills: serde_json::to_value(Vec::<StartingSkill>::new())
                .expect("serialisable"),
            updated_by: Some(owner_id),
        },
    )
    .expect("the second write replaces the row");

    // A whole-row upsert: every field the second call stated now holds, and
    // none of the first call's values survive underneath.
    assert_eq!(second.difficulty_mode, DifficultyMode::Free);
    assert!(!second.tie_succeeds);
    assert!(!second.statuses_enabled);
    assert!(second.skill_slots_enabled);
    assert!(second.starting_skills.is_empty());
    assert_eq!(
        read_or_default(&mut conn, world_id).expect("reads back"),
        second
    );
}

/// Clearing the starting skills must mean "this world uses the core default",
/// not "this world's characters start with nothing". The two are the same
/// stored value — an empty list — so the distinction lives in how it reads,
/// and this pins the storage half of it.
#[test]
fn an_empty_starting_skill_list_round_trips_as_empty() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    upsert(
        &mut conn,
        UpsertSettings {
            world_id,
            difficulty_mode: DifficultyMode::Rolled.as_str().to_string(),
            tie_succeeds: false,
            statuses_enabled: false,
            skill_slots_enabled: false,
            starting_skills: serde_json::to_value(Vec::<StartingSkill>::new())
                .expect("serialisable"),
            updated_by: Some(owner_id),
        },
    )
    .expect("stores");

    let settings = read_or_default(&mut conn, world_id).expect("reads");
    assert!(settings.starting_skills.is_empty());
    // Not the same as a world with no row at all: that one is Free.
    assert_eq!(settings.difficulty_mode, DifficultyMode::Rolled);
}

/// Deleting a world takes its settings with it. The row is keyed on the world
/// and means nothing without it, so the `ON DELETE CASCADE` is part of the
/// design rather than an incidental constraint — and account deletion removes
/// worlds, so this path runs for real.
#[test]
fn deleting_a_world_deletes_its_settings() {
    use diesel::sql_query;

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a test database connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    upsert(
        &mut conn,
        UpsertSettings {
            world_id,
            difficulty_mode: DifficultyMode::Target.as_str().to_string(),
            tie_succeeds: true,
            statuses_enabled: false,
            skill_slots_enabled: false,
            starting_skills: serde_json::to_value(Vec::<StartingSkill>::new())
                .expect("serialisable"),
            updated_by: Some(owner_id),
        },
    )
    .expect("stores");

    sql_query("DELETE FROM worlds WHERE id = $1")
        .bind::<diesel::sql_types::Uuid, _>(world_id)
        .execute(&mut conn)
        .expect("the world deletes");

    let remaining: i64 = world_roll_for_shoes_settings::table
        .filter(world_roll_for_shoes_settings::world_id.eq(world_id))
        .count()
        .get_result(&mut conn)
        .expect("counts");
    assert_eq!(remaining, 0, "the settings row must not outlive its world");
}
