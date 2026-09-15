//! Spec 046 T089: the economy of a round — C9 (an overspend is recorded and
//! shown, never refused), a budget comes back at its owner's turn and nobody
//! else's, a reaction spent between turns stays spent until then, movement is
//! counted against speed from the steps a move takes, and a multiattack is one
//! action.
//!
//! The offline path's spend is `mutations_reconcile_tests`
//! (`a_replayed_move_spends_movement_by_the_drag_rule`), where `apply_one` can
//! be reached.

use super::*;
use crate::combat::attack::{AttackRequest, FightRefusal, make_attack, preview_attack};
use crate::combat::fixtures::*;
use crate::combat::records::{FLAG_NO_REACH_DECLARED, FLAG_OVERSPENT};
use crate::graphql::mutations_combat::{
    AddCombatantInput, StartCombatInput, add_combatant_impl, advance_turn_impl, combat_world,
    load_combat, start_combat_impl,
};
use crate::schema::{world_abilities, world_combats};
use crate::state::AppState;
use crate::test_support::test_app_state;
use thunderforge_canvas_core::grid::Cell;

/// The test state, reading the real packs.
fn state() -> AppState {
    let mut state = test_app_state();
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    state
}

fn combatant_of(conn: &mut PgConnection, combat_id: Uuid, token: Uuid) -> Combatant {
    world_combatants::table
        .filter(world_combatants::combat_id.eq(combat_id))
        .filter(world_combatants::token_id.eq(token))
        .select(Combatant::as_select())
        .first(conn)
        .expect("combatant")
}

/// A token's budget as the tracker resolves it.
fn budget(conn: &mut PgConnection, t: &FightTable, combat_id: Uuid, token: Uuid) -> TurnBudget {
    let combatant = combatant_of(conn, combat_id, token);
    budgets_for(
        conn,
        SYSTEMS_DIR,
        t.world_id,
        std::slice::from_ref(&combatant),
    )
    .expect("budgets")
    .remove(&combatant.id)
    .expect("5e declares a budget")
}

fn player_attack(
    conn: &mut PgConnection,
    t: &FightTable,
    ability: Uuid,
    cost: Option<ActionCost>,
) -> Result<crate::combat::attack::MadeAttack, FightRefusal> {
    make_attack(
        conn,
        SYSTEMS_DIR,
        t.player,
        false,
        &AttackRequest {
            attacker_token_id: t.aria,
            ability_id: Some(ability),
            target_token_id: Some(t.goblin),
            action_cost: cost,
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

// ---------------------------------------------------------------------------
// A budget with each combatant
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_combatant_added_to_the_tracker_has_a_budget_with_nothing_spent() {
    let state = state();
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
    let combat = add_combatant_impl(
        &state,
        t.gm,
        false,
        AddCombatantInput {
            combat_id: combat.id,
            label: "Aria".into(),
            actor_id: Some(t.aria_actor),
            token_id: Some(t.aria),
            initiative: Some(15),
            tiebreak: None,
            is_npc: Some(false),
        },
    )
    .await
    .expect("added");

    let row = world_combatant_budgets::table
        .filter(world_combatant_budgets::combatant_id.eq(combat.combatants[0].id))
        .select(BudgetRow::as_select())
        .first(&mut conn)
        .expect("a row is created with the combatant");
    assert_eq!(row, BudgetRow::fresh(row.combatant_id));

    let shown = combat.combatants[0].budget.expect("shown on the tracker");
    assert_eq!(shown.action, BudgetLine::new(1.0, 0.0));
    assert_eq!(shown.bonus_action, BudgetLine::new(1.0, 0.0));
    assert_eq!(shown.reaction, BudgetLine::new(1.0, 0.0));
    assert_eq!(
        shown.movement,
        BudgetLine::new(30.0, 0.0),
        "movement is the creature's walk speed (5e's default when a sheet says none)"
    );
    assert_eq!(shown.legendary, None);
}

#[test]
fn a_speed_on_the_sheet_is_the_movement_allowed() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(t.aria_actor)),
    )
    .set(world_actor_system_data::ability_data.eq(serde_json::json!({ "speed_walk": 25 })))
    .execute(&mut conn)
    .expect("a dwarf");
    let combat = fight(&mut conn, &t, &[(t.aria, "Aria")], t.aria);
    assert_eq!(
        budget(&mut conn, &t, combat, t.aria).movement,
        BudgetLine::new(25.0, 0.0)
    );
}

#[test]
fn a_world_whose_system_declares_no_budget_is_given_none() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(worlds::table.filter(worlds::id.eq(t.world_id)))
        .set(worlds::game_system_id.eq(Some("blades_in_the_dark")))
        .execute(&mut conn)
        .expect("a system with no turn economy");
    let combat = fight(&mut conn, &t, &[(t.aria, "Aria")], t.aria);
    let combatant = combatant_of(&mut conn, combat, t.aria);
    assert!(
        budgets_for(&mut conn, SYSTEMS_DIR, t.world_id, &[combatant])
            .expect("budgets")
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// C9: recorded, shown, never refused
// ---------------------------------------------------------------------------

#[test]
fn c9_a_second_action_is_made_recorded_and_flagged_overspent_never_refused() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.goblin, "Goblin")],
        t.aria,
    );
    let request = AttackRequest {
        attacker_token_id: t.aria,
        ability_id: Some(t.longsword),
        target_token_id: Some(t.goblin),
        ..Default::default()
    };

    let warned =
        preview_attack(&mut conn, SYSTEMS_DIR, t.player, false, &request).expect("preview");
    assert!(!warned.flags.contains(&FLAG_OVERSPENT.to_string()));

    let first = player_attack(&mut conn, &t, t.longsword, None).expect("her action");
    assert!(!flags_of(&mut conn, first.attack_ids[0]).contains(&FLAG_OVERSPENT.to_string()));
    assert_eq!(
        budget(&mut conn, &t, combat, t.aria).action,
        BudgetLine::new(1.0, 1.0)
    );

    let warned =
        preview_attack(&mut conn, SYSTEMS_DIR, t.player, false, &request).expect("preview");
    assert!(
        warned.flags.contains(&FLAG_OVERSPENT.to_string()),
        "the warning before rolling says so"
    );

    let second = player_attack(&mut conn, &t, t.longsword, None)
        .expect("C9: an attack past the action is made, not refused");
    assert_eq!(
        flags_of(&mut conn, second.attack_ids[0]),
        vec![
            FLAG_NO_REACH_DECLARED.to_string(),
            FLAG_OVERSPENT.to_string()
        ],
        "the table is shown the overspend beside what reach already said"
    );
    let action = budget(&mut conn, &t, combat, t.aria).action;
    assert_eq!(action.spent, 2.0, "two of one is recorded");
    assert_eq!(action.remaining, -1.0, "and shown as a debt");
    assert!(action.is_overspent());
}

#[test]
fn a_bonus_action_and_a_reaction_spend_their_own_lines_and_a_free_attack_spends_nothing() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.goblin, "Goblin")],
        t.aria,
    );

    player_attack(&mut conn, &t, t.longsword, Some(ActionCost::BonusAction)).expect("bonus");
    player_attack(&mut conn, &t, t.longsword, Some(ActionCost::Free)).expect("free");
    // The ability's own cost, when the request names none.
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(t.whiff)))
        .set(world_abilities::action_cost.eq("reaction"))
        .execute(&mut conn)
        .expect("a reaction");
    player_attack(&mut conn, &t, t.whiff, None).expect("reaction");

    let spent = budget(&mut conn, &t, combat, t.aria);
    assert_eq!(spent.action.spent, 0.0);
    assert_eq!(spent.bonus_action.spent, 1.0);
    assert_eq!(spent.reaction.spent, 1.0);
}

#[test]
fn a_multiattack_spends_one_action_for_all_its_parts() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(&mut conn, &t, &[(t.ogre, "Ogre"), (t.aria, "Aria")], t.ogre);
    let fist = ability(&mut conn, t.world_id, t.gm, "Fist", "1d20+100", "2");
    let multi = ability(&mut conn, t.world_id, t.gm, "Multiattack", "1d20", "0");
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(multi)))
        .set(world_abilities::multiattack.eq(vec![Some(t.greatclub), Some(fist), Some(fist)]))
        .execute(&mut conn)
        .expect("three parts");

    let made = make_attack(
        &mut conn,
        SYSTEMS_DIR,
        t.gm,
        false,
        &AttackRequest {
            attacker_token_id: t.ogre,
            ability_id: Some(multi),
            target_token_id: Some(t.aria),
            ..Default::default()
        },
        &mut rng(),
    )
    .expect("multiattack");
    assert_eq!(made.attack_ids.len(), 3);
    assert_eq!(
        budget(&mut conn, &t, combat, t.ogre).action,
        BudgetLine::new(1.0, 1.0),
        "FR-044: one action, not three"
    );
    for id in made.attack_ids {
        assert!(!flags_of(&mut conn, id).contains(&FLAG_OVERSPENT.to_string()));
    }
}

#[test]
fn an_attack_outside_a_running_combat_spends_nothing_and_is_never_flagged() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    for _ in 0..3 {
        let made = player_attack(&mut conn, &t, t.longsword, None).expect("exploring");
        assert!(!flags_of(&mut conn, made.attack_ids[0]).contains(&FLAG_OVERSPENT.to_string()));
    }
    assert_eq!(
        world_combatant_budgets::table
            .inner_join(world_combatants::table)
            .filter(world_combatants::token_id.eq(t.aria))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("count"),
        0
    );
}

// ---------------------------------------------------------------------------
// It comes back at its owner's turn, and nobody else's
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_turn_resets_only_the_new_active_combatant_and_a_reaction_waits_for_its_owners_turn() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    // Aria 20, Goblin 19, Ogre 18: the goblin's turn.
    let combat = fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.goblin, "Goblin"), (t.ogre, "Ogre")],
        t.goblin,
    );
    let goblin = combatant_of(&mut conn, combat, t.goblin).id;
    let ogre = combatant_of(&mut conn, combat, t.ogre).id;

    // On the goblin's turn: Aria takes a reaction, the goblin its action, and
    // the ogre (somehow) has moved.
    player_attack(&mut conn, &t, t.longsword, Some(ActionCost::Reaction))
        .expect("FR-041: a reaction between turns");
    spend(&mut conn, goblin, Spend::Action, t.gm).expect("goblin acts");
    spend(&mut conn, ogre, Spend::Movement(15.0), t.gm).expect("ogre moved");

    advance_turn_impl(&state, t.gm, false, combat)
        .await
        .expect("to the ogre");
    assert_eq!(
        budget(&mut conn, &t, combat, t.ogre).movement.spent,
        0.0,
        "the ogre's turn begins fresh"
    );
    assert_eq!(
        budget(&mut conn, &t, combat, t.goblin).action.spent,
        1.0,
        "the goblin's turn ending gives it nothing back"
    );
    assert_eq!(
        budget(&mut conn, &t, combat, t.aria).reaction.spent,
        1.0,
        "Aria's reaction stays spent through somebody else's turn"
    );

    let shown = advance_turn_impl(&state, t.gm, false, combat)
        .await
        .expect("to Aria");
    assert_eq!(
        budget(&mut conn, &t, combat, t.aria).reaction.spent,
        0.0,
        "and returns at the start of her own"
    );
    let aria_row = shown
        .combatants
        .iter()
        .find(|c| c.token_id == Some(t.aria))
        .expect("Aria on the tracker");
    assert_eq!(
        aria_row.budget.expect("shown").reaction,
        BudgetLine::new(1.0, 0.0)
    );
    assert_eq!(budget(&mut conn, &t, combat, t.goblin).action.spent, 1.0);
}

#[test]
fn every_seat_sees_a_hidden_combatants_budget_under_unknown() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.ogre, OGRE_NAME)],
        t.ogre,
    );
    diesel::update(tokens::table.filter(tokens::token_id.eq(t.ogre)))
        .set(tokens::name_visible_to_players.eq(false))
        .execute(&mut conn)
        .expect("hide the ogre's name");
    let ogre = combatant_of(&mut conn, combat, t.ogre).id;
    spend(&mut conn, ogre, Spend::Action, t.gm).expect("ogre acts");

    let row = combat_world(&mut conn, combat).expect("combat");
    let seen = load_combat(&mut conn, SYSTEMS_DIR, row, t.player, false).expect("player's view");
    let hidden = seen
        .combatants
        .iter()
        .find(|c| c.id == ogre)
        .expect("the ogre is on the tracker");
    assert_eq!(hidden.label, "Unknown");
    assert_eq!(
        hidden.budget.expect("its budget is shown").action,
        BudgetLine::new(1.0, 1.0)
    );
}

// ---------------------------------------------------------------------------
// Movement, from the steps a move takes
// ---------------------------------------------------------------------------

/// The test scene: 5-unit squares on a 100-square map, so a vertex sits on
/// every multiple of 5 and a cell's centre on every 2.5 past one.
fn scene_grid() -> GridSpec {
    crate::combat::reach::scene_grid("square", 5, 100, 100)
}

fn place_at(conn: &mut PgConnection, token: Uuid, x: f64, y: f64) {
    diesel::update(tokens::table.filter(tokens::token_id.eq(token)))
        .set((tokens::x.eq(x), tokens::y.eq(y)))
        .execute(conn)
        .expect("place");
}

#[test]
fn movement_along_a_route_is_counted_against_speed_and_overspent_movement_is_a_debt() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(&mut conn, &t, &[(t.aria, "Aria")], t.aria);
    place_at(&mut conn, t.aria, 2.5, 2.5);

    // East, east, a diagonal, east: four squares, twenty feet (5-5-5).
    let route = [(2.5, 2.5), (7.5, 2.5), (12.5, 2.5), (17.5, 7.5)];
    let spent = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.aria,
        (2.5, 2.5),
        Some(&route),
        (22.5, 7.5),
        t.player,
    )
    .expect("spent");
    assert_eq!(spent, Some(20.0));
    assert_eq!(
        budget(&mut conn, &t, combat, t.aria).movement,
        BudgetLine::new(30.0, 20.0),
        "she moves 20 ft: 10 ft left"
    );

    // Three more squares, dragged: fifteen feet past ten left.
    spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.aria,
        (22.5, 7.5),
        None,
        (37.5, 7.5),
        t.player,
    )
    .expect("C9: a move past her speed is recorded, not refused");
    let movement = budget(&mut conn, &t, combat, t.aria).movement;
    assert_eq!(movement.spent, 35.0);
    assert_eq!(movement.remaining, -5.0);
}

#[test]
fn a_drag_costs_the_footprints_displacement_and_a_large_creature_counts_its_block() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(t.ogre_actor)),
    )
    .set(world_actor_system_data::trait_data.eq(serde_json::json!({
        "class": "monster", "level": 5, "size": "large"
    })))
    .execute(&mut conn)
    .expect("a Large ogre");
    let combat = fight(&mut conn, &t, &[(t.ogre, "Ogre")], t.ogre);
    // A Large token sits on a vertex.
    place_at(&mut conn, t.ogre, 5.0, 5.0);

    // Dragged one square east: its four squares overlap two of the old ones,
    // so the distance between the footprints is 0; it moved five feet.
    let dragged = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.ogre,
        (5.0, 5.0),
        None,
        (10.0, 5.0),
        t.gm,
    )
    .expect("a Game Master's drag spends the ogre's movement");
    assert_eq!(dragged, Some(5.0));

    // A route, vertex to vertex: east then a diagonal, two squares.
    let route = [(10.0, 5.0), (15.0, 5.0)];
    let routed = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.ogre,
        (10.0, 5.0),
        Some(&route),
        (20.0, 10.0),
        t.gm,
    )
    .expect("routed");
    assert_eq!(routed, Some(10.0));
    assert_eq!(budget(&mut conn, &t, combat, t.ogre).movement.spent, 15.0);
}

#[test]
fn a_token_that_is_not_a_combatant_in_a_running_combat_spends_nothing() {
    let state = state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let no_fight = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.aria,
        (0.0, 0.0),
        None,
        (50.0, 0.0),
        t.player,
    )
    .expect("exploring");
    assert_eq!(no_fight, None);

    let combat = fight(&mut conn, &t, &[(t.ogre, "Ogre")], t.ogre);
    let bystander = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.aria,
        (0.0, 0.0),
        None,
        (50.0, 0.0),
        t.player,
    )
    .expect("not in the fight");
    assert_eq!(bystander, None);
    assert_eq!(budget(&mut conn, &t, combat, t.ogre).movement.spent, 0.0);

    diesel::update(world_combats::table.filter(world_combats::id.eq(combat)))
        .set(world_combats::ended_at.eq(Some(chrono::Utc::now().naive_utc())))
        .execute(&mut conn)
        .expect("end the fight");
    let ended = spend_for_move(
        &mut conn,
        SYSTEMS_DIR,
        t.scene_id,
        t.ogre,
        (0.0, 0.0),
        None,
        (50.0, 0.0),
        t.gm,
    )
    .expect("after the fight");
    assert_eq!(ended, None, "an ended combat is not running");
}

#[test]
fn a_step_counts_the_cells_a_footprint_moves_on_every_kind_of_grid() {
    let square = scene_grid();
    let one = Footprint::new(1.0);
    let large = Footprint::new(2.0);
    let at = |x: f32, y: f32| Vec2::new(x, y);

    assert_eq!(step_cells(&square, one, at(2.5, 2.5), at(7.5, 2.5)), 1.0);
    assert_eq!(
        step_cells(&square, one, at(2.5, 2.5), at(17.5, 12.5)),
        3.0,
        "5-5-5"
    );
    assert_eq!(step_cells(&square, large, at(5.0, 5.0), at(10.0, 5.0)), 1.0);
    assert_eq!(step_cells(&square, large, at(5.0, 5.0), at(5.0, -5.0)), 2.0);
    assert_eq!(
        square.footprint_distance(at(5.0, 5.0), large, at(10.0, 5.0), large),
        0.0,
        "which is why a move is not the distance between footprints"
    );

    let hex = GridSpec {
        kind: GridKind::HexPointyTop,
        size: 10.0,
        origin: Vec2::ZERO,
    };
    let from = hex.cell_center(Cell::new(0, 0));
    let to = hex.cell_center(Cell::new(3, -1));
    assert_eq!(step_cells(&hex, one, from, to), 3.0);

    let gridless = GridSpec {
        kind: GridKind::Gridless,
        size: 10.0,
        origin: Vec2::ZERO,
    };
    assert_eq!(
        step_cells(&gridless, one, at(0.0, 0.0), at(30.0, 40.0)),
        5.0
    );
}

#[test]
fn a_move_costs_its_steps_in_the_systems_units() {
    let feet = GridUnits::new(5.0, "ft");
    let square = scene_grid();
    let one = Footprint::new(1.0);
    let at = |x: f32, y: f32| Vec2::new(x, y);

    // Standing still, or a route that goes nowhere, costs nothing.
    assert_eq!(
        move_cost(&square, &feet, one, "walk", at(2.5, 2.5), &[], at(2.5, 2.5)),
        0.0
    );
    // There and back is ten feet, not the zero between start and end.
    assert_eq!(
        move_cost(
            &square,
            &feet,
            one,
            "walk",
            at(2.5, 2.5),
            &[at(7.5, 2.5)],
            at(2.5, 2.5)
        ),
        10.0
    );
    // A route that does not start where the token stands is counted from
    // where it stands.
    assert_eq!(
        move_cost(
            &square,
            &feet,
            one,
            "walk",
            at(2.5, 2.5),
            &[at(12.5, 2.5)],
            at(17.5, 2.5)
        ),
        15.0
    );
    let gridless = GridSpec {
        kind: GridKind::Gridless,
        size: 10.0,
        origin: Vec2::ZERO,
    };
    assert_eq!(
        move_cost(
            &gridless,
            &feet,
            one,
            "walk",
            at(0.0, 0.0),
            &[at(30.0, 0.0)],
            at(30.0, 25.0)
        ),
        27.5
    );
}

#[test]
fn a_resolved_budget_may_go_negative() {
    let declared = SystemTurnBudget {
        action: Some(1),
        bonus_action: Some(1),
        reaction: None,
        movement: None,
    };
    let mut row = BudgetRow::fresh(Uuid::nil());
    row.action_spent = 3;
    row.reaction_spent = 1;
    row.movement_spent = 10.0;
    let budget = resolve(&declared, 30.0, &row);
    assert_eq!(budget.action, BudgetLine::new(1.0, 3.0));
    assert_eq!(budget.action.remaining, -2.0);
    assert_eq!(
        budget.reaction,
        BudgetLine::new(0.0, 1.0),
        "undeclared affords none"
    );
    assert_eq!(
        budget.movement.allowed, 0.0,
        "no movement declared, none allowed"
    );
}
