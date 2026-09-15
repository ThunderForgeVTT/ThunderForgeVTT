//! Spec 046 T099: legendary actions and the lair (US6, FR-050–FR-053).
//!
//! Spent between other creatures' turns and refilled at the owner's own;
//! flagged, never refused, on its own turn and past zero; a lair at 20 losing
//! ties, with no budget, acting only for a Game Master, and named to every
//! seat.

use super::*;
use crate::combat::attack::{ActionCost, AttackRequest, Attacker, FightRefusal, make_attack};
use crate::combat::budget::{BudgetLine, budgets_for};
use crate::combat::fixtures::*;
use crate::combat::records::{FLAG_LEGENDARY_ON_OWN_TURN, FLAG_OVERSPENT, KIND_LAIR};
use crate::graphql::mutations_combat::{
    AddCombatantInput, CombatantKind, GraphQLCombat, StartCombatInput, add_combatant_impl,
    advance_turn_impl, combat_world, load_combat, start_combat_impl,
};
use crate::graphql::mutations_combat_lair::add_lair_combatant_impl;
use crate::graphql::types::types_attacks::{Sights, build_attacks};
use crate::models::Combatant;
use crate::schema::{world_abilities, world_attacks, world_combatants};
use crate::state::AppState;
use crate::test_support::test_app_state;
use serde_json::json;

fn state() -> AppState {
    let mut state = test_app_state();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    state
}

/// The ogre's sheet says it has `n` legendary actions a round.
fn make_legendary(conn: &mut PgConnection, t: &FightTable, n: serde_json::Value) {
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(t.ogre_actor)),
    )
    .set(world_actor_system_data::trait_data.eq(json!({ "legendary_actions": n })))
    .execute(conn)
    .expect("a legendary ogre");
}

/// The ogre's tail swipe: a legendary action costing `cost`.
fn tail(conn: &mut PgConnection, t: &FightTable, cost: i32) -> Uuid {
    let id = ability(conn, t.world_id, t.gm, "Tail", "1d20+100", "3");
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(id)))
        .set((
            world_abilities::action_cost.eq("legendary"),
            world_abilities::legendary_cost.eq(cost),
        ))
        .execute(conn)
        .expect("legendary");
    attach(conn, t.ogre_actor, id);
    id
}

async fn start(state: &AppState, t: &FightTable) -> GraphQLCombat {
    start_combat_impl(
        state,
        t.gm,
        false,
        StartCombatInput {
            world_id: t.world_id,
            scene_id: Some(t.scene_id),
        },
    )
    .await
    .expect("started")
}

async fn add(
    state: &AppState,
    t: &FightTable,
    combat_id: Uuid,
    token: Uuid,
    actor: Uuid,
    label: &str,
    initiative: i32,
) -> GraphQLCombat {
    add_combatant_impl(
        state,
        t.gm,
        false,
        AddCombatantInput {
            combat_id,
            label: label.into(),
            actor_id: Some(actor),
            token_id: Some(token),
            initiative: Some(initiative),
            tiebreak: None,
            is_npc: None,
        },
    )
    .await
    .expect("added")
}

/// Aria (18), the ogre (15), the goblin (10): the ogre's turn sits between
/// two others.
async fn legendary_fight(state: &AppState, t: &FightTable) -> Uuid {
    let combat = start(state, t).await;
    add(state, t, combat.id, t.aria, t.aria_actor, "Aria", 18).await;
    add(state, t, combat.id, t.ogre, t.ogre_actor, "Ogre", 15).await;
    add(state, t, combat.id, t.goblin, t.goblin_actor, "Goblin", 10).await;
    combat.id
}

fn legendary_of(conn: &mut PgConnection, t: &FightTable, combat_id: Uuid) -> Option<BudgetLine> {
    let ogre = world_combatants::table
        .filter(world_combatants::combat_id.eq(combat_id))
        .filter(world_combatants::token_id.eq(t.ogre))
        .select(Combatant::as_select())
        .first(conn)
        .expect("the ogre");
    budgets_for(conn, SYSTEMS_DIR, t.world_id, std::slice::from_ref(&ogre))
        .expect("budgets")
        .remove(&ogre.id)
        .expect("5e declares a budget")
        .legendary
}

fn swipe(
    conn: &mut PgConnection,
    t: &FightTable,
    ability_id: Uuid,
) -> Result<crate::combat::attack::MadeAttack, FightRefusal> {
    make_attack(
        conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &AttackRequest {
            attacker: Attacker::Token(t.ogre),
            ability_id: Some(ability_id),
            target_token_id: Some(t.aria),
            ..Default::default()
        },
        &mut rng(),
    )
}

fn flags_of(conn: &mut PgConnection, attack: Uuid) -> Vec<String> {
    attack_row(conn, attack)
        .flags
        .into_iter()
        .flatten()
        .collect()
}

async fn advance(state: &AppState, t: &FightTable, combat_id: Uuid) -> GraphQLCombat {
    advance_turn_impl(state, t.gm, false, combat_id)
        .await
        .expect("advanced")
}

// ---------------------------------------------------------------------------
// FR-050: read from the sheet when added
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_legendary_creature_joins_with_its_pool_full_and_a_creature_without_one_has_no_line() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    make_legendary(&mut conn, &t, json!(3));

    let combat_id = legendary_fight(&state, &t).await;
    assert_eq!(
        legendary_of(&mut conn, &t, combat_id),
        Some(BudgetLine::new(3.0, 0.0)),
        "three a round, none spent"
    );
    let row = combat_world(&mut conn, combat_id).expect("combat");
    let combat = load_combat(&mut conn, SYSTEMS_DIR, row, t.player, false).expect("loaded");
    for row in &combat.combatants {
        let legendary = row.budget.as_ref().expect("a budget").legendary;
        if row.token_id == Some(t.ogre) {
            assert_eq!(
                legendary,
                Some(BudgetLine::new(3.0, 0.0)),
                "players see it too"
            );
        } else {
            assert_eq!(legendary, None, "{} has no legendary actions", row.label);
        }
    }
}

#[tokio::test]
async fn zero_legendary_actions_is_no_pool() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    make_legendary(&mut conn, &t, json!(0));
    let combat_id = legendary_fight(&state, &t).await;
    assert_eq!(legendary_of(&mut conn, &t, combat_id), None);
}

// ---------------------------------------------------------------------------
// FR-051, FR-052, SC-006: spent across other turns, refilled at its own
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sc006_three_legendary_actions_across_other_turns_then_three_again_at_its_own() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    make_legendary(&mut conn, &t, json!(3));
    let swipe_id = tail(&mut conn, &t, 1);
    let second_goblin = place(
        &mut conn,
        t.scene_id,
        Some(t.goblin_actor),
        Some(t.gm),
        false,
        Some(json!({ "current_hp": GOBLIN_HP, "max_hp": GOBLIN_HP, "temporary_hp": 0 })),
        "Goblin 2",
        (100.0, 100.0),
    );
    // The ogre (20), then three others: Aria (18), the goblin (10), a second
    // goblin (5).
    let combat = start(&state, &t).await;
    add(&state, &t, combat.id, t.ogre, t.ogre_actor, "Ogre", 20).await;
    add(&state, &t, combat.id, t.aria, t.aria_actor, "Aria", 18).await;
    add(
        &state,
        &t,
        combat.id,
        t.goblin,
        t.goblin_actor,
        "Goblin",
        10,
    )
    .await;
    add(
        &state,
        &t,
        combat.id,
        second_goblin,
        t.goblin_actor,
        "Goblin 2",
        5,
    )
    .await;
    let ogre_turn = advance(&state, &t, combat.id).await;
    let ogre_id = ogre_turn.active_combatant_id;
    assert_eq!(
        legendary_of(&mut conn, &t, combat.id),
        Some(BudgetLine::new(3.0, 0.0))
    );

    for remaining in [2.0, 1.0, 0.0] {
        let turn = advance(&state, &t, combat.id).await;
        assert_ne!(turn.active_combatant_id, ogre_id, "somebody else's turn");
        let made = swipe(&mut conn, &t, swipe_id).expect("a legendary action");
        let flags = flags_of(&mut conn, made.attack_ids[0]);
        assert!(!flags.contains(&FLAG_OVERSPENT.to_string()), "{flags:?}");
        assert!(!flags.contains(&FLAG_LEGENDARY_ON_OWN_TURN.to_string()));
        assert_eq!(
            attack_row(&mut conn, made.attack_ids[0]).action_cost,
            "legendary"
        );
        assert_eq!(
            legendary_of(&mut conn, &t, combat.id).map(|l| l.remaining),
            Some(remaining),
            "FR-050: the tracker shows how many remain"
        );
    }

    let back = advance(&state, &t, combat.id).await;
    assert_eq!(
        back.active_combatant_id, ogre_id,
        "round 2, the ogre's turn"
    );
    assert_eq!(
        legendary_of(&mut conn, &t, combat.id),
        Some(BudgetLine::new(3.0, 0.0)),
        "FR-052, SC-006: three again at the start of its own turn"
    );
}

#[tokio::test]
async fn a_legendary_action_on_its_own_turn_and_past_zero_is_flagged_never_refused() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    make_legendary(&mut conn, &t, json!(1));
    let swipe_id = tail(&mut conn, &t, 2);
    let combat_id = legendary_fight(&state, &t).await;
    advance(&state, &t, combat_id).await; // Aria
    advance(&state, &t, combat_id).await; // the ogre

    let request = AttackRequest {
        attacker: Attacker::Token(t.ogre),
        ability_id: Some(swipe_id),
        target_token_id: Some(t.aria),
        ..Default::default()
    };
    let warned =
        crate::combat::attack::preview_attack(&mut conn, SYSTEMS_DIR, t.gm, false, &request)
            .expect("preview");
    assert!(
        warned
            .flags
            .contains(&FLAG_LEGENDARY_ON_OWN_TURN.to_string())
    );
    assert!(
        warned.flags.contains(&FLAG_OVERSPENT.to_string()),
        "a cost of 2 from a pool of 1 is warned"
    );

    let made = swipe(&mut conn, &t, swipe_id).expect("made, not refused");
    let flags = flags_of(&mut conn, made.attack_ids[0]);
    assert!(
        flags.contains(&FLAG_LEGENDARY_ON_OWN_TURN.to_string()),
        "{flags:?}"
    );
    assert!(flags.contains(&FLAG_OVERSPENT.to_string()), "{flags:?}");
    let line = legendary_of(&mut conn, &t, combat_id).expect("pool");
    assert_eq!(
        line.remaining, -1.0,
        "legendary_cost 2 from 1: a debt, shown"
    );
    assert!(line.is_overspent());
    // Its action was not spent by a legendary action.
    let ogre = world_combatants::table
        .filter(world_combatants::token_id.eq(t.ogre))
        .select(Combatant::as_select())
        .first(&mut conn)
        .expect("ogre");
    let budget = budgets_for(
        &mut conn,
        SYSTEMS_DIR,
        t.world_id,
        std::slice::from_ref(&ogre),
    )
    .expect("budgets")
    .remove(&ogre.id)
    .expect("budget");
    assert_eq!(budget.action.spent, 0.0);
}

#[tokio::test]
async fn a_legendary_action_by_a_creature_with_none_is_flagged_overspent_and_spends_nothing() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let swipe_id = tail(&mut conn, &t, 1);
    let combat_id = legendary_fight(&state, &t).await;
    advance(&state, &t, combat_id).await; // Aria
    let made = swipe(&mut conn, &t, swipe_id).expect("made");
    assert!(flags_of(&mut conn, made.attack_ids[0]).contains(&FLAG_OVERSPENT.to_string()));
    assert_eq!(legendary_of(&mut conn, &t, combat_id), None);
}

#[test]
fn an_actor_added_twice_without_a_token_is_ambiguous_and_neither_row_is_spent() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat_id = fight(&mut conn, &t, &[(t.aria, "Aria")], t.aria);
    let mut rows = Vec::new();
    for label in ["Ogre A", "Ogre B"] {
        let id = Uuid::now_v7();
        diesel::insert_into(world_combatants::table)
            .values(&crate::models::NewCombatant {
                id,
                combat_id,
                actor_id: Some(t.ogre_actor),
                token_id: None,
                label: label.into(),
                initiative: 5,
                tiebreak: 0,
                is_npc: true,
            })
            .execute(&mut conn)
            .expect("token-less ogre");
        create_budget_for(
            &mut conn,
            SYSTEMS_DIR,
            t.world_id,
            id,
            None,
            Some(t.ogre_actor),
            t.gm,
        )
        .expect("budget");
        rows.push(id);
    }
    assert!(
        crate::combat::budget::combatant_of_token(&mut conn, t.scene_id, t.ogre)
            .expect("lookup")
            .is_none(),
        "two rows could be the ogre's token: neither is taken"
    );

    // With one, the token acts as it (the turn check's rule).
    diesel::delete(world_combatants::table.filter(world_combatants::id.eq(rows[1])))
        .execute(&mut conn)
        .expect("remove one");
    let found = crate::combat::budget::combatant_of_token(&mut conn, t.scene_id, t.ogre)
        .expect("lookup")
        .map(|(_, c)| c.id);
    assert_eq!(found, Some(rows[0]));
}

// ---------------------------------------------------------------------------
// FR-053: a lair at 20, losing ties
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_lair_sits_at_twenty_loses_ties_and_has_no_budget() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = start(&state, &t).await;
    add(
        &state,
        &t,
        combat.id,
        t.goblin,
        t.goblin_actor,
        "Goblin",
        19,
    )
    .await;
    add(&state, &t, combat.id, t.aria, t.aria_actor, "Aria", 20).await;
    let combat = add_lair_combatant_impl(&state, t.gm, false, combat.id, " The Crypt ".into())
        .await
        .expect("a lair");

    let order: Vec<&str> = combat.combatants.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        order,
        ["Aria", "The Crypt", "Goblin"],
        "a tie at 20 goes to the creature"
    );
    let lair = &combat.combatants[1];
    assert_eq!(lair.kind, CombatantKind::Lair);
    assert_eq!((lair.initiative, lair.tiebreak), (20, -1));
    assert_eq!((lair.token_id, lair.actor_id), (None, None));
    assert!(lair.budget.is_none(), "no Move 0/0 ft beside a lair");
    assert_eq!(combat.combatants[0].kind, CombatantKind::Creature);

    // Its turn comes round like anyone's, and still has no budget.
    advance(&state, &t, combat.id).await;
    let on_lair = advance(&state, &t, combat.id).await;
    assert_eq!(on_lair.active_combatant_id, Some(lair.id));
    assert!(on_lair.combatants[1].budget.is_none());

    // A player may not add one.
    let refused = add_lair_combatant_impl(&state, t.player, false, combat.id, "Mine".into()).await;
    assert!(refused.is_err());
    // Nor may a lair be given a token.
    let bad = diesel::update(world_combatants::table.filter(world_combatants::id.eq(lair.id)))
        .set(world_combatants::token_id.eq(Some(t.goblin)))
        .execute(&mut conn);
    assert!(bad.is_err(), "the table refuses a lair with a creature");
    assert_eq!(
        world_combatants::table
            .filter(world_combatants::id.eq(lair.id))
            .select(world_combatants::kind)
            .first::<String>(&mut conn)
            .expect("kind"),
        KIND_LAIR
    );
}

#[tokio::test]
async fn a_lairs_action_is_a_game_masters_resolves_like_an_attack_and_names_the_lair_to_players() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = start(&state, &t).await;
    add(&state, &t, combat.id, t.aria, t.aria_actor, "Aria", 12).await;
    let combat = add_lair_combatant_impl(&state, t.gm, false, combat.id, "The Crypt".into())
        .await
        .expect("lair");
    let lair = combat
        .combatants
        .iter()
        .find(|c| c.kind == CombatantKind::Lair)
        .expect("lair")
        .id;
    // Aria's turn: a lair action needs no turn, and nothing measures it.
    advance(&state, &t, combat.id).await;
    advance(&state, &t, combat.id).await;
    let falling_rocks = ability(
        &mut conn,
        t.world_id,
        t.gm,
        "Falling Rocks",
        "1d20+100",
        "4",
    );
    let request = |target| AttackRequest {
        attacker: Attacker::Lair(lair),
        ability_id: Some(falling_rocks),
        target_token_id: target,
        action_cost: Some(ActionCost::Action),
        ..Default::default()
    };

    // C2: a player is refused, before anything is written.
    let before = world_attacks::table
        .count()
        .get_result::<i64>(&mut conn)
        .expect("count");
    let refused = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.player,
        false,
        &request(Some(t.aria)),
        &mut rng(),
    );
    assert_eq!(refused.unwrap_err(), FightRefusal::NotControlled);
    assert!(
        crate::combat::attack::preview_attack(
            &mut conn,
            SYSTEMS_DIR,
            t.player,
            false,
            &request(Some(t.aria))
        )
        .is_err()
    );
    assert_eq!(
        world_attacks::table
            .count()
            .get_result::<i64>(&mut conn)
            .expect("count"),
        before
    );

    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &request(Some(t.aria)),
        &mut rng(),
    )
    .expect("the Game Master acts for the lair");
    let row = attack_row(&mut conn, made.attack_ids[0]);
    assert_eq!(row.attacker_token_id, None);
    assert_eq!(row.attacker_kind, KIND_LAIR);
    assert_eq!(row.attacker_label, "The Crypt");
    assert_eq!(row.outcome, "hit", "resolved against Aria's armour class");
    assert_eq!(row.distance, None, "no body to measure from");
    assert!(
        row.flags.is_empty(),
        "no reach, range, sight or budget flags"
    );
    assert_eq!(
        made.offer_ids.len(),
        1,
        "Aria's player is offered the damage"
    );

    // Every seat is told the lair's name and what it used.
    let mut sights = Sights::new(SYSTEMS_DIR, t.stranger, false);
    let seen = build_attacks(&mut conn, &mut sights, vec![row]).expect("read");
    assert_eq!(seen[0].attacker.label, "The Crypt");
    assert_eq!(seen[0].attacker.token_id, None);
    assert_eq!(seen[0].ability_name.as_deref(), Some("Falling Rocks"));

    // A lair that is not in a running fight is not there.
    let gone = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &AttackRequest {
            attacker: Attacker::Lair(Uuid::now_v7()),
            ability_id: Some(falling_rocks),
            ..Default::default()
        },
        &mut rng(),
    );
    assert!(matches!(gone, Err(FightRefusal::NotFound(_))));
}
