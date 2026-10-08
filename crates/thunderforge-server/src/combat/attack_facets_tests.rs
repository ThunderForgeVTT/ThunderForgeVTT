//! Spec 084 US1: an attack's to-hit is rolled with advantage or
//! disadvantage, judged on the die kept, and says whose attack it was.

use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::attack::{ActionCost, AttackRequest, Attacker, FightRefusal, make_attack};
use crate::combat::fixtures::*;
use crate::combat::lair::LAIR_NO_ADVANTAGE;
use crate::graphql::mutations_combat::{CombatantKind, StartCombatInput, start_combat_impl};
use crate::graphql::mutations_combat_lair::add_lair_combatant_impl;
use crate::models::RollRecord;
use crate::schema::world_roll_records;
use crate::test_support::test_app_state;
use thunderforge_canvas_core::roll_facets::Advantage;
use thunderforge_dice::RollResolution;

fn roll_row(conn: &mut PgConnection, id: Uuid) -> RollRecord {
    world_roll_records::table
        .find(id)
        .select(RollRecord::as_select())
        .first(conn)
        .expect("the roll")
}

fn attack_with(
    conn: &mut PgConnection,
    t: &FightTable,
    ability: Uuid,
    target: Uuid,
    advantage: Advantage,
) -> Result<crate::combat::attack::MadeAttack, FightRefusal> {
    make_attack(
        conn,
        SYSTEMS_DIR,
        t.player,
        false,
        &AttackRequest {
            attacker: Attacker::Token(t.aria),
            ability_id: Some(ability),
            target_token_id: Some(target),
            advantage,
            ..Default::default()
        },
        &mut rng(),
    )
}

#[test]
fn an_attack_with_advantage_rolls_two_d20s_for_the_to_hit_and_names_the_attacker() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let made = attack_with(&mut conn, &t, t.longsword, t.ogre, Advantage::Advantage)
        .expect("Aria attacks with advantage");
    let row = attack_row(&mut conn, made.attack_ids[0]);

    let to_hit = roll_row(&mut conn, row.to_hit_roll_id.expect("a to-hit"));
    assert_eq!(to_hit.formula, "2d20kh1 + 100");
    assert_eq!(to_hit.roll_kind.as_deref(), Some("to_hit"));
    assert_eq!(to_hit.actor_id, Some(t.aria_actor));
    assert_eq!(to_hit.facet_ids(), vec!["advantage".to_string()]);

    // Damage never takes the choice.
    let damage = roll_row(&mut conn, row.damage_roll_id.expect("a hit rolls damage"));
    assert_eq!(damage.formula, "5");
    assert_eq!(damage.roll_kind.as_deref(), Some("damage"));
    assert_eq!(damage.actor_id, Some(t.aria_actor));
    assert!(damage.facet_ids().is_empty());
}

#[test]
fn the_hit_is_judged_on_the_die_kept() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let sling = ability(&mut conn, t.world_id, t.gm, "Sling", "1d20", "3");
    attach(&mut conn, t.aria_actor, sling);

    for (advantage, keep) in [
        (
            Advantage::Advantage,
            Iterator::max as fn(std::vec::IntoIter<i64>) -> Option<i64>,
        ),
        (Advantage::Disadvantage, Iterator::min),
    ] {
        let made = attack_with(&mut conn, &t, sling, t.goblin, advantage).expect("attack");
        let row = attack_row(&mut conn, made.attack_ids[0]);
        let to_hit = roll_row(&mut conn, row.to_hit_roll_id.expect("a to-hit"));
        let resolution: RollResolution = serde_json::from_value(to_hit.detail).expect("detail");
        let dice: Vec<i64> = resolution.dice.iter().map(|d| d.final_value).collect();
        assert_eq!(dice.len(), 2, "{advantage:?} rolls two d20s");
        let kept = keep(dice.into_iter()).expect("two dice") as f64;
        assert_eq!(to_hit.result_value, kept, "{advantage:?} keeps one die");
        let expected = thunderforge_combat::attack::judge(true, Some(GOBLIN_AC), kept);
        assert_eq!(row.outcome, expected, "{advantage:?} is judged on {kept}");
    }
}

#[tokio::test]
async fn a_lairs_attack_has_no_actor_and_refuses_advantage() {
    let mut state = test_app_state();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = start_combat_impl(
        &state,
        t.gm,
        false,
        StartCombatInput {
            world_id: t.world_id,
            scene_id: Some(t.scene_id),
        },
    )
    .await
    .expect("started");
    let combat = add_lair_combatant_impl(&state, t.gm, false, combat.id, "The Crypt".into())
        .await
        .expect("lair");
    let lair = combat
        .combatants
        .iter()
        .find(|c| c.kind == CombatantKind::Lair)
        .expect("lair")
        .id;
    let rocks = ability(
        &mut conn,
        t.world_id,
        t.gm,
        "Falling Rocks",
        "1d20+100",
        "4",
    );
    let request = |advantage| AttackRequest {
        attacker: Attacker::Lair(lair),
        ability_id: Some(rocks),
        target_token_id: Some(t.aria),
        action_cost: Some(ActionCost::Action),
        advantage,
        ..Default::default()
    };

    let refused = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &request(Advantage::Advantage),
        &mut rng(),
    );
    assert_eq!(
        refused.unwrap_err(),
        FightRefusal::Invalid(LAIR_NO_ADVANTAGE.to_string())
    );

    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &request(Advantage::Normal),
        &mut rng(),
    )
    .expect("the lair acts");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    let to_hit = roll_row(&mut conn, row.to_hit_roll_id.expect("a to-hit"));
    assert_eq!(to_hit.formula, "1d20+100");
    assert_eq!(to_hit.actor_id, None);
    assert_eq!(to_hit.roll_kind.as_deref(), Some("to_hit"));
}
