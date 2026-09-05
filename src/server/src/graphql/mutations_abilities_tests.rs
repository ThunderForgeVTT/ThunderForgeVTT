use super::*;
use crate::test_support::*;

fn create_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
    CreateAbilityInput {
        world_id,
        name: name.to_string(),
        description: None,
        classification: "spell".to_string(),
        grade: None,
        gm_only: None,
    }
}

/// FR-002: only the DM may create.
#[tokio::test]
async fn only_dm_can_create_ability() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let player_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let err = create_ability_impl(&state, player_id, false, create_input(world_id, "Nope"))
        .await
        .expect_err("a Player must not create abilities");
    assert!(err.message.contains("Only the DM"));

    let created = create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
        .await
        .expect("the world owner may create");
    assert_eq!(created.name, "Fireball");
    assert!(!created.gm_only, "abilities default to visible (FR-024a)");
}

/// FR-006: duplicate names are permitted within a world.
#[tokio::test]
async fn ability_names_may_collide() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let a = create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
        .await
        .expect("first insert");
    let b = create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
        .await
        .expect("a duplicate name must be permitted (FR-006)");
    assert_ne!(a.id, b.id);
    assert_eq!(a.name, b.name);
}

/// research.md §3 defect 1: `updateItem` cannot clear a description because
/// `description.or(existing)` treats null as "unchanged". The ability
/// version must not inherit that.
#[tokio::test]
async fn update_ability_can_clear_description() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let mut input = create_input(world_id, "Fireball");
    input.description = Some("A ball of fire.".to_string());
    let created = create_ability_impl(&state, owner_id, false, input)
        .await
        .unwrap();
    assert!(created.description.is_some());

    // Omitting the field leaves it untouched...
    let untouched = update_ability_impl(
        &state,
        owner_id,
        false,
        UpdateAbilityInput {
            ability_id: created.id,
            name: Some("Fireball II".to_string()),
            description: None,
            classification: None,
            grade: None,
            clear_description: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        untouched.description.as_deref(),
        Some("A ball of fire."),
        "an omitted description must not be silently cleared"
    );

    // ...and the explicit flag actually clears it.
    let cleared = update_ability_impl(
        &state,
        owner_id,
        false,
        UpdateAbilityInput {
            ability_id: created.id,
            name: None,
            description: None,
            classification: None,
            grade: None,
            clear_description: Some(true),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        cleared.description, None,
        "clear_description must actually clear it — the item version cannot"
    );
}

/// FR-024c: visibility is DM-only. An ability-level Owner is not enough,
/// which is the whole reason this is a separate mutation.
#[tokio::test]
async fn only_dm_can_set_gm_only() {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let member_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_world_member(&mut conn, world_id, member_id, "Player");
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, create_input(world_id, "Secret"))
        .await
        .unwrap();

    // Grant the member Owner-level permission on the ability itself.
    let mut conn = state.db_pool.get().unwrap();
    diesel::insert_into(crate::schema::world_ability_permissions::table)
        .values((
            crate::schema::world_ability_permissions::id.eq(Uuid::now_v7()),
            crate::schema::world_ability_permissions::ability_id.eq(ability.id),
            crate::schema::world_ability_permissions::user_id.eq(member_id),
            crate::schema::world_ability_permissions::level.eq("Owner"),
        ))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let err = set_ability_gm_only_impl(&state, member_id, false, ability.id, true)
        .await
        .expect_err("ability-level Owner must NOT be able to change visibility");
    assert!(err.message.contains("Only the DM"));

    let hidden = set_ability_gm_only_impl(&state, owner_id, false, ability.id, true)
        .await
        .expect("the DM may hide it");
    assert!(hidden.gm_only);

    let shown = set_ability_gm_only_impl(&state, owner_id, false, ability.id, false)
        .await
        .expect("the DM may reveal it again");
    assert!(!shown.gm_only, "unhiding must be possible (US5 scenario 3)");
}
