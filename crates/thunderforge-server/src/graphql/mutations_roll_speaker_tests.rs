//! Spec 103 T007: a roll made as a token, against a real Postgres.

use super::*;
use crate::combat::fixtures::*;
use crate::graphql::queries::roll::world_roll_impl;
use crate::graphql::speaker::{REFUSED, SOMEONE};
use crate::graphql::types::WorldRollEntry;
use crate::schema::world_roll_records;
use crate::test_support::test_app_state;

fn d20(t: &FightTable, speaker: Option<Uuid>) -> RollDiceInput {
    RollDiceInput {
        world_id: t.world_id,
        formula: "1d20".to_string(),
        bindings: None,
        visibility: None,
        label: Some("Stealth".to_string()),
        speaker_token_id: speaker,
    }
}

fn last_roll(state: &AppState, world_id: Uuid) -> Option<Uuid> {
    world_roll_records::table
        .filter(world_roll_records::world_id.eq(world_id))
        .order(world_roll_records::created_at.desc())
        .select(world_roll_records::id)
        .first(&mut state.db_pool.get().unwrap())
        .optional()
        .unwrap()
}

async fn speaker_seen_by(
    state: &AppState,
    viewer: Uuid,
    t: &FightTable,
    roll: Uuid,
) -> (Option<Uuid>, Option<String>) {
    match world_roll_impl(state, viewer, false, t.world_id, roll).await {
        Ok(Some(WorldRollEntry::WorldRoll(roll))) => (roll.speaker_token_id, roll.speaker_label),
        other => panic!("the roll is seen whole: {other:?}"),
    }
}

#[tokio::test]
async fn a_roll_made_as_a_token_carries_its_name() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    roll_dice_impl(&state, t.player, d20(&t, Some(t.aria)), &mut rand::rng())
        .await
        .expect("Aria is the player's");
    let roll = last_roll(&state, t.world_id).unwrap();

    let aria = (Some(t.aria), Some("Aria".to_string()));
    assert_eq!(speaker_seen_by(&state, t.stranger, &t, roll).await, aria);
    assert_eq!(speaker_seen_by(&state, t.gm, &t, roll).await, aria);
}

#[tokio::test]
async fn a_hidden_speaker_rolls_as_someone_to_players() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    roll_dice_impl(&state, t.gm, d20(&t, Some(t.ogre)), &mut rand::rng())
        .await
        .expect("the GM rolls as any token");
    let roll = last_roll(&state, t.world_id).unwrap();

    assert_eq!(
        speaker_seen_by(&state, t.gm, &t, roll).await.1.as_deref(),
        Some(OGRE_NAME)
    );
    assert_eq!(
        speaker_seen_by(&state, t.player, &t, roll).await,
        (Some(t.ogre), Some(SOMEONE.to_string()))
    );
}

#[tokio::test]
async fn a_token_the_roller_may_not_speak_as_is_refused_and_not_rolled() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    let err = roll_dice_impl(&state, t.player, d20(&t, Some(t.goblin)), &mut rand::rng())
        .await
        .expect_err("the goblin is the GM's");
    assert_eq!(err.message, REFUSED);
    assert_eq!(last_roll(&state, t.world_id), None);
}
