//! Spec 103 T004: who may speak as which token (research R4), and what a
//! player reads of a hidden token's name (R5).

use super::*;
use crate::combat::fixtures::*;
use crate::test_support::{
    insert_test_scene_named, insert_test_user, insert_test_world, test_app_state,
};

fn refused(result: GraphQLResult<(Uuid, String)>) -> bool {
    match result {
        Ok(_) => false,
        Err(e) => {
            e.message == REFUSED
                && e.extensions
                    .as_ref()
                    .and_then(|x| x.get("code"))
                    .is_some_and(|code| code == &async_graphql::Value::from("FORBIDDEN"))
        }
    }
}

#[test]
fn the_gm_may_speak_as_any_token_of_the_world() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    for token in [t.aria, t.goblin, t.ogre] {
        let (id, _) = resolve_speaker(&mut conn, t.world_id, Some(t.scene_id), t.gm, false, token)
            .expect("the GM speaks as anyone");
        assert_eq!(id, token);
    }
    let (_, name) =
        resolve_speaker(&mut conn, t.world_id, None, t.gm, false, t.ogre).expect("no scene given");
    assert_eq!(name, OGRE_NAME, "the stored label is the real name");
}

#[test]
fn a_player_speaks_only_as_a_token_they_may_move() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let (id, name) = resolve_speaker(
        &mut conn,
        t.world_id,
        Some(t.scene_id),
        t.player,
        false,
        t.aria,
    )
    .expect("Aria is the player's");
    assert_eq!((id, name.as_str()), (t.aria, "Aria"));

    assert!(refused(resolve_speaker(
        &mut conn,
        t.world_id,
        Some(t.scene_id),
        t.player,
        false,
        t.goblin
    )));
    assert!(refused(resolve_speaker(
        &mut conn,
        t.world_id,
        Some(t.scene_id),
        t.stranger,
        false,
        t.aria
    )));
}

#[test]
fn a_token_of_another_world_or_off_the_scene_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);

    let other_scene = insert_test_scene_named(&mut conn, t.world_id, t.gm, "The Other Room");
    assert!(refused(resolve_speaker(
        &mut conn,
        t.world_id,
        Some(other_scene),
        t.player,
        false,
        t.aria
    )));
    assert!(refused(resolve_speaker(
        &mut conn,
        t.world_id,
        Some(other_scene),
        t.gm,
        false,
        t.aria
    )));

    let elsewhere_gm = insert_test_user(&mut conn);
    let elsewhere = insert_test_world(&mut conn, elsewhere_gm);
    assert!(refused(resolve_speaker(
        &mut conn,
        elsewhere,
        None,
        elsewhere_gm,
        false,
        t.aria
    )));
}

#[test]
fn a_missing_token_is_refused_like_a_forbidden_one() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    assert!(refused(resolve_speaker(
        &mut conn,
        t.world_id,
        None,
        t.gm,
        false,
        Uuid::now_v7()
    )));
}

#[test]
fn a_player_reads_someone_where_the_name_is_hidden() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let t = table(&mut conn);
    let ogre = Some(OGRE_NAME.to_string());

    // The ogre is a hidden NPC until the GM shows it (owner decision 2026-09-15).
    assert_eq!(
        speaker_label_for(&mut conn, true, Some(t.ogre), ogre.clone()).unwrap(),
        ogre
    );
    assert_eq!(
        speaker_label_for(&mut conn, false, Some(t.ogre), ogre.clone()).unwrap(),
        Some(SOMEONE.to_string())
    );
    show_npcs(&mut conn, &t);
    assert_eq!(
        speaker_label_for(&mut conn, false, Some(t.ogre), ogre.clone()).unwrap(),
        ogre
    );

    // A token since deleted keeps its line, nameless to players.
    assert_eq!(
        speaker_label_for(&mut conn, false, None, ogre.clone()).unwrap(),
        Some(SOMEONE.to_string())
    );
    // Said as the author: no speaker at all.
    assert_eq!(
        speaker_label_for(&mut conn, false, None, None).unwrap(),
        None
    );
}
