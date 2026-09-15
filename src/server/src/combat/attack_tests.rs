//! Spec 046 T058: an attack's rules — C1, C2, C3, C4, C5, C10, multiattack,
//! and where a copy's defence comes from.
//!
//! C6 and C7/C8 through an offer are `offers_tests`; C7 and C8 themselves are
//! `hit_points_tests`; C9 is Phase 8's. Redaction (§3) is `redaction_tests`.

use super::*;
use crate::combat::fixtures::*;
use crate::test_support::test_app_state;

fn events(conn: &mut PgConnection, world_id: Uuid, code: i32) -> Vec<serde_json::Value> {
    use crate::schema::world_events;
    world_events::table
        .filter(world_events::world_id.eq(world_id))
        .filter(world_events::event_code.eq(code))
        .select(world_events::token_event)
        .load::<Option<serde_json::Value>>(conn)
        .expect("events")
        .into_iter()
        .flatten()
        .collect()
}

fn roll_records(conn: &mut PgConnection, world_id: Uuid) -> i64 {
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .count()
        .get_result(conn)
        .expect("count rolls")
}

// ---------------------------------------------------------------------------
// C2: control
// ---------------------------------------------------------------------------

#[test]
fn c2_a_player_cannot_attack_with_a_creature_they_do_not_control() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);

    // Aria is the player's; the stranger controls nothing.
    let refused = attack(&mut conn, t.stranger, t.aria, t.longsword, Some(t.goblin)).unwrap_err();
    assert_eq!(refused, FightRefusal::NotControlled);
    assert_eq!(refused.message(), "You do not control that creature");
    // Nor may the player swing the ogre's greatclub from the ogre.
    let refused = attack(&mut conn, t.player, t.ogre, t.greatclub, Some(t.aria)).unwrap_err();
    assert_eq!(refused, FightRefusal::NotControlled);
    assert_eq!(attacks_in(&mut conn, t.scene_id), 0, "nothing is recorded");
    assert_eq!(roll_records(&mut conn, t.world_id), 0, "nothing is rolled");

    // The controller, and a Game Master for any creature, may.
    attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("Aria's player");
    attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("the Game Master");
}

#[test]
fn a_player_attacks_only_with_what_their_creature_has() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let refused = attack(&mut conn, t.player, t.aria, t.greatclub, Some(t.goblin)).unwrap_err();
    assert!(matches!(refused, FightRefusal::NotFound(_)), "{refused:?}");
}

// ---------------------------------------------------------------------------
// C1: the turn
// ---------------------------------------------------------------------------

#[test]
fn c1_an_attack_on_somebody_elses_turn_is_refused_names_whose_and_spends_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    fight(&mut conn, &t, &[(t.aria, "Aria"), (t.ogre, "Ogre")], t.ogre);

    let refused = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).unwrap_err();
    assert_eq!(
        refused,
        FightRefusal::NotYourTurn("It is Ogre's turn".into())
    );
    assert_eq!(attacks_in(&mut conn, t.scene_id), 0);
    assert_eq!(roll_records(&mut conn, t.world_id), 0, "no die was rolled");
    assert_eq!(offers_in(&mut conn, t.world_id), 0);
}

#[test]
fn c1_a_reaction_is_not_held_to_the_turn() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    fight(&mut conn, &t, &[(t.aria, "Aria"), (t.ogre, "Ogre")], t.ogre);

    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.player,
        false,
        &AttackRequest {
            attacker_token_id: t.aria,
            ability_id: Some(t.longsword),
            target_token_id: Some(t.ogre),
            action_cost: Some(ActionCost::Reaction),
            ..Default::default()
        },
        &mut rng(),
    )
    .expect("a reaction between turns");
    assert_eq!(
        attack_row(&mut conn, made.attack_ids[0]).action_cost,
        "reaction"
    );
}

#[test]
fn c1_a_game_master_is_never_held_to_the_turn() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    fight(&mut conn, &t, &[(t.aria, "Aria"), (t.ogre, "Ogre")], t.aria);
    attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("on Aria's turn");
}

// ---------------------------------------------------------------------------
// The roll, the defence and the outcome
// ---------------------------------------------------------------------------

#[test]
fn a_hit_is_recorded_with_attacker_target_total_defence_and_damage() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);

    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.ogre)).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.attacker_token_id, Some(t.aria));
    assert_eq!(row.target_token_id, Some(t.ogre));
    assert_eq!(row.outcome, OUTCOME_HIT);
    assert_eq!(
        row.defence,
        Some(OGRE_AC),
        "the ogre's armour class, read from its sheet"
    );
    assert_eq!(row.ability_name, "Longsword");
    assert!(row.to_hit_roll_id.is_some() && row.damage_roll_id.is_some());
    assert_eq!(row.distance, None, "Phase 7 measures distance");
    assert!(
        row.flags.is_empty(),
        "C3: nothing is flagged, nothing refused"
    );

    // Event 29 carries the attack's id and nothing else (contract §4).
    let announced = events(
        &mut conn,
        t.world_id,
        crate::world_events::EVENT_CODE_ATTACK_MADE,
    );
    assert_eq!(
        announced,
        vec![serde_json::json!({ "attackId": made.attack_ids[0] })]
    );
}

#[test]
fn a_copy_reads_its_defence_from_the_npc_it_copies() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.defence, Some(GOBLIN_AC));
    assert_eq!(row.outcome, OUTCOME_HIT);
}

#[test]
fn a_copy_whose_npc_is_gone_has_no_defence_and_offers_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::delete(world_actors::table.filter(world_actors::id.eq(t.goblin_actor)))
        .execute(&mut conn)
        .expect("delete the goblin NPC");

    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.defence, None);
    assert_eq!(row.outcome, OUTCOME_NO_DEFENCE);
    assert_eq!(row.damage_roll_id, None);
    assert!(made.offer_ids.is_empty());
    assert_eq!(
        copy_hp(&mut conn, t.goblin),
        GOBLIN_HP as i64,
        "and nothing changed"
    );
}

// ---------------------------------------------------------------------------
// C4: nothing is offered for a miss, or at nothing
// ---------------------------------------------------------------------------

#[test]
fn c4_a_miss_creates_no_offer_and_rolls_no_damage() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let made = attack(&mut conn, t.player, t.aria, t.whiff, Some(t.goblin)).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.outcome, OUTCOME_MISS);
    assert_eq!(row.damage_roll_id, None);
    assert!(made.offer_ids.is_empty());
    assert_eq!(offers_in(&mut conn, t.world_id), 0);
}

#[test]
fn c4_an_attack_at_nothing_creates_no_offer() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // Auto-apply on, to show a roll into the air applies to nothing either.
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::auto_apply_npc_damage.eq(true))
        .execute(&mut conn)
        .expect("auto-apply on");
    let made = attack(&mut conn, t.gm, t.ogre, t.greatclub, None).expect("attack");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.outcome, OUTCOME_NO_TARGET);
    assert_eq!(row.target_token_id, None);
    assert!(made.offer_ids.is_empty());
    assert_eq!(offers_in(&mut conn, t.world_id), 0);
}

// ---------------------------------------------------------------------------
// C5: a hit offers, or auto-applies to what the Game Master runs
// ---------------------------------------------------------------------------

#[test]
fn c5_a_hit_creates_a_pending_offer_and_changes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    assert_eq!(made.offer_ids.len(), 1);
    let offer = offer_row(&mut conn, made.offer_ids[0]);
    assert_eq!(offer.status, OFFER_PENDING);
    assert_eq!(offer.kind, OFFER_DAMAGE);
    assert_eq!(offer.amount, 5);
    assert_eq!(offer.target_token_id, t.goblin);
    assert!(!offer.target_linked, "made against a copy");
    assert_eq!(copy_hp(&mut conn, t.goblin), GOBLIN_HP as i64);
    let announced = events(
        &mut conn,
        t.world_id,
        crate::world_events::EVENT_CODE_OFFER_CHANGED,
    );
    assert_eq!(announced, vec![serde_json::json!({ "offerId": offer.id })]);
}

#[test]
fn c5_auto_apply_applies_a_hit_on_a_game_master_npc_in_the_same_action() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::auto_apply_npc_damage.eq(true))
        .execute(&mut conn)
        .expect("auto-apply on");

    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    let offer = offer_row(&mut conn, made.offer_ids[0]);
    assert_eq!(offer.status, OFFER_APPLIED);
    assert!(offer.resolved_at.is_some());
    assert_eq!(copy_hp(&mut conn, t.goblin), (GOBLIN_HP - 5) as i64);
}

#[test]
fn c5_damage_to_a_creature_a_player_controls_is_always_an_offer() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::auto_apply_npc_damage.eq(true))
        .execute(&mut conn)
        .expect("auto-apply on");
    let combat = fight(&mut conn, &t, &[(t.ogre, "Ogre"), (t.aria, "Aria")], t.ogre);
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat)))
        .set(world_combats::auto_apply.eq(Some(true)))
        .execute(&mut conn)
        .expect("and on for this encounter");

    let made = attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.aria)).expect("attack");
    let offer = offer_row(&mut conn, made.offer_ids[0]);
    assert_eq!(offer.status, OFFER_PENDING, "Aria is her player's to take");
    assert_eq!(actor_hp(&mut conn, t.aria_actor), ARIA_HP as i64);

    // An NPC a Game Master handed to a player is that player's, too.
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.goblin)))
        .set(tokens::owner_user_id.eq(Some(t.stranger)))
        .execute(&mut conn)
        .expect("give the goblin away");
    let made = attack(&mut conn, t.gm, t.ogre, t.greatclub, Some(t.goblin)).expect("attack");
    assert_eq!(
        offer_row(&mut conn, made.offer_ids[0]).status,
        OFFER_PENDING
    );
}

#[test]
fn c5_the_encounter_override_decides_over_the_world_setting() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // The world says no; this encounter says yes.
    let combat = fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.goblin, "Goblin")],
        t.aria,
    );
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat)))
        .set(world_combats::auto_apply.eq(Some(true)))
        .execute(&mut conn)
        .expect("override on");
    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    assert_eq!(
        offer_row(&mut conn, made.offer_ids[0]).status,
        OFFER_APPLIED
    );

    // And the other way: the world says yes, this encounter says no.
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::auto_apply_npc_damage.eq(true))
        .execute(&mut conn)
        .expect("world on");
    diesel::update(world_combats::table.filter(world_combats::id.eq(combat)))
        .set(world_combats::auto_apply.eq(Some(false)))
        .execute(&mut conn)
        .expect("override off");
    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("attack");
    assert_eq!(
        offer_row(&mut conn, made.offer_ids[0]).status,
        OFFER_PENDING
    );
}

#[test]
fn auto_apply_is_skipped_without_line_of_sight_unless_the_attack_ignores_walls() {
    // Phase 7 sets the flag; the rule that reads it is here.
    let flags = vec!["no_line_of_sight".to_string()];
    assert!(!auto_apply_holds(true, &[], &flags, true));
    assert!(auto_apply_holds(true, &[], &flags, false));
    assert!(auto_apply_holds(true, &[], &[], true));
    assert!(!auto_apply_holds(false, &[], &[], true));
    assert!(!auto_apply_holds(true, &[Uuid::now_v7()], &[], true));
}

#[test]
fn a_creature_with_no_hit_points_recorded_leaves_an_auto_applied_offer_pending() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::auto_apply_npc_damage.eq(true))
        .execute(&mut conn)
        .expect("auto-apply on");
    // A copy placed before its NPC's sheet had hit points.
    let bare = place(
        &mut conn,
        t.scene_id,
        Some(t.goblin_actor),
        Some(t.gm),
        false,
        None,
        "Bare",
        (50.0, 0.0),
    );
    let made =
        attack(&mut conn, t.player, t.aria, t.longsword, Some(bare)).expect("the attack stands");
    assert_eq!(
        offer_row(&mut conn, made.offer_ids[0]).status,
        OFFER_PENDING
    );
}

// ---------------------------------------------------------------------------
// Multiattack (FR-044)
// ---------------------------------------------------------------------------

#[test]
fn a_multiattack_makes_one_row_per_part_each_against_its_own_target() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let fist = ability(&mut conn, t.world_id, t.gm, "Fist", "1d20+100", "2");
    let multi = ability(&mut conn, t.world_id, t.gm, "Multiattack", "1d20", "0");
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(multi)))
        .set(world_abilities::multiattack.eq(vec![Some(t.greatclub), Some(fist)]))
        .execute(&mut conn)
        .expect("name its parts");

    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &AttackRequest {
            attacker_token_id: t.ogre,
            ability_id: Some(multi),
            target_token_id: Some(t.aria),
            targets: Some(vec![t.aria, t.goblin]),
            ..Default::default()
        },
        &mut rng(),
    )
    .expect("multiattack");
    assert_eq!(made.attack_ids.len(), 2);
    let first = attack_row(&mut conn, made.attack_ids[0]);
    let second = attack_row(&mut conn, made.attack_ids[1]);
    assert_eq!(first.ability_name, "Greatclub");
    assert_eq!(first.target_token_id, Some(t.aria));
    assert_eq!(first.multiattack_of, None, "the first part is the parent");
    assert_eq!(second.ability_name, "Fist");
    assert_eq!(second.target_token_id, Some(t.goblin));
    assert_eq!(second.multiattack_of, Some(first.id));
    assert_eq!(made.offer_ids.len(), 2, "each hit its own offer");
}

// ---------------------------------------------------------------------------
// C10: a paused world
// ---------------------------------------------------------------------------

#[test]
fn c10_a_paused_world_refuses_an_attack_before_anything_else() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    pause(&mut conn, &t);
    // Even from somebody who controls nothing: the pause speaks first.
    let refused = attack(&mut conn, t.stranger, t.aria, t.longsword, Some(t.goblin)).unwrap_err();
    assert!(matches!(refused, FightRefusal::Paused(_)), "{refused:?}");
    let refused = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).unwrap_err();
    assert!(matches!(refused, FightRefusal::Paused(_)), "{refused:?}");
    assert_eq!(attacks_in(&mut conn, t.scene_id), 0);
}
