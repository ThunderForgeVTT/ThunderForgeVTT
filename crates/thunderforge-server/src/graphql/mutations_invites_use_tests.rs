//! The owner's decision on campaign links: a use counts only when someone
//! becomes a new member of the world, in the same transaction as the
//! membership. Opening the link, viewing the join page, being a member
//! already, and a join that fails all leave the count where it was.

use super::tests::{insert_test_invite, load_invite};
use super::*;
use crate::graphql::queries::invite::{already_member_impl, world_by_invite_code_impl};
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

fn uses(state: &AppState, invite_id: Uuid) -> i32 {
    let mut conn = state.db_pool.get().unwrap();
    load_invite(&mut conn, invite_id).used_count
}

fn memberships(state: &AppState, world_id: Uuid, user_id: Uuid) -> i64 {
    let mut conn = state.db_pool.get().unwrap();
    world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// What the join page does on every load, for whoever loads it: resolve the
/// world, and ask whether the viewer already belongs. Neither is a join.
#[tokio::test]
async fn viewing_the_join_page_burns_no_use() {
    let state = test_app_state();
    let (invite_id, code, visitor) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let (id, code) = insert_test_invite(&mut conn, world, owner, 1, 0, None);
        (id, code, insert_test_user(&mut conn))
    };

    for _ in 0..3 {
        let preview = world_by_invite_code_impl(&state, &code).await.unwrap();
        assert!(preview.is_some(), "a live link must preview its world");
        assert!(!already_member_impl(&state, visitor, &code).await.unwrap());
    }
    assert_eq!(uses(&state, invite_id), 0, "a page view must burn nothing");
}

/// The world's owner has no `world_members` row — ownership is
/// `worlds.created_by`. Opening their own link must read as "already a
/// member", and pressing Join must neither burn a use nor file the owner as a
/// Player of their own world.
#[tokio::test]
async fn the_owner_opening_their_own_link_burns_nothing() {
    let state = test_app_state();
    let (world, owner, invite_id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let (id, code) = insert_test_invite(&mut conn, world, owner, 1, 0, None);
        (world, owner, id, code)
    };

    assert!(
        already_member_impl(&state, owner, &code).await.unwrap(),
        "the owner already belongs to the world"
    );

    let err = join_world_impl(&state, owner, JoinWorldInput { invite_code: code })
        .await
        .expect_err("the owner cannot join their own world");
    assert_eq!(err.message, ALREADY_A_MEMBER_MESSAGE);
    assert_eq!(
        uses(&state, invite_id),
        0,
        "the owner's click must burn nothing"
    );
    assert_eq!(memberships(&state, world, owner), 0);
}

/// A join that fails after the link was judged usable rolls the use back with
/// it. The failure here is the membership insert itself (an account that no
/// longer exists), which is the last thing the transaction does.
#[tokio::test]
async fn a_failed_join_burns_nothing() {
    let state = test_app_state();
    let (invite_id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_test_invite(&mut conn, world, owner, 1, 0, None)
    };

    let gone = Uuid::now_v7();
    join_world_impl(&state, gone, JoinWorldInput { invite_code: code })
        .await
        .expect_err("a join whose membership cannot be written must fail");
    assert_eq!(
        uses(&state, invite_id),
        0,
        "a failed join must burn nothing"
    );
}

/// A join burns exactly one use — one membership, one use.
#[tokio::test]
async fn a_join_burns_exactly_one_use() {
    let state = test_app_state();
    let (world, invite_id, code, joiner) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let (id, code) = insert_test_invite(&mut conn, world, owner, 3, 0, None);
        (world, id, code, insert_test_user(&mut conn))
    };

    join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
        .await
        .expect("a live link admits a new member");
    assert_eq!(uses(&state, invite_id), 1);
    assert_eq!(memberships(&state, world, joiner), 1);
}

/// A double click that lands twice at once: one join, one use, and the
/// second answer says what happened rather than calling the link dead.
#[tokio::test]
async fn the_same_person_joining_twice_at_once_burns_one_use() {
    let state = test_app_state();
    let (world, invite_id, code, joiner) = {
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let (id, code) = insert_test_invite(&mut conn, world, owner, 5, 0, None);
        (world, id, code, insert_test_user(&mut conn))
    };

    let mut handles = Vec::new();
    for _ in 0..2 {
        let state = state.clone();
        let code = code.clone();
        handles.push(tokio::spawn(async move {
            join_world_impl(&state, joiner, JoinWorldInput { invite_code: code }).await
        }));
    }
    let mut joined = 0;
    for h in handles {
        match h.await.unwrap() {
            Ok(_) => joined += 1,
            Err(e) => assert_eq!(e.message, ALREADY_A_MEMBER_MESSAGE),
        }
    }
    assert_eq!(joined, 1);
    assert_eq!(uses(&state, invite_id), 1, "one membership, one use");
    assert_eq!(memberships(&state, world, joiner), 1);
}
