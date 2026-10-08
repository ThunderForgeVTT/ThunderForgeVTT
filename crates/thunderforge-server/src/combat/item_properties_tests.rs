//! Spec 084 T061: the item properties a pack declares (research R6), and
//! `setItemAttack` storing only those.

use super::*;
use crate::combat::attack::ActionCost;
use crate::combat::fixtures::*;
use crate::graphql::mutations_attacks::{AttackFieldsOwner, set_attack_fields_impl};
use crate::graphql::types::types_attacks::AttackFieldsInput;
use crate::schema::world_items;
use crate::test_support::test_app_state;
use diesel::prelude::*;

fn packs() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .to_str()
        .unwrap()
        .to_string()
}

fn with(properties: Option<Vec<&str>>) -> AttackFieldsInput {
    AttackFieldsInput {
        reach: Some(5.0),
        range_normal: None,
        range_long: None,
        needs_line_of_sight: true,
        action_cost: ActionCost::Action,
        legendary_cost: 1,
        multiattack: Vec::new(),
        properties: properties.map(|ids| ids.into_iter().map(str::to_string).collect()),
    }
}

#[test]
fn five_e_declares_the_nine_weapon_properties() {
    let ids: Vec<String> = item_properties_for_system(&packs(), "dnd5e")
        .into_iter()
        .map(|property| property.id)
        .collect();
    assert_eq!(
        ids,
        [
            "ammunition",
            "finesse",
            "heavy",
            "light",
            "loading",
            "reach",
            "thrown",
            "two_handed",
            "versatile"
        ]
    );
    let two_handed = item_properties_for_system(&packs(), "dnd5e")
        .into_iter()
        .find(|property| property.id == "two_handed")
        .unwrap();
    assert_eq!(two_handed.label, "Two-Handed");
}

#[test]
fn a_pack_without_the_key_declares_none() {
    assert!(item_properties_for_system(&packs(), "roll_for_shoes").is_empty());
    assert!(item_properties_for_system(&packs(), "no_such_system").is_empty());
}

#[tokio::test]
async fn set_item_attack_stores_declared_properties_and_refuses_the_rest() {
    let mut state = test_app_state();
    state.directories.systems_dir = packs();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let item = crate::test_support::insert_test_item(&mut conn, t.world_id, t.gm);
    drop(conn);
    let stored = |state: &crate::AppState| {
        let mut conn = state.db_pool.get().expect("conn");
        world_items::table
            .filter(world_items::id.eq(item))
            .select(world_items::properties)
            .first::<Vec<Option<String>>>(&mut conn)
            .expect("item")
            .into_iter()
            .flatten()
            .collect::<Vec<String>>()
    };

    let owner = AttackFieldsOwner::Item(item);
    set_attack_fields_impl(&state, t.gm, false, owner, with(Some(vec!["two_handed"])))
        .await
        .expect("a greatsword is two-handed");
    assert_eq!(stored(&state), ["two_handed"]);

    // Absent leaves them as they were.
    set_attack_fields_impl(&state, t.gm, false, owner, with(None))
        .await
        .expect("unchanged");
    assert_eq!(stored(&state), ["two_handed"]);

    let err = set_attack_fields_impl(&state, t.gm, false, owner, with(Some(vec!["glowing"])))
        .await
        .unwrap_err();
    assert_eq!(err.message, "This system has no item property \"glowing\".");
    assert_eq!(stored(&state), ["two_handed"], "nothing written");

    // An ability is not an item, and has no properties.
    let err = set_attack_fields_impl(
        &state,
        t.gm,
        false,
        AttackFieldsOwner::Ability(t.longsword),
        with(Some(vec!["two_handed"])),
    )
    .await
    .unwrap_err();
    assert_eq!(err.message, ONLY_AN_ITEM);
}

#[test]
fn the_schema_offers_the_properties_to_the_editor() {
    let sdl = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl();
    assert!(sdl.contains("systemItemProperties(worldId: UUID!): [RollFacet!]!"));
    assert!(sdl.contains("properties: [String!]\n"), "AttackFieldsInput");
    assert!(sdl.contains("properties: [String!]!"), "Item");
}
