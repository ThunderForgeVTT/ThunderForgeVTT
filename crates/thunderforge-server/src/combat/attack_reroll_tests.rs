//! Spec 084 T056 (US4, research R7): a missed attack rerolled with Heroic
//! Inspiration is the same attack, judged again against the defence it was
//! made against; a hit is never rerolled, and nor is a lair's.
//!
//! The dice are seeded. The first attack's seed is the first that misses
//! the goblin; the reroll's is the first whose replayed d20 hits (or misses)
//! the stored defence, found by replaying exactly as `rerollRoll` does.

use chrono::Utc;
use diesel::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::json;
use uuid::Uuid;

use crate::combat::attack::{ActionCost, AttackRequest, Attacker, MadeAttack, make_attack};
use crate::combat::fixtures::*;
use crate::combat::records::OUTCOME_HIT;
use crate::graphql::mutations_combat::{CombatantKind, StartCombatInput, start_combat_impl};
use crate::graphql::mutations_combat_lair::add_lair_combatant_impl;
use crate::graphql::mutations_reroll::{RerollRequest, reroll_roll_impl};
use crate::graphql::mutations_roll_check::tests::state_with_real_packs;
use crate::models::RollRecord;
use crate::rolls::reroll::{A_HIT, ONLY_MAKER};
use crate::schema::{
    world_actor_permissions, world_actor_system_data, world_attacks, world_combatant_budgets,
    world_combatants, world_events, world_offers, world_roll_records,
};
use crate::state::AppState;
use thunderforge_dice::{
    PlaceholderBindings, Recorded, ReplayEdit, RollResolution, lowest_die, replay,
};

const SLING: &str = "1d20";

fn roll_row(conn: &mut PgConnection, id: Uuid) -> RollRecord {
    world_roll_records::table
        .find(id)
        .select(RollRecord::as_select())
        .first(conn)
        .expect("the roll")
}

fn inspire(conn: &mut PgConnection, actor: Uuid) {
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(actor)),
    )
    .set(world_actor_system_data::trait_data.eq(Some(json!({ "level": 5, "inspiration": true }))))
    .execute(conn)
    .expect("inspire");
}

fn inspired(conn: &mut PgConnection, actor: Uuid) -> serde_json::Value {
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor))
        .select(world_actor_system_data::trait_data)
        .first::<Option<serde_json::Value>>(conn)
        .expect("sheet")
        .unwrap_or_default()["inspiration"]
        .clone()
}

fn editor(conn: &mut PgConnection, actor: Uuid, user: Uuid) {
    let now = Utc::now().naive_utc();
    diesel::insert_into(world_actor_permissions::table)
        .values((
            world_actor_permissions::id.eq(Uuid::now_v7()),
            world_actor_permissions::actor_id.eq(actor),
            world_actor_permissions::user_id.eq(user),
            world_actor_permissions::level.eq("Editor"),
            world_actor_permissions::created_at.eq(now),
            world_actor_permissions::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("editor");
}

fn events(conn: &mut PgConnection, world: Uuid, code: i32) -> i64 {
    world_events::table
        .filter(world_events::world_id.eq(world))
        .filter(world_events::event_code.eq(code))
        .count()
        .get_result(conn)
        .expect("events")
}

fn actions_spent(conn: &mut PgConnection, combat: Uuid) -> Vec<(Uuid, i32)> {
    world_combatant_budgets::table
        .inner_join(world_combatants::table)
        .filter(world_combatants::combat_id.eq(combat))
        .select((
            world_combatant_budgets::combatant_id,
            world_combatant_budgets::action_spent,
        ))
        .order(world_combatant_budgets::combatant_id)
        .load(conn)
        .expect("budgets")
}

/// The first seed whose replay of `to_hit`'s lowest d20, as `rerollRoll`
/// replays it, totals at least `defence` when `hit`, below it otherwise.
fn reroll_seed(to_hit: &RollRecord, defence: i32, hit: bool) -> u64 {
    let resolution: RollResolution = serde_json::from_value(to_hit.detail.clone()).unwrap();
    let bindings: PlaceholderBindings = to_hit
        .bindings
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let die = lowest_die(&resolution, 20).expect("a d20");
    (0..10_000)
        .find(|seed| {
            let mut rng = StdRng::from_rng(&mut StdRng::seed_from_u64(*seed));
            let replayed = replay(
                Recorded {
                    formula: &to_hit.formula,
                    bindings: &bindings,
                    resolution: &resolution,
                },
                None,
                ReplayEdit::RerollDie(die),
                &mut rng,
            )
            .expect("replays");
            (thunderforge_combat::attack::roll_value(&replayed) >= f64::from(defence)) == hit
        })
        .expect("a seed")
}

async fn reroll(
    state: &AppState,
    who: Uuid,
    world: Uuid,
    roll: Uuid,
    seed: u64,
) -> async_graphql::Result<crate::graphql::types::types_rolls::WorldRoll> {
    reroll_roll_impl(
        state,
        who,
        false,
        RerollRequest {
            world_id: world,
            roll_id: roll,
            spend: "inspiration".to_string(),
        },
        &mut StdRng::seed_from_u64(seed),
        Utc::now(),
    )
    .await
}

/// A fight on Aria's turn, the player an Editor of her and inspired, and
/// her sling's first attack on the goblin a miss.
struct Missed {
    state: AppState,
    t: FightTable,
    made: MadeAttack,
    combat: Uuid,
}

fn a_miss() -> Missed {
    let mut state = state_with_real_packs();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    let mut conn = state.db_pool.get().expect("conn");
    for seed in 0.. {
        let t = table(&mut conn);
        let sling = ability(&mut conn, t.world_id, t.gm, "Sling", SLING, "3");
        attach(&mut conn, t.aria_actor, sling);
        let combat = fight(
            &mut conn,
            &t,
            &[(t.aria, "Aria"), (t.goblin, "Goblin")],
            t.aria,
        );
        let made = make_attack(
            &mut conn,
            SYSTEMS_DIR,
            t.player,
            false,
            &AttackRequest {
                attacker: Attacker::Token(t.aria),
                ability_id: Some(sling),
                target_token_id: Some(t.goblin),
                action_cost: Some(ActionCost::Action),
                ..Default::default()
            },
            &mut StdRng::seed_from_u64(seed),
        )
        .expect("the attack is made");
        if attack_row(&mut conn, made.attack_ids[0]).outcome != OUTCOME_HIT {
            editor(&mut conn, t.aria_actor, t.player);
            inspire(&mut conn, t.aria_actor);
            drop(conn);
            return Missed {
                state,
                t,
                made,
                combat,
            };
        }
    }
    unreachable!()
}

#[tokio::test]
async fn a_missed_attack_rerolled_into_a_hit_is_the_same_attack_judged_again() {
    let Missed {
        state,
        t,
        made,
        combat,
    } = a_miss();
    let mut conn = state.db_pool.get().expect("conn");
    let first = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(first.defence, Some(GOBLIN_AC));
    let to_hit = roll_row(&mut conn, first.to_hit_roll_id.unwrap());
    let seed = reroll_seed(&to_hit, GOBLIN_AC, true);

    // The goblin dons plate between the roll and the reroll: the reroll is
    // judged against the armour it was made against.
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(t.goblin_actor)),
    )
    .set(world_actor_system_data::ability_data.eq(json!({
        "strength": 8, "dexterity": 14, "constitution": 10,
        "intelligence": 10, "wisdom": 8, "charisma": 8, "armor_class": 99
    })))
    .execute(&mut conn)
    .expect("plate");

    let budget = actions_spent(&mut conn, combat);
    assert_eq!(budget.iter().map(|(_, spent)| spent).sum::<i32>(), 1);
    let before: Vec<i64> = [29, 30, 36, 26]
        .iter()
        .map(|code| events(&mut conn, t.world_id, *code))
        .collect();
    let rolls_before: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(t.world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();

    let new_roll = reroll(&state, t.player, t.world_id, to_hit.id, seed)
        .await
        .expect("rerolled");

    let rows: Vec<_> = world_attacks::table
        .filter(world_attacks::reroll_of.eq(first.id))
        .select(crate::combat::records::AttackRecord::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(rows.len(), 1, "one new row points back at the miss");
    let second = &rows[0];
    assert_eq!(second.to_hit_roll_id, Some(new_roll.id));
    assert_eq!(second.outcome, OUTCOME_HIT);
    assert_eq!(second.defence, Some(GOBLIN_AC), "the old defence");
    assert_eq!(second.target_token_id, first.target_token_id);
    assert_eq!(second.attacker_label, first.attacker_label);
    assert_eq!(second.target_label, first.target_label);
    assert_eq!(second.ability_name, first.ability_name);
    assert_eq!(second.distance, first.distance);
    assert_eq!(second.action_cost, first.action_cost);
    assert_eq!(
        attack_row(&mut conn, first.id).outcome,
        first.outcome,
        "the miss stays a miss"
    );

    let damage = roll_row(&mut conn, second.damage_roll_id.expect("damage rolled"));
    assert_eq!(damage.formula, "3");
    assert_eq!(damage.roll_kind.as_deref(), Some("damage"));
    let offers: i64 = world_offers::table
        .filter(world_offers::attack_id.eq(second.id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(offers, 1, "one offer, pending or applied");
    let rolls_after: i64 = world_roll_records::table
        .filter(world_roll_records::world_id.eq(t.world_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(rolls_after, rolls_before + 2, "the to-hit and its damage");

    let after: Vec<i64> = [29, 30, 36, 26]
        .iter()
        .map(|code| events(&mut conn, t.world_id, *code))
        .collect();
    for ((code, was), now) in [29, 30, 36, 26].iter().zip(&before).zip(&after) {
        assert!(now > was, "event {code} was recorded");
    }
    assert_eq!(
        actions_spent(&mut conn, combat),
        budget,
        "the budget is unchanged"
    );
    assert_eq!(inspired(&mut conn, t.aria_actor), json!(false));
}

#[tokio::test]
async fn a_reroll_that_misses_too_deals_nothing_and_still_spends() {
    let Missed { state, t, made, .. } = a_miss();
    let mut conn = state.db_pool.get().expect("conn");
    let first = attack_row(&mut conn, made.attack_ids[0]);
    let to_hit = roll_row(&mut conn, first.to_hit_roll_id.unwrap());
    let seed = reroll_seed(&to_hit, GOBLIN_AC, false);
    let offers_before = offers_in(&mut conn, t.world_id);

    reroll(&state, t.player, t.world_id, to_hit.id, seed)
        .await
        .expect("rerolled");

    let second = world_attacks::table
        .filter(world_attacks::reroll_of.eq(first.id))
        .select(crate::combat::records::AttackRecord::as_select())
        .first(&mut conn)
        .expect("the reroll's row");
    assert_ne!(second.outcome, OUTCOME_HIT);
    assert_eq!(second.damage_roll_id, None, "no damage");
    assert_eq!(offers_in(&mut conn, t.world_id), offers_before, "no offer");
    assert_eq!(inspired(&mut conn, t.aria_actor), json!(false));
}

#[tokio::test]
async fn a_hit_is_never_rerolled() {
    let mut state = state_with_real_packs();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    editor(&mut conn, t.aria_actor, t.player);
    inspire(&mut conn, t.aria_actor);
    let made = attack(&mut conn, t.player, t.aria, t.longsword, Some(t.goblin)).expect("hits");
    let first = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(first.outcome, OUTCOME_HIT);

    let refused = reroll(
        &state,
        t.player,
        t.world_id,
        first.to_hit_roll_id.unwrap(),
        0,
    )
    .await
    .unwrap_err();
    assert_eq!(refused.message, A_HIT);
    assert_eq!(
        inspired(&mut conn, t.aria_actor),
        json!(true),
        "nothing spent"
    );
    assert_eq!(attacks_in(&mut conn, t.scene_id), 1, "nothing recorded");
}

#[tokio::test]
async fn one_missed_part_of_a_multiattack_is_rerolled_on_its_own() {
    let mut state = state_with_real_packs();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    let mut conn = state.db_pool.get().expect("conn");
    // The ogre's greatclub always lands; its sling may not.
    let (t, made) = (0..)
        .find_map(|seed| {
            let t = table(&mut conn);
            let sling = ability(&mut conn, t.world_id, t.gm, "Sling", SLING, "3");
            let multi = ability(&mut conn, t.world_id, t.gm, "Multiattack", "1d20", "0");
            diesel::update(
                crate::schema::world_abilities::table
                    .filter(crate::schema::world_abilities::id.eq(multi)),
            )
            .set(
                crate::schema::world_abilities::multiattack
                    .eq(vec![Some(t.greatclub), Some(sling)]),
            )
            .execute(&mut conn)
            .expect("name its parts");
            attach(&mut conn, t.ogre_actor, multi);
            let made = make_attack(
                &mut conn,
                SYSTEMS_DIR,
                t.gm,
                false,
                &AttackRequest {
                    attacker: Attacker::Token(t.ogre),
                    ability_id: Some(multi),
                    targets: Some(vec![t.aria, t.goblin]),
                    ..Default::default()
                },
                &mut StdRng::seed_from_u64(seed),
            )
            .expect("multiattack");
            (attack_row(&mut conn, made.attack_ids[1]).outcome != OUTCOME_HIT).then_some((t, made))
        })
        .unwrap();
    inspire(&mut conn, t.ogre_actor);
    let part = attack_row(&mut conn, made.attack_ids[1]);
    assert_eq!(part.ability_name, "Sling");
    let to_hit = roll_row(&mut conn, part.to_hit_roll_id.unwrap());
    let seed = reroll_seed(&to_hit, GOBLIN_AC, true);

    reroll(&state, t.gm, t.world_id, to_hit.id, seed)
        .await
        .expect("rerolled");

    let second = world_attacks::table
        .filter(world_attacks::reroll_of.eq(part.id))
        .select(crate::combat::records::AttackRecord::as_select())
        .first(&mut conn)
        .expect("the reroll's row");
    assert_eq!(second.outcome, OUTCOME_HIT);
    assert_eq!(second.ability_name, "Sling");
    assert_eq!(second.multiattack_of, part.multiattack_of, "still a part");
    assert_eq!(
        attacks_in(&mut conn, t.scene_id),
        3,
        "only that part gets a new row"
    );
    let club = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(club.outcome, OUTCOME_HIT, "the other part stands");
    assert_eq!(second.target_token_id, Some(t.goblin));
    let damage = roll_row(&mut conn, second.damage_roll_id.expect("damage"));
    assert_eq!(damage.formula, "3", "the sling's damage, not the club's");
}

#[tokio::test]
async fn a_lairs_to_hit_has_no_maker_to_reroll_it() {
    let mut state = state_with_real_packs();
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
        "1d20-100",
        "4",
    );
    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &AttackRequest {
            attacker: Attacker::Lair(lair),
            ability_id: Some(rocks),
            target_token_id: Some(t.aria),
            action_cost: Some(ActionCost::Action),
            ..Default::default()
        },
        &mut rng(),
    )
    .expect("the lair acts");
    let first = attack_row(&mut conn, made.attack_ids[0]);
    assert_ne!(first.outcome, OUTCOME_HIT);

    let refused = reroll(&state, t.gm, t.world_id, first.to_hit_roll_id.unwrap(), 0)
        .await
        .unwrap_err();
    assert_eq!(refused.message, ONLY_MAKER);
    assert_eq!(attacks_in(&mut conn, t.scene_id), 1, "nothing recorded");
}
