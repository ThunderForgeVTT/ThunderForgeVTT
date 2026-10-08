//! Spec 083 FR-002, FR-003: a whole roll says which numbers the server put in
//! for its placeholders, and why each extra die in a chain was rolled. A
//! masked roll gains neither.

use diesel::prelude::*;
use rand::SeedableRng;
use uuid::Uuid;

use super::*;
use crate::graphql::mutations_roll::{PlaceholderBindingInput, RollDiceInput, roll_dice_impl};
use crate::graphql::mutations_roll_check::roll_check_impl;
use crate::graphql::mutations_roll_check::tests::{
    StepRng, state_with_real_packs, world_with_actor,
};
use crate::graphql::queries::roll::world_roll_impl;
use crate::graphql::types::{DieStep, GraphQLDieOutcome};
use crate::schema::world_roll_records;
use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member};
use thunderforge_dice::{ChainStep, DieOutcome, DieSides};

fn row(bindings: Option<serde_json::Value>) -> RollRecord {
    RollRecord {
        id: Uuid::now_v7(),
        world_id: Uuid::now_v7(),
        triggered_by: Uuid::now_v7(),
        formula: "1d20 + MODIFIER".to_string(),
        bindings,
        detail: serde_json::json!({}),
        result_kind: "total".to_string(),
        result_value: 16.0,
        created_at: chrono::Utc::now(),
        outcome: None,
        visibility: "everyone".to_string(),
        label: None,
        revealed_at: None,
        revealed_by: None,
        actor_id: None,
        roll_kind: None,
        check_id: None,
        facets: Vec::new(),
        reroll_of: None,
        reroll_spent: None,
    }
}

fn bindings_of(value: Option<serde_json::Value>) -> Vec<(String, f64)> {
    WorldRoll::from_row(row(value), "Ayla".to_string(), None, RollLinks::default())
        .bindings
        .into_iter()
        .map(|b| (b.placeholder, b.value))
        .collect()
}

#[test]
fn a_rows_bindings_become_placeholder_value_pairs() {
    assert_eq!(
        bindings_of(Some(serde_json::json!({ "MODIFIER": 3 }))),
        vec![("MODIFIER".to_string(), 3.0)]
    );
}

#[test]
fn no_bindings_or_unreadable_ones_read_as_none() {
    assert!(bindings_of(None).is_empty());
    assert!(bindings_of(Some(serde_json::Value::Null)).is_empty());
    assert!(bindings_of(Some(serde_json::json!([1, 2]))).is_empty());
    assert!(bindings_of(Some(serde_json::json!({ "X": "three" }))).is_empty());
}

#[test]
fn bindings_come_back_sorted_by_placeholder() {
    assert_eq!(
        bindings_of(Some(serde_json::json!({ "STAT": 2, "BONUS": -1 }))),
        vec![("BONUS".to_string(), -1.0), ("STAT".to_string(), 2.0)]
    );
}

#[test]
fn a_dies_steps_are_exposed_in_order() {
    let die = DieOutcome {
        sides: DieSides::Numeric(6),
        rolls: vec![1, 6, 3],
        steps: vec![ChainStep::Reroll, ChainStep::Explode],
        kept: true,
        final_value: 3,
    };
    assert_eq!(
        GraphQLDieOutcome::from(&die).steps,
        vec![DieStep::Reroll, DieStep::Explode]
    );
}

#[tokio::test]
async fn a_check_rolled_from_the_sheet_reads_back_with_its_bindings() {
    let state = state_with_real_packs();
    let (owner_id, world_id, actor_id) = world_with_actor(&state, "dnd5e");
    roll_check_impl(
        &state,
        owner_id,
        false,
        world_id,
        actor_id,
        "dexterity".to_string(),
        thunderforge_canvas_core::roll_facets::Advantage::Normal,
        &mut StepRng(11),
    )
    .await
    .expect("the owner may roll their check");

    let roll_id: Uuid = {
        let mut conn = state.db_pool.get().unwrap();
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .select(world_roll_records::id)
            .first(&mut conn)
            .unwrap()
    };
    let entry = world_roll_impl(&state, owner_id, false, world_id, roll_id)
        .await
        .unwrap()
        .expect("the roller sees their own roll");
    let WorldRollEntry::WorldRoll(roll) = entry else {
        panic!("the roller gets the whole roll");
    };
    // Dexterity 16 is +3, and the server says so.
    assert_eq!(
        roll.bindings
            .iter()
            .map(|b| (b.placeholder.as_str(), b.value))
            .collect::<Vec<_>>(),
        vec![("MODIFIER", 3.0)]
    );
}

#[tokio::test]
async fn a_masked_roll_has_no_bindings_to_ask_for() {
    let state = crate::test_support::test_app_state();
    let (gm, roller, other, world) = {
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let roller = insert_test_user(&mut conn);
        let other = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, gm);
        insert_test_world_member(&mut conn, world, roller, "Player");
        insert_test_world_member(&mut conn, world, other, "Player");
        (gm, roller, other, world)
    };
    let _ = gm;
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    roll_dice_impl(
        &state,
        roller,
        RollDiceInput {
            world_id: world,
            formula: "1d20 + MODIFIER".to_string(),
            bindings: Some(vec![PlaceholderBindingInput {
                name: "MODIFIER".to_string(),
                value: 3.0,
            }]),
            visibility: Some(RollVisibility::GmEyes),
            label: Some("Stealth".to_string()),
        },
        &mut rng,
    )
    .await
    .expect("the roll is allowed");
    let roll_id: Uuid = {
        let mut conn = state.db_pool.get().unwrap();
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(world))
            .select(world_roll_records::id)
            .first(&mut conn)
            .unwrap()
    };
    let entry = world_roll_impl(&state, other, false, world, roll_id)
        .await
        .unwrap()
        .expect("masked, not hidden");
    assert!(
        matches!(entry, WorldRollEntry::MaskedRoll(_)),
        "another player gets the masked shape"
    );

    // And the schema has no field by which a masked roll could carry them.
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish();
    let response = schema
        .execute(format!(
            "{{ worldRoll(worldId: \"{world}\", rollId: \"{roll_id}\") {{ ... on MaskedRoll {{ bindings {{ placeholder }} }} }} }}"
        ))
        .await;
    assert!(
        response
            .errors
            .iter()
            .any(|e| e.message.contains("bindings") && e.message.contains("MaskedRoll")),
        "{:?}",
        response.errors
    );
}
