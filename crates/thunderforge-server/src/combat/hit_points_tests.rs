//! Spec 046 T014 (C7, M1) and T015 (C8): the hit-point operation.
//!
//! The arithmetic is tested alone; everything that needs a lock, a validator
//! or a combatant runs against the database, committed rather than inside a
//! test transaction, because "two changes that land together both land" is a
//! claim about two connections.

use super::*;
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world, test_app_state,
};
use serde_json::json;

const SYSTEMS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packs/systems");

fn hp(current: i32, max: i32, temporary: i32) -> HitPoints {
    HitPoints {
        current,
        max,
        temporary,
    }
}

// ---------------------------------------------------------------------------
// C7: the arithmetic
// ---------------------------------------------------------------------------

#[test]
fn c7_temporary_hit_points_absorb_damage_first() {
    assert_eq!(
        apply_to(hp(10, 10, 4), HitPointChangeKind::Damage, 3),
        hp(10, 10, 1)
    );
    assert_eq!(
        apply_to(hp(10, 10, 4), HitPointChangeKind::Damage, 6),
        hp(8, 10, 0)
    );
}

#[test]
fn c7_current_never_goes_below_zero() {
    assert_eq!(
        apply_to(hp(7, 7, 0), HitPointChangeKind::Damage, 50),
        hp(0, 7, 0)
    );
}

#[test]
fn c7_healing_never_exceeds_the_maximum_and_leaves_temporary_alone() {
    assert_eq!(
        apply_to(hp(2, 7, 3), HitPointChangeKind::Healing, 40),
        hp(7, 7, 3)
    );
    // A sheet holding more than its maximum is not cut down by a heal.
    assert_eq!(
        apply_to(hp(9, 7, 0), HitPointChangeKind::Healing, 1),
        hp(9, 7, 0)
    );
}

#[test]
fn the_declared_fields_are_read_and_written_and_nothing_else_is_touched() {
    let declared = SystemHitPoints {
        slot: "resourceData".into(),
        current: "current_hp".into(),
        max: "max_hp".into(),
        temporary: Some("temporary_hp".into()),
    };
    let slot = json!({ "max_hp": 7, "hit_dice": "2d6" });
    // No current recorded: at full.
    assert_eq!(read_hit_points(&slot, &declared).unwrap(), hp(7, 7, 0));
    let written = write_hit_points(&slot, &declared, hp(2, 7, 0));
    assert_eq!(
        written,
        json!({ "max_hp": 7, "hit_dice": "2d6", "current_hp": 2 })
    );
    assert!(read_hit_points(&json!({}), &declared).is_err());
}

// ---------------------------------------------------------------------------
// Against the database
// ---------------------------------------------------------------------------

struct Creature {
    world_id: Uuid,
    token_id: Uuid,
    actor_id: Uuid,
    user_id: Uuid,
}

fn creature(conn: &mut PgConnection, system: &str, resources: serde_json::Value) -> Creature {
    let user_id = insert_test_user(conn);
    let world_id = insert_test_world(conn, user_id);
    let scene_id = insert_test_scene(conn, world_id, user_id);
    let actor_id = insert_test_actor(conn, world_id, scene_id, user_id);
    diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
        .set(world_actors::game_system_id.eq(system))
        .execute(conn)
        .expect("set system");
    diesel::insert_into(world_actor_system_data::table)
        .values((
            world_actor_system_data::actor_id.eq(actor_id),
            world_actor_system_data::game_system_id.eq(system),
            world_actor_system_data::resource_data.eq(resources),
            world_actor_system_data::created_by.eq(user_id),
            world_actor_system_data::updated_by.eq(user_id),
        ))
        .execute(conn)
        .expect("insert system data");
    let token_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(token_id),
            tokens::scene_id.eq(scene_id),
            tokens::actor_id.eq(Some(actor_id)),
            tokens::x.eq(0.0),
            tokens::y.eq(0.0),
            tokens::rotation.eq(0.0),
            tokens::scale.eq(1.0),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("insert token");
    Creature {
        world_id,
        token_id,
        actor_id,
        user_id,
    }
}

fn goblin(conn: &mut PgConnection) -> Creature {
    creature(conn, "dnd5e", json!({ "current_hp": 7, "max_hp": 7 }))
}

fn stored(conn: &mut PgConnection, actor_id: Uuid) -> serde_json::Value {
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(world_actor_system_data::resource_data)
        .first::<Option<serde_json::Value>>(conn)
        .expect("read")
        .expect("present")
}

/// A running combat with this creature in it; returns the combatant id.
fn in_combat(conn: &mut PgConnection, c: &Creature) -> Uuid {
    let combat_id = Uuid::now_v7();
    diesel::insert_into(world_combats::table)
        .values(&crate::models::NewCombat {
            id: combat_id,
            world_id: c.world_id,
            scene_id: None,
            created_by: c.user_id,
        })
        .execute(conn)
        .expect("start combat");
    let combatant_id = Uuid::now_v7();
    diesel::insert_into(world_combatants::table)
        .values(&crate::models::NewCombatant {
            id: combatant_id,
            combat_id,
            actor_id: Some(c.actor_id),
            token_id: Some(c.token_id),
            label: "Goblin".into(),
            initiative: 10,
            tiebreak: 0,
            is_npc: true,
        })
        .execute(conn)
        .expect("add combatant");
    combatant_id
}

fn combatant_state(conn: &mut PgConnection, id: Uuid) -> (bool, Option<String>) {
    world_combatants::table
        .filter(world_combatants::id.eq(id))
        .select((world_combatants::active, world_combatants::downed_by))
        .first(conn)
        .expect("combatant")
}

fn change(
    c: &Creature,
    conn: &mut PgConnection,
    kind: HitPointChangeKind,
    amount: i32,
) -> HitPoints {
    apply_hit_point_change(conn, SYSTEMS_DIR, c.token_id, kind, amount, c.user_id)
        .expect("applied")
        .after
}

#[test]
fn damage_is_written_to_the_actor_and_announced_as_a_sheet_change() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = creature(
        &mut conn,
        "dnd5e",
        json!({ "current_hp": 7, "max_hp": 7, "temporary_hp": 2 }),
    );

    let after = change(&c, &mut conn, HitPointChangeKind::Damage, 5);
    assert_eq!(after, hp(4, 7, 0));
    assert_eq!(
        stored(&mut conn, c.actor_id),
        json!({ "current_hp": 4, "max_hp": 7, "temporary_hp": 0 })
    );

    use crate::schema::world_events;
    let payload = world_events::table
        .filter(world_events::world_id.eq(c.world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_ACTOR_SHEET_CHANGED))
        .select(world_events::token_event)
        .first::<Option<serde_json::Value>>(&mut conn)
        .expect("event 26 recorded")
        .expect("with a payload");
    assert_eq!(payload["actorId"], json!(c.actor_id));
    assert_eq!(payload["dataType"], json!("resource_data"));
}

#[test]
fn c7_healing_is_bounded_at_the_maximum_in_the_store() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = creature(&mut conn, "dnd5e", json!({ "current_hp": 1, "max_hp": 7 }));
    assert_eq!(
        change(&c, &mut conn, HitPointChangeKind::Healing, 99),
        hp(7, 7, 0)
    );
    assert_eq!(stored(&mut conn, c.actor_id)["current_hp"], json!(7));
}

#[test]
fn a_negative_amount_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = goblin(&mut conn);
    let error = apply_hit_point_change(
        &mut conn,
        SYSTEMS_DIR,
        c.token_id,
        HitPointChangeKind::Damage,
        -3,
        c.user_id,
    )
    .expect_err("refused");
    assert!(error.contains("negative"), "{error}");
}

#[test]
fn m1_a_system_without_declared_hit_points_refuses_with_a_clear_message() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    // Blades in the Dark tracks stress and trauma, and declares no hit points.
    let c = creature(&mut conn, "blades_in_the_dark", json!({ "stress": 2 }));
    let error = apply_hit_point_change(
        &mut conn,
        SYSTEMS_DIR,
        c.token_id,
        HitPointChangeKind::Damage,
        3,
        c.user_id,
    )
    .expect_err("refused");
    assert!(error.contains("declares no hit points"), "{error}");
    assert_eq!(stored(&mut conn, c.actor_id), json!({ "stress": 2 }));
}

#[test]
fn two_concurrent_changes_both_land() {
    let state = test_app_state();
    let c = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            "dnd5e",
            json!({ "current_hp": 30, "max_hp": 30 }),
        )
    };

    // Twenty hits of 1 across four threads. Without the row lock, two that
    // read 30 together both write 29 and one hit vanishes.
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let pool = state.db_pool.clone();
            let barrier = barrier.clone();
            let (token_id, user_id) = (c.token_id, c.user_id);
            std::thread::spawn(move || {
                let mut conn = pool.get().expect("conn");
                barrier.wait();
                for _ in 0..5 {
                    apply_hit_point_change(
                        &mut conn,
                        SYSTEMS_DIR,
                        token_id,
                        HitPointChangeKind::Damage,
                        1,
                        user_id,
                    )
                    .expect("applied");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread");
    }

    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(stored(&mut conn, c.actor_id)["current_hp"], json!(10));
}

// ---------------------------------------------------------------------------
// C8: zero is out
// ---------------------------------------------------------------------------

#[test]
fn c8_reaching_zero_marks_the_combatant_out_by_hit_points() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = goblin(&mut conn);
    let combatant = in_combat(&mut conn, &c);

    let outcome = apply_hit_point_change(
        &mut conn,
        SYSTEMS_DIR,
        c.token_id,
        HitPointChangeKind::Damage,
        5,
        c.user_id,
    )
    .expect("applied");
    assert_eq!(outcome.combat_changed, None, "2 left is still in the fight");
    assert_eq!(combatant_state(&mut conn, combatant), (true, None));

    let outcome = apply_hit_point_change(
        &mut conn,
        SYSTEMS_DIR,
        c.token_id,
        HitPointChangeKind::Damage,
        2,
        c.user_id,
    )
    .expect("applied");
    assert!(outcome.combat_changed.is_some());
    assert_eq!(
        combatant_state(&mut conn, combatant),
        (false, Some(DOWNED_BY_HIT_POINTS.to_string()))
    );
}

#[test]
fn c8_healing_brings_back_a_creature_hit_points_took_out() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = goblin(&mut conn);
    let combatant = in_combat(&mut conn, &c);

    change(&c, &mut conn, HitPointChangeKind::Damage, 7);
    assert!(!combatant_state(&mut conn, combatant).0);
    change(&c, &mut conn, HitPointChangeKind::Healing, 3);
    assert_eq!(combatant_state(&mut conn, combatant), (true, None));
}

#[test]
fn c8_healing_never_undoes_a_game_masters_down() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = goblin(&mut conn);
    let combatant = in_combat(&mut conn, &c);

    // The tracker's Down, as `update_combatant` writes it.
    diesel::update(world_combatants::table.filter(world_combatants::id.eq(combatant)))
        .set((
            world_combatants::active.eq(false),
            world_combatants::downed_by.eq(Some(DOWNED_BY_GAME_MASTER)),
        ))
        .execute(&mut conn)
        .expect("down");

    change(&c, &mut conn, HitPointChangeKind::Damage, 7);
    change(&c, &mut conn, HitPointChangeKind::Healing, 3);
    assert_eq!(
        combatant_state(&mut conn, combatant),
        (false, Some(DOWNED_BY_GAME_MASTER.to_string())),
        "a creature the Game Master took out stays out when healed"
    );
}

#[test]
fn c8_a_creature_with_no_running_combat_changes_nothing_else() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let c = goblin(&mut conn);
    let outcome = apply_hit_point_change(
        &mut conn,
        SYSTEMS_DIR,
        c.token_id,
        HitPointChangeKind::Damage,
        7,
        c.user_id,
    )
    .expect("applied");
    assert_eq!(outcome.after.current, 0);
    assert_eq!(outcome.combat_changed, None);
}
