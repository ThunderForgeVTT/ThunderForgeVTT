use super::*;
use crate::graphql::mutations_invites::*;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};
use diesel::PgConnection;

// ===== Spec 027 US2: member removal must clear grants on EVERY type =====
//
// `remove_member_impl` cleans up actor, item and lore grants in three
// hand-written blocks, each commented to explain there is no FK cascade
// from `world_members`. Spec 025 added `world_ability_permissions` and
// never added a fourth block, so a removed member kept their ability
// grants — and re-adding them silently restored Editor/Owner rights.
//
// These fail on the code as it stood before spec 027.

/// Sets up a member holding an explicit grant on one row of each of the
/// four permissioned content types.
/// Returns `(world_id, owner_id, member_id)`.
fn world_with_a_fully_granted_member(conn: &mut PgConnection, level: &str) -> (Uuid, Uuid, Uuid) {
    use crate::test_support::{
        grant_all_content_permissions, insert_test_ability, insert_test_actor, insert_test_item,
        insert_test_lore_entry, insert_test_scene,
    };

    let owner_id = insert_test_user(conn);
    let world_id = insert_test_world(conn, owner_id);
    let scene_id = insert_test_scene(conn, world_id, owner_id);

    let actor_id = insert_test_actor(conn, world_id, scene_id, owner_id);
    let item_id = insert_test_item(conn, world_id, owner_id);
    let lore_id = insert_test_lore_entry(conn, world_id, owner_id);
    let ability_id = insert_test_ability(conn, world_id, owner_id);

    let member_id = insert_test_user(conn);
    insert_test_world_member(conn, world_id, member_id, "Player");
    grant_all_content_permissions(
        conn, member_id, actor_id, item_id, lore_id, ability_id, level,
    );

    (world_id, owner_id, member_id)
}

/// FR-018 / US2-1: all four grant types are cleared on removal.
/// Before the fix this failed on the ability count alone.
#[tokio::test]
async fn removing_a_member_clears_grants_on_every_content_type() {
    use crate::test_support::count_content_permissions;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let (world_id, owner_id, member_id) = world_with_a_fully_granted_member(&mut conn, "Editor");

    // Precondition: the member really does hold all four.
    let before = count_content_permissions(&mut conn, world_id, member_id);
    assert_eq!(
        before,
        (1, 1, 1, 1),
        "setup failed — member should hold one grant of each type"
    );
    drop(conn);

    remove_member_impl(&state, owner_id, world_id, member_id)
        .await
        .expect("owner must be able to remove a player");

    let mut conn = state.db_pool.get().unwrap();
    let after = count_content_permissions(&mut conn, world_id, member_id);
    assert_eq!(
        after,
        (0, 0, 0, 0),
        "every grant must be cleared on removal; \
         a non-zero fourth element is the ability-cleanup gap (FR-018)"
    );
}

/// SC-008 / US2-2: readmission grants nothing back.
#[tokio::test]
async fn a_readmitted_member_holds_no_elevated_rights() {
    use crate::test_support::count_content_permissions;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let (world_id, owner_id, member_id) = world_with_a_fully_granted_member(&mut conn, "Owner");
    drop(conn);

    remove_member_impl(&state, owner_id, world_id, member_id)
        .await
        .expect("removal should succeed");

    // Re-invite: they come back as an ordinary Player.
    let mut conn = state.db_pool.get().unwrap();
    insert_test_world_member(&mut conn, world_id, member_id, "Player");

    let after = count_content_permissions(&mut conn, world_id, member_id);
    assert_eq!(
        after,
        (0, 0, 0, 0),
        "a readmitted member must not silently regain any prior grant"
    );
}

/// US2-3: removal is scoped to one world, and an empty grant set is fine.
#[tokio::test]
async fn removal_is_world_scoped_and_tolerates_no_grants() {
    use crate::test_support::count_content_permissions;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();

    let (world_a, owner_a, member_id) = world_with_a_fully_granted_member(&mut conn, "Editor");

    // The same user holds grants in an unrelated world.
    let world_b = {
        use crate::test_support::{
            grant_all_content_permissions, insert_test_ability, insert_test_actor,
            insert_test_item, insert_test_lore_entry, insert_test_scene,
        };
        let owner_b = insert_test_user(&mut conn);
        let world_b = insert_test_world(&mut conn, owner_b);
        let scene_b = insert_test_scene(&mut conn, world_b, owner_b);
        let actor_b = insert_test_actor(&mut conn, world_b, scene_b, owner_b);
        let item_b = insert_test_item(&mut conn, world_b, owner_b);
        let lore_b = insert_test_lore_entry(&mut conn, world_b, owner_b);
        let ability_b = insert_test_ability(&mut conn, world_b, owner_b);
        insert_test_world_member(&mut conn, world_b, member_id, "Player");
        grant_all_content_permissions(
            &mut conn, member_id, actor_b, item_b, lore_b, ability_b, "Editor",
        );
        world_b
    };
    drop(conn);

    remove_member_impl(&state, owner_a, world_a, member_id)
        .await
        .expect("removal from world A should succeed");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        count_content_permissions(&mut conn, world_b, member_id),
        (1, 1, 1, 1),
        "removal from one world must not touch grants in another"
    );

    // A member with no grants at all removes cleanly rather than erroring.
    let bare_member = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_a, bare_member, "Player");
    drop(conn);

    remove_member_impl(&state, owner_a, world_a, bare_member)
        .await
        .expect("removing a member holding zero grants must succeed quietly");
}

#[tokio::test]
async fn owner_with_no_membership_row_can_change_roles_and_remove_members() {
    // Spec 023 (research.md §3): identical bug class to
    // `owner_can_be_authorized_for_invites_immediately_after_world_creation`
    // above, now fixed in `update_member_role_impl`/`remove_member_impl`
    // via `require_world_member`'s Owner-fallback.
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    // No insert_test_world_member for the owner — deliberately, matching
    // what `create_world` actually leaves behind.
    let target_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, target_id, "Player");
    drop(conn);

    let payload = update_member_role_impl(
        &state,
        owner_id,
        UpdateMemberRoleInput {
            world_id,
            user_id: target_id,
            role: "GM".to_string(),
        },
    )
    .await
    .expect("the world's own owner, with no world_members row, must be able to change roles");
    assert_eq!(payload.role, "GM");

    let removed = remove_member_impl(&state, owner_id, world_id, target_id)
        .await
        .expect("the world's own owner, with no world_members row, must be able to remove members");
    assert!(removed);

    let mut conn = state.db_pool.get().unwrap();
    let remaining: Option<WorldMember> = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(target_id))
        .select(WorldMember::as_select())
        .first(&mut conn)
        .optional()
        .unwrap();
    assert!(remaining.is_none(), "removed member's row must be gone");
}
