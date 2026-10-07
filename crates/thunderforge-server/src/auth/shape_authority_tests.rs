use super::*;
use crate::models::NewWorldAuthoringToolRevocation;
use crate::schema::{world_authoring_tool_revocations, world_members};
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

struct Table {
    owner_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
}

fn table(conn: &mut PgConnection) -> Table {
    let owner_id = insert_test_user(conn);
    let world_id = insert_test_world(conn, owner_id);
    let scene_id = insert_test_scene(conn, world_id, owner_id);
    Table {
        owner_id,
        world_id,
        scene_id,
    }
}

fn player(conn: &mut PgConnection, world_id: Uuid) -> Uuid {
    let id = insert_test_user(conn);
    insert_test_world_member(conn, world_id, id, "Player");
    id
}

fn authority(
    conn: &mut PgConnection,
    user_id: Uuid,
    scene_id: Uuid,
    created_by: Option<Uuid>,
) -> ShapeAuthority {
    shape_authority(conn, user_id, false, scene_id, created_by).expect("authority")
}

#[test]
fn a_dm_may_write_any_shape() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let gm_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, t.world_id, gm_id, "GM");
    let player_id = player(&mut conn, t.world_id);

    for caller in [t.owner_id, gm_id] {
        assert_eq!(
            authority(&mut conn, caller, t.scene_id, None),
            ShapeAuthority::Dm
        );
        assert_eq!(
            authority(&mut conn, caller, t.scene_id, Some(player_id)),
            ShapeAuthority::Dm
        );
    }
}

#[test]
fn a_player_may_create_and_write_their_own() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let player_id = player(&mut conn, t.world_id);

    assert_eq!(
        authority(&mut conn, player_id, t.scene_id, None),
        ShapeAuthority::Creator
    );
    assert_eq!(
        authority(&mut conn, player_id, t.scene_id, Some(player_id)),
        ShapeAuthority::Creator
    );
}

#[test]
fn a_player_may_not_write_anothers_shape() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let a = player(&mut conn, t.world_id);
    let b = player(&mut conn, t.world_id);

    assert_eq!(
        authority(&mut conn, a, t.scene_id, Some(b)),
        ShapeAuthority::None
    );
    assert_eq!(
        authority(&mut conn, a, t.scene_id, Some(t.owner_id)),
        ShapeAuthority::None
    );
}

#[test]
fn a_revoked_creator_may_write_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let player_id = player(&mut conn, t.world_id);
    let member_id = world_members::table
        .filter(world_members::world_id.eq(t.world_id))
        .filter(world_members::user_id.eq(player_id))
        .select(world_members::id)
        .first::<Uuid>(&mut conn)
        .unwrap();
    diesel::insert_into(world_authoring_tool_revocations::table)
        .values(&NewWorldAuthoringToolRevocation {
            world_member_id: member_id,
            tool: "shapes".to_string(),
            revoked_by: Some(t.owner_id),
        })
        .execute(&mut conn)
        .unwrap();

    assert_eq!(
        authority(&mut conn, player_id, t.scene_id, None),
        ShapeAuthority::None
    );
    assert_eq!(
        authority(&mut conn, player_id, t.scene_id, Some(player_id)),
        ShapeAuthority::None
    );
}

#[test]
fn a_non_member_may_write_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let stranger = insert_test_user(&mut conn);

    assert_eq!(
        authority(&mut conn, stranger, t.scene_id, None),
        ShapeAuthority::None
    );
    assert_eq!(
        authority(&mut conn, stranger, t.scene_id, Some(stranger)),
        ShapeAuthority::None
    );
}

#[test]
fn a_site_admin_writes_as_a_dm_and_a_missing_scene_as_nobody() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = table(&mut conn);
    let admin = insert_test_user(&mut conn);

    assert_eq!(
        shape_authority(&mut conn, admin, true, t.scene_id, None).unwrap(),
        ShapeAuthority::Dm
    );
    assert_eq!(
        shape_authority(&mut conn, t.owner_id, false, Uuid::now_v7(), None).unwrap(),
        ShapeAuthority::None
    );
}
