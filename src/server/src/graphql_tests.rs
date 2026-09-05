use super::{GraphQLCreateWorldInput, create_world_impl, prepare_world_input, validate_world_name};

/// Spec 008 (T022, FR-004/FR-006): `create_world` must always yield
/// exactly one scene — never zero — since the whole point of this
/// feature is that a freshly created world's canvas has content on it
/// immediately, with no separate "create a scene" step.
#[tokio::test]
async fn create_world_always_yields_exactly_one_scene() {
    use crate::schema::scenes;
    use crate::test_support::*;
    use diesel::prelude::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let world = create_world_impl(
        &state,
        user_id,
        GraphQLCreateWorldInput {
            name: "The Ember Crown".to_string(),
            description: None,
            game_system_id: None,
            interface_pack_id: None,
        },
    )
    .await
    .expect("world creation should succeed");

    let mut conn = state.db_pool.get().unwrap();
    let scene_count = scenes::table
        .filter(scenes::world_id.eq(world.id))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("scene count query should succeed");

    assert_eq!(
        scene_count, 1,
        "create_world must always produce exactly one default scene"
    );
}

/// Spec 008 (T022): an invalid world name must fail validation
/// *before* any DB write happens — confirming create_world_impl's
/// early-return on prepare_world_input's error leaves nothing
/// persisted (no orphaned world, no orphaned scene) for a rejected
/// input, the same "both succeed or both fail" guarantee research.md
/// §1 describes for the transaction itself.
#[tokio::test]
async fn create_world_rejects_invalid_name_before_any_write() {
    use crate::schema::worlds;
    use crate::test_support::*;
    use diesel::prelude::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let user_id = insert_test_user(&mut conn);
    // Scoped to this test's own throwaway user, NOT a global world count.
    // A global count is not isolation-safe: any concurrently-running test
    // that creates a world lands between the two reads and fails this
    // assertion spuriously. Scoping preserves the intent exactly — "this
    // rejected call wrote nothing" — while being immune to neighbours.
    let before_count = worlds::table
        .filter(worlds::created_by.eq(user_id))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("world count query should succeed");
    drop(conn);

    let result = create_world_impl(
        &state,
        user_id,
        GraphQLCreateWorldInput {
            name: "ab".to_string(), // below MIN_WORLD_NAME_LEN
            description: None,
            game_system_id: None,
            interface_pack_id: None,
        },
    )
    .await;

    assert!(result.is_err(), "a too-short name must be rejected");

    let mut conn = state.db_pool.get().unwrap();
    let after_count = worlds::table
        .filter(worlds::created_by.eq(user_id))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("world count query should succeed");
    assert_eq!(
        before_count, after_count,
        "a rejected create_world call must not write a world row"
    );
}

#[test]
fn world_name_validation_rejects_invalid_characters() {
    let result = validate_world_name("Bad@World");

    assert_eq!(
        result,
        Err(
            "World name may only contain letters, numbers, spaces, apostrophes, and - _ . , : ! ? ( )"
                .to_string(),
        )
    );
}

#[test]
fn prepare_world_input_trims_optional_fields() {
    let prepared = prepare_world_input(
        GraphQLCreateWorldInput {
            name: "  The   Ember   Crown  ".to_string(),
            description: Some("  A fallen kingdom  ".to_string()),
            game_system_id: Some("  systemless-sandbox ".to_string()),
            interface_pack_id: Some(" guild-hall-default ".to_string()),
        },
        None,
    )
    .expect("world input should be valid");

    assert_eq!(prepared.name, "The Ember Crown");
    assert_eq!(prepared.description.as_deref(), Some("A fallen kingdom"));
    assert_eq!(
        prepared.game_system_id.as_deref(),
        Some("systemless-sandbox")
    );
    assert_eq!(
        prepared.interface_pack_id.as_deref(),
        Some("guild-hall-default")
    );
}

// Phase 1.4: Security test - validate_world_name rejects XSS attempts
#[test]
fn world_name_validation_rejects_xss_attempts() {
    let xss_attempts = vec![
        "<script>alert('xss')</script>",
        "World<img src=x onerror=alert('xss')>",
        "'; DROP TABLE worlds; --",
        "World\x00Null",
    ];

    for xss in xss_attempts {
        let result = validate_world_name(xss);
        assert!(result.is_err(), "Should reject XSS attempt: {}", xss);
    }
}

// Phase 1.4: Security test - world name length limits
#[test]
fn world_name_validation_enforces_length_limits() {
    // Valid: 64 characters (MAX_WORLD_NAME_LEN)
    let valid = "A".repeat(64);
    assert!(
        validate_world_name(&valid).is_ok(),
        "64 chars should be valid"
    );

    // Invalid: 65+ characters
    let invalid = "A".repeat(65);
    assert!(
        validate_world_name(&invalid).is_err(),
        "65+ chars should be rejected"
    );

    // Invalid: 2 characters (MIN_WORLD_NAME_LEN is 3)
    let too_short = "AB";
    assert!(
        validate_world_name(too_short).is_err(),
        "2 chars should be rejected"
    );

    // Valid: 3 characters (MIN_WORLD_NAME_LEN)
    let min_valid = "ABC";
    assert!(
        validate_world_name(min_valid).is_ok(),
        "3 chars should be valid"
    );
}

// Phase 1.4: Security test - prepare_world_input rejects empty name
#[test]
fn prepare_world_input_rejects_empty_name() {
    let result = prepare_world_input(
        GraphQLCreateWorldInput {
            name: "  \t\n  ".to_string(), // Only whitespace
            description: None,
            game_system_id: None,
            interface_pack_id: None,
        },
        None,
    );

    assert!(result.is_err(), "Should reject empty/whitespace-only name");
}

// Phase 1.4: Security test - validate special characters are allowed (D&D names)
#[test]
fn world_name_validation_allows_dnd_style_names() {
    let valid_names = vec![
        "The Forgotten Realms",
        "Dragonlance: Time of Legend",
        "Spelljammer (Far Realm)",
        "Ravenloft's Dark Masters",
    ];

    for name in valid_names {
        assert!(
            validate_world_name(name).is_ok(),
            "Should allow D&D-style name: {}",
            name
        );
    }
}

// Spec 011: World Session Notes (contracts/session-notes.md)

#[tokio::test]
async fn dm_can_update_session_notes_and_read_it_back() {
    use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let updated = update_world_session_notes_impl(
        &state,
        owner_id,
        false,
        UpdateWorldSessionNotesInput {
            world_id,
            notes: "The party defeated the goblin ambush and pressed on.".to_string(),
        },
    )
    .await
    .expect("the DM should be able to update session notes");

    assert_eq!(
        updated.session_notes.as_deref(),
        Some("The party defeated the goblin ambush and pressed on.")
    );
}

#[tokio::test]
async fn saving_empty_session_notes_succeeds() {
    use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let updated = update_world_session_notes_impl(
        &state,
        owner_id,
        false,
        UpdateWorldSessionNotesInput {
            world_id,
            notes: "".to_string(),
        },
    )
    .await
    .expect("saving an explicit empty value must not error");

    assert_eq!(updated.session_notes.as_deref(), Some(""));
}

#[tokio::test]
async fn player_role_cannot_update_session_notes() {
    use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let result = update_world_session_notes_impl(
        &state,
        player_id,
        false,
        UpdateWorldSessionNotesInput {
            world_id,
            notes: "Should not be saved".to_string(),
        },
    )
    .await;

    assert!(
        result.is_err(),
        "a Player-role world member must not be able to update session notes"
    );
}

#[tokio::test]
async fn non_member_cannot_update_session_notes() {
    use super::{UpdateWorldSessionNotesInput, update_world_session_notes_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let result = update_world_session_notes_impl(
        &state,
        outsider_id,
        false,
        UpdateWorldSessionNotesInput {
            world_id,
            notes: "Should not be saved".to_string(),
        },
    )
    .await;

    assert!(
        result.is_err(),
        "a user with no relationship to the world must not be able to update session notes"
    );
}

// Spec 016: World System Assignment (T009)

// Spec 033 User Story 2: the guarded system switch (FR-024 to FR-033).
//
// SC-007 says the two-confirmation rule holds "including attempts that call
// the operation directly without the interface", which makes these the
// tests that matter most — a dialog anyone can skip is not a guard.

/// A world with content is refused when nothing is acknowledged (FR-028).
#[tokio::test]
async fn a_world_with_content_refuses_an_unacknowledged_system_change() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_ability(&mut conn, world_id, owner_id);
    drop(conn);

    let refused = update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            acknowledged_digest: None,
        },
    )
    .await;

    assert!(
        refused.is_err(),
        "a world holding authored content must not switch system unacknowledged"
    );
}

/// A digest taken before the world changed no longer acknowledges it.
///
/// The reason this is a digest and not a boolean: a Game Master who left
/// the dialog open while an actor was added is asked again, rather than
/// switching on numbers that were true a minute ago.
#[tokio::test]
async fn a_stale_acknowledgement_is_refused() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::graphql::queries::world_content::inventory_of;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_ability(&mut conn, world_id, owner_id);

    let taken = inventory_of(
        &mut conn,
        &state.directories.systems_dir,
        world_id,
        Some("dnd5e"),
    )
    .expect("counting a seeded world should work");

    // The world moves on while the dialog is open.
    insert_test_ability(&mut conn, world_id, owner_id);
    drop(conn);

    let refused = update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            acknowledged_digest: Some(taken.digest),
        },
    )
    .await;

    assert!(
        refused.is_err(),
        "a digest taken before the world changed must not still acknowledge it"
    );
}

/// The matching digest applies the change (FR-027's second confirmation
/// arriving as an acknowledgement the server can check).
#[tokio::test]
async fn a_matching_acknowledgement_applies_the_change() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::graphql::queries::world_content::inventory_of;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_ability(&mut conn, world_id, owner_id);

    let inventory = inventory_of(
        &mut conn,
        &state.directories.systems_dir,
        world_id,
        Some("dnd5e"),
    )
    .expect("counting a seeded world should work");
    assert!(!inventory.is_empty, "the world was seeded with an ability");
    drop(conn);

    let updated = update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            acknowledged_digest: Some(inventory.digest),
        },
    )
    .await
    .expect("acknowledging the counts should allow the change");

    assert_eq!(updated.game_system_id.as_deref(), Some("dnd5e"));
}

/// FR-005: a system change alters no authored content.
#[tokio::test]
async fn a_system_change_alters_no_authored_content() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::graphql::queries::world_content::inventory_of;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_ability(&mut conn, world_id, owner_id);
    insert_test_item(&mut conn, world_id, owner_id);

    let before = inventory_of(&mut conn, &state.directories.systems_dir, world_id, None)
        .expect("counting should work");
    let digest = inventory_of(
        &mut conn,
        &state.directories.systems_dir,
        world_id,
        Some("dnd5e"),
    )
    .expect("counting should work")
    .digest;
    drop(conn);

    update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            acknowledged_digest: Some(digest),
        },
    )
    .await
    .expect("the change should apply");

    let mut conn = state.db_pool.get().unwrap();
    let after = inventory_of(&mut conn, &state.directories.systems_dir, world_id, None)
        .expect("counting should work");

    assert_eq!(
        before.counts.len(),
        after.counts.len(),
        "a system change must not add or remove content"
    );
    for entry in &before.counts {
        let matched = after
            .counts
            .iter()
            .find(|other| other.kind == entry.kind && other.system_id == entry.system_id)
            .map(|other| other.count);
        assert_eq!(matched, Some(entry.count), "{} changed", entry.kind);
    }
}

/// FR-029: an empty world switches with no acknowledgement at all.
#[tokio::test]
async fn an_empty_world_needs_no_acknowledgement() {
    use crate::graphql::queries::world_content::inventory_of;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);

    let inventory = inventory_of(&mut conn, &state.directories.systems_dir, world_id, None)
        .expect("counting should work");

    // Its auto-created default scene does not make it non-empty: every
    // world has one (spec 010), so counting scenes would put the red
    // warning in front of a Game Master on a world a minute old.
    assert!(
        inventory.is_empty,
        "a world holding only its default scene is empty"
    );
}

#[tokio::test]
async fn dm_can_assign_a_world_game_system() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let updated = update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            // Empty world: FR-029's one-step path needs no acknowledgement.
            acknowledged_digest: None,
        },
    )
    .await
    .expect("the DM should be able to assign a game system");

    assert_eq!(updated.game_system_id.as_deref(), Some("dnd5e"));
}

#[tokio::test]
async fn assigning_an_empty_game_system_id_is_rejected() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let result = update_world_game_system_impl(
        &state,
        owner_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "  ".to_string(),
            acknowledged_digest: None,
        },
    )
    .await;

    assert!(result.is_err(), "an empty gameSystemId must be rejected");
}

#[tokio::test]
async fn player_role_cannot_assign_a_world_game_system() {
    use super::{UpdateWorldGameSystemInput, update_world_game_system_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let result = update_world_game_system_impl(
        &state,
        player_id,
        false,
        UpdateWorldGameSystemInput {
            world_id,
            game_system_id: "dnd5e".to_string(),
            // Empty world: FR-029's one-step path needs no acknowledgement.
            acknowledged_digest: None,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "a Player-role world member must not be able to change the world's game system"
    );
}

// ===== Spec 022: Scene Management Overhaul =====

#[tokio::test]
async fn update_scene_hidden_requires_dm_role() {
    use super::update_scene_hidden_impl;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let result = update_scene_hidden_impl(&state, player_id, false, scene_id, false).await;
    assert!(
        result.is_err(),
        "a Player-role world member must not be able to toggle a scene's hidden state"
    );

    let updated = update_scene_hidden_impl(&state, owner_id, false, scene_id, false)
        .await
        .expect("the DM (Owner) toggling hidden should succeed");
    assert!(!updated.hidden, "hidden should now be false");
}

#[tokio::test]
async fn launch_scene_requires_dm_role_and_rejects_cross_world_scene() {
    use super::launch_scene_impl;
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");

    // A second, unrelated world/scene pair — launching this scene into
    // the first world must be rejected (FR-002b implicitly assumes
    // same-world).
    let other_owner_id = insert_test_user(&mut conn);
    let other_world_id = insert_test_world(&mut conn, other_owner_id);
    let other_scene_id = insert_test_scene(&mut conn, other_world_id, other_owner_id);
    drop(conn);

    let player_result = launch_scene_impl(&state, player_id, false, world_id, scene_id).await;
    assert!(
        player_result.is_err(),
        "a Player-role world member must not be able to launch a scene"
    );

    let cross_world_result =
        launch_scene_impl(&state, owner_id, false, world_id, other_scene_id).await;
    assert!(
        cross_world_result.is_err(),
        "launching a scene that belongs to a different world must be rejected"
    );

    let updated_world = launch_scene_impl(&state, owner_id, false, world_id, scene_id)
        .await
        .expect("the DM launching an in-world scene should succeed");
    assert_eq!(
        updated_world.active_scene_id,
        Some(scene_id),
        "active_scene_id should now be the launched scene"
    );
}

#[tokio::test]
async fn update_world_default_scene_grid_type_requires_dm_role_and_valid_value() {
    use super::{UpdateWorldDefaultSceneGridTypeInput, update_world_default_scene_grid_type_impl};
    use crate::test_support::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let player_result = update_world_default_scene_grid_type_impl(
        &state,
        player_id,
        false,
        UpdateWorldDefaultSceneGridTypeInput {
            world_id,
            grid_type: "hex".to_string(),
        },
    )
    .await;
    assert!(
        player_result.is_err(),
        "a Player-role world member must not be able to change the default scene grid type"
    );

    let invalid_result = update_world_default_scene_grid_type_impl(
        &state,
        owner_id,
        false,
        UpdateWorldDefaultSceneGridTypeInput {
            world_id,
            grid_type: "triangles".to_string(),
        },
    )
    .await;
    assert!(
        invalid_result.is_err(),
        "an invalid gridType must be rejected"
    );

    let updated = update_world_default_scene_grid_type_impl(
        &state,
        owner_id,
        false,
        UpdateWorldDefaultSceneGridTypeInput {
            world_id,
            grid_type: "hex".to_string(),
        },
    )
    .await
    .expect("the DM setting a valid gridType should succeed");
    assert_eq!(updated.default_scene_grid_type, "hex");
}

// Note: `create_scene`'s "inherit world.default_scene_grid_type when
// gridType is omitted" behavior (FR-015) is exercised end-to-end by
// the Playwright e2e spec (apps/web/e2e/scene-default-grid-type.spec.ts)
// instead of a unit test here — `create_scene` is an inline
// `#[Object]` method (pre-existing, not extracted to a testable
// `_impl` by this feature) whose GraphQL context is impractical to
// construct in a focused unit test without also duplicating the
// full `MutationRoot`/`QueryRoot` wiring.
