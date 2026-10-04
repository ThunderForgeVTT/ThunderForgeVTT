//! Spec 046 T052: who controls a creature (research R7).

use super::*;
use crate::combat::fixtures::*;
use crate::test_support::test_app_state;

fn control(conn: &mut PgConnection, token: Uuid) -> TokenControl {
    token_control(conn, token).expect("load").expect("present")
}

#[test]
fn a_token_is_controlled_by_its_owner_and_by_owners_of_its_actor() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let aria = control(&mut conn, t.aria);
    assert!(may_move(&mut conn, t.player, false, &aria).unwrap());
    assert!(!may_move(&mut conn, t.stranger, false, &aria).unwrap());
    assert_eq!(
        player_controllers(&mut conn, &aria).unwrap(),
        vec![t.player]
    );

    // Owner on the actor makes the stranger a controller too.
    diesel::insert_into(world_actor_permissions::table)
        .values((
            world_actor_permissions::id.eq(Uuid::now_v7()),
            world_actor_permissions::actor_id.eq(t.aria_actor),
            world_actor_permissions::user_id.eq(t.stranger),
            world_actor_permissions::level.eq("Owner"),
            world_actor_permissions::created_at.eq(chrono::Utc::now().naive_utc()),
            world_actor_permissions::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .expect("grant");
    assert!(may_move(&mut conn, t.stranger, false, &aria).unwrap());
    let mut expected = vec![t.player, t.stranger];
    expected.sort();
    assert_eq!(player_controllers(&mut conn, &aria).unwrap(), expected);
    assert!(
        controlled_tokens_in_scene(&mut conn, t.stranger, t.scene_id)
            .unwrap()
            .contains(&t.aria)
    );
}

#[test]
fn a_creature_no_player_controls_is_the_game_masters() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let goblin = control(&mut conn, t.goblin);
    assert!(
        player_controllers(&mut conn, &goblin).unwrap().is_empty(),
        "owned by the Game Master, who is not a player controller"
    );
    assert!(may_act_for(&mut conn, t.gm, false, &goblin).unwrap());
    let aria = control(&mut conn, t.aria);
    assert!(may_act_for(&mut conn, t.gm, false, &aria).unwrap());
    assert!(!may_act_for(&mut conn, t.player, false, &goblin).unwrap());
}
