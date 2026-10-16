//! Spec 103 T006: a chat line said as a token, against a real Postgres.

use super::*;
use crate::combat::fixtures::*;
use crate::graphql::speaker::{REFUSED, SOMEONE};
use crate::test_support::test_app_state;

fn line(t: &FightTable, body: &str, speaker: Option<Uuid>) -> SendChatMessageInput {
    SendChatMessageInput {
        world_id: t.world_id,
        scene_id: Some(t.scene_id),
        body: body.to_string(),
        gm_only: None,
        speaker_token_id: speaker,
    }
}

fn said(messages: &[GraphQLChatMessage], body: &str) -> (Option<Uuid>, Option<String>) {
    let m = messages
        .iter()
        .find(|m| m.body == body)
        .expect("the line is listed");
    (m.speaker_token_id, m.speaker_label.clone())
}

#[tokio::test]
async fn a_line_said_as_a_token_is_stored_with_its_name() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    let sent = send_chat_message_impl(&state, t.player, false, line(&t, "Onward!", Some(t.aria)))
        .await
        .expect("Aria is the player's");
    assert_eq!(sent.speaker_token_id, Some(t.aria));
    assert_eq!(sent.speaker_label.as_deref(), Some("Aria"));

    let plain = send_chat_message_impl(&state, t.player, false, line(&t, "brb", None))
        .await
        .expect("said as the author");
    assert_eq!((plain.speaker_token_id, plain.speaker_label), (None, None));

    let seen = world_chat_messages_impl(&state, t.stranger, false, t.world_id, None)
        .await
        .unwrap();
    assert_eq!(
        said(&seen, "Onward!"),
        (Some(t.aria), Some("Aria".to_string()))
    );
}

#[tokio::test]
async fn a_hidden_speaker_is_someone_to_players() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    send_chat_message_impl(&state, t.gm, false, line(&t, "You dare?", Some(t.ogre)))
        .await
        .expect("the GM speaks as any token");

    let gm = world_chat_messages_impl(&state, t.gm, false, t.world_id, None)
        .await
        .unwrap();
    assert_eq!(said(&gm, "You dare?").1.as_deref(), Some(OGRE_NAME));
    let player = world_chat_messages_impl(&state, t.player, false, t.world_id, None)
        .await
        .unwrap();
    assert_eq!(
        said(&player, "You dare?"),
        (Some(t.ogre), Some(SOMEONE.to_string()))
    );
}

#[tokio::test]
async fn a_token_the_sender_may_not_speak_as_is_refused_and_not_sent() {
    let state = test_app_state();
    let t = table(&mut state.db_pool.get().unwrap());

    let err = send_chat_message_impl(
        &state,
        t.player,
        false,
        line(&t, "I am a goblin", Some(t.goblin)),
    )
    .await
    .expect_err("the goblin is the GM's");
    assert_eq!(err.message, REFUSED);

    let seen = world_chat_messages_impl(&state, t.gm, false, t.world_id, None)
        .await
        .unwrap();
    assert!(seen.iter().all(|m| m.body != "I am a goblin"));
}
