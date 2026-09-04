//! Spec 033 User Story 4 — binding and grading, refused at the data boundary.
//!
//! In a sibling file so `check-system-registry.mjs` and
//! `check-ability-vocabulary.mjs` treat it as a test: asserting "5e's
//! Enchantment binds to items" requires naming both.

use super::*;
use crate::test_support::*;

/// `test_app_state` points its data path at the system temp directory, so its
/// `systems_dir` holds no packs and every vocabulary falls back to the
/// built-ins. These tests are about a *shipped* pack's declarations, so they
/// point at the real ones — the property under test is that 5e's own
/// Enchantment binds to items, and a fixture would prove only that the parser
/// works.
fn state_with_real_packs() -> AppState {
    let mut state = test_app_state();
    state.directories.systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist")
        .to_string_lossy()
        .into_owned();
    state
}

/// SC-011: a type that binds to characters is refused on an item, through the
/// API rather than through a disabled button.
#[tokio::test]
async fn a_character_bound_type_is_refused_on_an_item() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    // 5e, whose Spells bind to characters and whose Enchantments bind to items.
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();

    let ability_id = insert_test_ability(&mut conn, world_id, owner_id); // spell
    let item_id = insert_test_item(&mut conn, world_id, owner_id);
    drop(conn);

    let refused = attach_ability_to_item_impl(
        &state,
        owner_id,
        false,
        AttachAbilityToItemInput {
            item_id,
            ability_id,
        },
    )
    .await;

    assert!(
        refused.is_err(),
        "a Spell binds to a character; attaching it to an item must be refused"
    );
    assert!(
        refused
            .unwrap_err()
            .message
            .contains("do not attach to items"),
        "the refusal should say what the type does, not just 'no'"
    );
}

/// The other direction: a type that declares `binds: item` is accepted, and
/// appears on the item.
#[tokio::test]
async fn an_item_bound_type_attaches_and_is_listed() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();

    let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
    // Re-typed to 5e's own item-bound type — the one no shared file names.
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
        .set(world_abilities::classification.eq("enchantment"))
        .execute(&mut conn)
        .unwrap();

    let item_id = insert_test_item(&mut conn, world_id, owner_id);
    drop(conn);

    let attached = attach_ability_to_item_impl(
        &state,
        owner_id,
        false,
        AttachAbilityToItemInput {
            item_id,
            ability_id,
        },
    )
    .await
    .expect("an item-bound type should attach to an item");

    assert_eq!(attached.ability_id, Some(ability_id));
    assert_eq!(attached.classification.as_deref(), Some("enchantment"));

    let listed = item_abilities_impl(&state, owner_id, false, item_id)
        .await
        .expect("the item's abilities should list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].ability_name, attached.ability_name);
}

/// FR-039: an ability cannot be attached to an item in another world.
#[tokio::test]
async fn an_ability_cannot_reach_an_item_in_another_world() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_a = insert_test_world(&mut conn, owner_id);
    let world_b = insert_test_world(&mut conn, owner_id);

    let ability_id = insert_test_ability(&mut conn, world_a, owner_id);
    let item_id = insert_test_item(&mut conn, world_b, owner_id);
    drop(conn);

    let refused = attach_ability_to_item_impl(
        &state,
        owner_id,
        false,
        AttachAbilityToItemInput {
            item_id,
            ability_id,
        },
    )
    .await;

    assert!(
        refused.is_err(),
        "one world's ability must not reach another world's item"
    );
}

// ---------------------------------------------------------------------------
// Grades (FR-021 to FR-023)
// ---------------------------------------------------------------------------

use crate::graphql::mutations_abilities::{CreateAbilityInput, create_ability_impl};

/// FR-023: a value outside the type's declared range is refused at authoring.
#[tokio::test]
async fn a_grade_outside_the_declared_range_is_refused() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    // 5e declares Spells graded by Level, 0 to 9.
    let refused = create_ability_impl(
        &state,
        owner_id,
        false,
        CreateAbilityInput {
            world_id,
            name: "Wish Plus".to_string(),
            description: None,
            classification: "spell".to_string(),
            grade: Some(12),
            gm_only: None,
        },
    )
    .await;

    assert!(refused.is_err(), "a Level 12 spell is outside 5e's range");
    let message = refused.unwrap_err().message;
    assert!(
        message.contains("Level"),
        "the refusal should use the system's word for the grade, not ours: {message}"
    );
}

/// FR-022: a value on a type that declares no grade is refused rather than
/// stored — a number on a sheet that nothing explains is worse than none.
#[tokio::test]
async fn a_grade_on_an_ungraded_type_is_refused() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    // 5e's Feats declare no grade.
    let refused = create_ability_impl(
        &state,
        owner_id,
        false,
        CreateAbilityInput {
            world_id,
            name: "Alert".to_string(),
            description: None,
            classification: "feat".to_string(),
            grade: Some(3),
            gm_only: None,
        },
    )
    .await;

    assert!(refused.is_err(), "an ungraded type takes no grade");
}

/// A value inside the range is accepted and stored.
#[tokio::test]
async fn a_grade_inside_the_declared_range_is_kept() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some("dnd5e".to_string())))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let created = create_ability_impl(
        &state,
        owner_id,
        false,
        CreateAbilityInput {
            world_id,
            name: "Fireball".to_string(),
            description: None,
            classification: "spell".to_string(),
            grade: Some(3),
            gm_only: None,
        },
    )
    .await
    .expect("a Level 3 spell is inside 5e's range");

    assert_eq!(created.grade, Some(3));
}

/// FR-023's other half: a stored value outside a *newly narrowed* range is
/// retained, never clamped or discarded.
///
/// Written against the database rather than a manifest edit, because the
/// requirement is about what happens to content already stored — a system
/// narrowing its range does not get to edit a Game Master's abilities, and the
/// column carries no constraint precisely so that it cannot.
#[tokio::test]
async fn a_stored_grade_outside_a_narrowed_range_is_retained() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

    // Authored under a wider range than the one in force now.
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
        .set(world_abilities::grade.eq(Some(12)))
        .execute(&mut conn)
        .unwrap();

    let stored: Option<i32> = world_abilities::table
        .filter(world_abilities::id.eq(ability_id))
        .select(world_abilities::grade)
        .first(&mut conn)
        .unwrap();

    assert_eq!(
        stored,
        Some(12),
        "a value outside the current range stays exactly as authored"
    );
}
