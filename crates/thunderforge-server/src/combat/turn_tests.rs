//! Spec 046 T026: C1, the turn check.
//!
//! The offline replay path is proved beside the rest of reconcile's rules, in
//! `graphql/mutations_reconcile_tests.rs`
//! (`a_queued_move_on_somebody_elses_turn_is_refused_and_says_whose`), where
//! `apply_one` can be reached.

use super::*;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

pub(crate) struct Fight {
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub gm: Uuid,
    pub player: Uuid,
    pub hero: Uuid,
    pub ogre: Uuid,
    pub bystander: Uuid,
    pub combat_id: Uuid,
    pub hero_combatant: Uuid,
    pub ogre_combatant: Uuid,
}

fn token(conn: &mut PgConnection, scene_id: Uuid, owner: Option<Uuid>) -> Uuid {
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(id),
            tokens::scene_id.eq(scene_id),
            tokens::x.eq(0.0),
            tokens::y.eq(0.0),
            tokens::owner_user_id.eq(owner),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("insert token");
    id
}

fn combatant(
    conn: &mut PgConnection,
    combat_id: Uuid,
    token_id: Uuid,
    label: &str,
    initiative: i32,
) -> Uuid {
    let id = Uuid::now_v7();
    diesel::insert_into(world_combatants::table)
        .values(&crate::models::NewCombatant {
            id,
            combat_id,
            actor_id: None,
            token_id: Some(token_id),
            label: label.into(),
            initiative,
            tiebreak: 0,
            is_npc: false,
        })
        .execute(conn)
        .expect("insert combatant");
    id
}

/// A Game Master, a player with a hero, an ogre, and a bystander token the
/// player also owns that is not in the fight. The ogre's turn.
pub(crate) fn fight(conn: &mut PgConnection) -> Fight {
    let gm = insert_test_user(conn);
    let player = insert_test_user(conn);
    let world_id = insert_test_world(conn, gm);
    insert_test_world_member(conn, world_id, player, "Player");
    let scene_id = insert_test_scene(conn, world_id, gm);
    let hero = token(conn, scene_id, Some(player));
    let ogre = token(conn, scene_id, None);
    let bystander = token(conn, scene_id, Some(player));

    let combat_id = Uuid::now_v7();
    diesel::insert_into(world_combats::table)
        .values(&crate::models::NewCombat {
            id: combat_id,
            world_id,
            scene_id: Some(scene_id),
            created_by: gm,
        })
        .execute(conn)
        .expect("start combat");
    let hero_combatant = combatant(conn, combat_id, hero, "Aria", 10);
    let ogre_combatant = combatant(conn, combat_id, ogre, "Ogre", 15);
    set_turn(conn, combat_id, ogre_combatant);

    Fight {
        world_id,
        scene_id,
        gm,
        player,
        hero,
        ogre,
        bystander,
        combat_id,
        hero_combatant,
        ogre_combatant,
    }
}

pub(crate) fn set_turn(conn: &mut PgConnection, combat_id: Uuid, combatant: Uuid) {
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat_id)))
        .set(world_combats::active_combatant_id.eq(Some(combatant)))
        .execute(conn)
        .expect("set turn");
}

#[test]
fn a_player_is_refused_on_somebody_elses_turn_and_told_whose() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);

    let check = turn_check(&mut conn, f.scene_id, f.hero, f.player, false).expect("checked");
    assert!(!check.allowed);
    assert_eq!(check.active_label.as_deref(), Some("Ogre"));
    assert_eq!(check.refusal().as_deref(), Some("It is Ogre's turn"));
}

#[test]
fn a_player_is_allowed_on_their_own_turn() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);
    set_turn(&mut conn, f.combat_id, f.hero_combatant);

    let check = turn_check(&mut conn, f.scene_id, f.hero, f.player, false).expect("checked");
    assert!(check.allowed);
    assert_eq!(check.refusal(), None);
}

#[test]
fn a_game_master_is_never_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);
    set_turn(&mut conn, f.combat_id, f.ogre_combatant);

    // Moving the hero on the ogre's turn, as the world's owner.
    assert!(
        turn_check(&mut conn, f.scene_id, f.hero, f.gm, false)
            .expect("checked")
            .allowed
    );
    // And a Game Master who is a member rather than the owner.
    let second_gm = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, f.world_id, second_gm, "GM");
    assert!(
        turn_check(&mut conn, f.scene_id, f.hero, second_gm, false)
            .expect("checked")
            .allowed
    );
}

#[test]
fn a_token_that_is_not_in_the_fight_moves_freely() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);

    let check = turn_check(&mut conn, f.scene_id, f.bystander, f.player, false).expect("checked");
    assert!(check.allowed);
}

#[test]
fn a_hidden_name_is_unknown_in_the_refusal() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);
    diesel::update(tokens::table.filter(tokens::token_id.eq(f.ogre)))
        .set(tokens::name_visible_to_players.eq(false))
        .execute(&mut conn)
        .expect("hide name");

    let check = turn_check(&mut conn, f.scene_id, f.hero, f.player, false).expect("checked");
    assert_eq!(check.refusal().as_deref(), Some("It is Unknown's turn"));
}

#[test]
fn no_turn_taken_yet_and_no_combat_hold_nobody() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let f = fight(&mut conn);

    diesel::update(world_combats::table.filter(world_combats::id.eq(f.combat_id)))
        .set(world_combats::active_combatant_id.eq(None::<Uuid>))
        .execute(&mut conn)
        .expect("clear turn");
    assert!(
        turn_check(&mut conn, f.scene_id, f.hero, f.player, false)
            .expect("checked")
            .allowed,
        "a combat nobody has started the turns of holds nobody"
    );

    set_turn(&mut conn, f.combat_id, f.ogre_combatant);
    diesel::update(world_combats::table.filter(world_combats::id.eq(f.combat_id)))
        .set(world_combats::ended_at.eq(Some(chrono::Utc::now().naive_utc())))
        .execute(&mut conn)
        .expect("end combat");
    assert!(
        turn_check(&mut conn, f.scene_id, f.hero, f.player, false)
            .expect("checked")
            .allowed,
        "an ended combat holds nobody"
    );
}
