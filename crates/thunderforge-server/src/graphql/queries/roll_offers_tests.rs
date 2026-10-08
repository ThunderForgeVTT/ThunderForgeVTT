//! Spec 084 T038: a roll's maker is told what they could reroll it by, and
//! until when; nobody else is offered anything.

use diesel::prelude::*;
use uuid::Uuid;

use super::roll::world_rolls_impl;
use crate::graphql::mutations_roll_check::roll_check_impl;
use crate::graphql::mutations_roll_check::tests::{
    StepRng, state_with_real_packs, world_with_actor,
};
use crate::graphql::types::{WorldRoll, WorldRollEntry};
use crate::schema::world_actor_system_data;
use crate::state::AppState;
use crate::test_support::{insert_test_user, insert_test_world_member};
use thunderforge_canvas_core::roll_facets::Advantage;

fn give_inspiration(state: &AppState, actor_id: Uuid, inspired: bool) {
    let mut conn = state.db_pool.get().unwrap();
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(actor_id)),
    )
    .set(world_actor_system_data::trait_data.eq(Some(
        serde_json::json!({ "level": 5, "inspiration": inspired }),
    )))
    .execute(&mut conn)
    .unwrap();
}

async fn the_roll(state: &AppState, viewer: Uuid, world_id: Uuid) -> WorldRoll {
    let mut feed = world_rolls_impl(state, viewer, false, world_id, None, None)
        .await
        .unwrap();
    assert_eq!(feed.len(), 1, "one roll was made");
    match feed.pop().unwrap() {
        WorldRollEntry::WorldRoll(roll) => *roll,
        WorldRollEntry::MaskedRoll(_) => panic!("an everyone roll is shown whole"),
    }
}

fn offered(roll: &WorldRoll) -> Vec<String> {
    roll.reroll_offers.iter().map(|o| o.label.clone()).collect()
}

#[tokio::test]
async fn the_maker_is_offered_heroic_inspiration_and_the_table_is_not() {
    let state = state_with_real_packs();
    let (owner, world_id, actor_id) = world_with_actor(&state, "dnd5e");
    let player = {
        let mut conn = state.db_pool.get().unwrap();
        let player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player, "Player");
        player
    };
    give_inspiration(&state, actor_id, true);
    roll_check_impl(
        &state,
        owner,
        false,
        world_id,
        actor_id,
        "stealth".to_string(),
        Advantage::Normal,
        &mut StepRng(11),
    )
    .await
    .unwrap();

    let mine = the_roll(&state, owner, world_id).await;
    assert_eq!(offered(&mine), vec!["Heroic Inspiration"]);
    let until = chrono::DateTime::parse_from_rfc3339(mine.reroll_until.as_deref().unwrap())
        .unwrap()
        .with_timezone(&chrono::Utc);
    let made = chrono::DateTime::parse_from_rfc3339(&mine.created_at)
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert_eq!(until - made, chrono::Duration::minutes(2));

    let theirs = the_roll(&state, player, world_id).await;
    assert!(offered(&theirs).is_empty());
    assert_eq!(theirs.reroll_until, mine.reroll_until);

    give_inspiration(&state, actor_id, false);
    let spent = the_roll(&state, owner, world_id).await;
    assert!(offered(&spent).is_empty(), "nothing to pay with");
}
