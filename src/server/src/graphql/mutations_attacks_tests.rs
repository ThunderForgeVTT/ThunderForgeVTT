//! Spec 046 T056: the GraphQL surface of attacks — who may set auto-apply, what
//! an ability is as an attack, and `makeAttack` answering per viewer.

use super::*;
use crate::combat::attack::ActionCost;
use crate::combat::fixtures::*;
use crate::test_support::test_app_state;

fn fields(reach: Option<f64>, normal: Option<f64>, long: Option<f64>) -> AttackFieldsInput {
    AttackFieldsInput {
        reach,
        range_normal: normal,
        range_long: long,
        needs_line_of_sight: false,
        action_cost: ActionCost::BonusAction,
        legendary_cost: 2,
        multiattack: Vec::new(),
    }
}

#[tokio::test]
async fn only_a_game_master_sets_auto_apply_for_the_world_or_an_encounter() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let combat = fight(&mut conn, &t, &[(t.aria, "Aria")], t.aria);
    drop(conn);

    let input = UpdateWorldAutoApplyNpcDamageInput {
        world_id: t.world_id,
        enabled: true,
    };
    assert!(
        update_world_auto_apply_npc_damage_impl(&state, t.player, false, input.clone())
            .await
            .is_err()
    );
    let world = update_world_auto_apply_npc_damage_impl(&state, t.gm, false, input)
        .await
        .expect("the Game Master");
    assert!(world.auto_apply_npc_damage);

    assert!(
        set_combat_auto_apply_impl(&state, t.player, false, combat, Some(false))
            .await
            .is_err()
    );
    let encounter = set_combat_auto_apply_impl(&state, t.gm, false, combat, Some(false))
        .await
        .expect("the Game Master");
    assert_eq!(encounter.auto_apply, Some(false));
    let cleared = set_combat_auto_apply_impl(&state, t.gm, false, combat, None)
        .await
        .expect("back to the world's");
    assert_eq!(cleared.auto_apply, None);
}

#[tokio::test]
async fn an_ability_or_item_is_given_its_attack_fields_and_nonsense_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let item = crate::test_support::insert_test_item(&mut conn, t.world_id, t.gm);
    drop(conn);

    let owner = AttackFieldsOwner::Ability(t.longsword);
    set_attack_fields_impl(&state, t.gm, false, owner, fields(Some(5.0), None, None))
        .await
        .expect("reach 5");
    let err = set_attack_fields_impl(
        &state,
        t.gm,
        false,
        owner,
        fields(None, Some(80.0), Some(20.0)),
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("Long range"), "{}", err.message);
    let err = set_attack_fields_impl(&state, t.gm, false, owner, fields(Some(-5.0), None, None))
        .await
        .unwrap_err();
    assert!(err.message.contains("negative"), "{}", err.message);
    let mut own = fields(None, None, None);
    own.multiattack = vec![t.longsword];
    assert!(
        set_attack_fields_impl(&state, t.gm, false, owner, own)
            .await
            .is_err(),
        "a multiattack naming itself"
    );
    // A player without Editor on it may not.
    assert!(
        set_attack_fields_impl(
            &state,
            t.player,
            false,
            owner,
            fields(Some(10.0), None, None)
        )
        .await
        .is_err()
    );

    set_attack_fields_impl(
        &state,
        t.gm,
        false,
        AttackFieldsOwner::Item(item),
        fields(None, Some(80.0), Some(320.0)),
    )
    .await
    .expect("a shortbow's range");

    let mut conn = state.db_pool.get().expect("conn");
    let (reach, sight, cost, legendary): (Option<f64>, bool, String, i32) = world_abilities::table
        .filter(world_abilities::id.eq(t.longsword))
        .select((
            world_abilities::reach,
            world_abilities::needs_line_of_sight,
            world_abilities::action_cost,
            world_abilities::legendary_cost,
        ))
        .first(&mut conn)
        .expect("row");
    assert_eq!(
        (reach, sight, cost.as_str(), legendary),
        (Some(5.0), false, "bonus_action", 2)
    );
    let range: (Option<f64>, Option<f64>) = world_items::table
        .filter(world_items::id.eq(item))
        .select((world_items::range_normal, world_items::range_long))
        .first(&mut conn)
        .expect("row");
    assert_eq!(range, (Some(80.0), Some(320.0)));
}

#[tokio::test]
async fn make_attack_answers_the_caller_as_they_may_see_it() {
    let mut state = test_app_state();
    // The bundled packs, where 5e declares its armour class.
    state.directories.systems_dir = SYSTEMS_DIR.to_string();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    drop(conn);
    let attacks = make_attack_impl(
        &state,
        t.player,
        false,
        AttackInput {
            attacker_token_id: Some(t.aria),
            lair_combatant_id: None,
            ability_id: Some(t.longsword),
            item_id: None,
            target_token_id: Some(t.goblin),
            targets: None,
            action_cost: None,
            bindings: None,
        },
        rng(),
    )
    .await
    .expect("attack");
    assert_eq!(attacks.len(), 1);
    let attack = &attacks[0];
    assert_eq!(attack.attacker.token_id, Some(t.aria));
    assert_eq!(attack.target.as_ref().unwrap().label, "Goblin");
    assert_eq!(attack.defence, Some(GOBLIN_AC));
    let offer = attack.offer.as_ref().expect("a hit offers");
    assert_eq!(offer.amount, 5);
    assert!(
        !offer.may_resolve,
        "the goblin is not Aria's player's to take"
    );

    // Off turn, the refusal is the contract's sentence.
    let mut conn = state.db_pool.get().expect("conn");
    fight(
        &mut conn,
        &t,
        &[(t.aria, "Aria"), (t.goblin, "Goblin")],
        t.goblin,
    );
    drop(conn);
    let err = make_attack_impl(
        &state,
        t.player,
        false,
        AttackInput {
            attacker_token_id: Some(t.aria),
            lair_combatant_id: None,
            ability_id: Some(t.longsword),
            item_id: None,
            target_token_id: Some(t.goblin),
            targets: None,
            action_cost: None,
            bindings: None,
        },
        rng(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.message, "It is Goblin's turn");
}
