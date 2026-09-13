//! Spec 015 T042: a scene is a moderated entity type, on the same terms as the
//! other four — filed against, disabled for everyone, its owner told, restored
//! by a counter-notice, and counted as a strike.
//!
//! Every notice here goes through `submit_takedown_notice_impl`, not a row
//! written by hand: the claim under test is that the programme accepts a scene,
//! and a hand-written row would prove only that `effective_status` reads one.

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::scene_visibility::{asset_scene_visible, visible_scene_ids};
use crate::graphql::mutations_moderation::{
    SubmitCounterNoticeInput, SubmitTakedownNoticeInput, submit_counter_notice_impl,
    submit_takedown_notice_impl,
};
use crate::graphql::queries::moderation::repeat_infringer_flags_impl;
use crate::graphql::queries::scene::{scene_impl, scenes_impl};
use crate::graphql::types::{ModerationActionType, ModerationEntityType};
use crate::moderation::{action_type, effective_status, strikes_of};
use crate::schema::content_moderation_actions;
use crate::state::AppState;
use crate::test_support::{
    insert_test_scene_named, insert_test_user, insert_test_world, test_app_state,
};

fn notice_against(scene_id: Uuid) -> SubmitTakedownNoticeInput {
    SubmitTakedownNoticeInput {
        entity_type: ModerationEntityType::Scene,
        entity_id: scene_id,
        claimant_name: "Scene Test Claimant".into(),
        claimant_contact: "claimant@example.test".into(),
        copyrighted_work_description: "A published battle map".into(),
        infringing_material_location: "A scene in a ThunderForge world".into(),
        good_faith_statement: true,
        accuracy_statement: true,
        signature: "Scene Test Claimant".into(),
    }
}

/// A world with two scenes: one to take down, one to prove FR-010 by.
struct World {
    owner: Uuid,
    world: Uuid,
    contested: Uuid,
    untouched: Uuid,
}

fn a_world(state: &AppState) -> World {
    let mut conn = state.db_pool.get().expect("connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let contested = insert_test_scene_named(&mut conn, world, owner, "Contested Map");
    let untouched = insert_test_scene_named(&mut conn, world, owner, "Ordinary Cellar");
    World {
        owner,
        world,
        contested,
        untouched,
    }
}

async fn scene_status(state: &AppState, scene_id: Uuid) -> Option<String> {
    effective_status(state, "scene", scene_id)
        .await
        .expect("status readable")
}

/// Moves every forwarded counter-notice on `case_id` a day into the past —
/// the statutory waiting period, elapsed. The same simulation
/// `forwarded_counter_notice_past_due_auto_restores` and the counter-notice
/// e2e use.
fn elapse_waiting_period(conn: &mut PgConnection, case_id: Uuid) {
    diesel::update(
        content_moderation_actions::table
            .filter(content_moderation_actions::case_id.eq(case_id))
            .filter(
                content_moderation_actions::action_type.eq(action_type::COUNTER_NOTICE_FORWARDED),
            ),
    )
    .set(
        content_moderation_actions::restoration_due_at
            .eq(Some(Utc::now() - chrono::Duration::days(1))),
    )
    .execute(conn)
    .expect("waiting period moved");
}

/// FR-004, FR-010 and the enforcement contract: a notice against a scene
/// disables that scene for every reader including its owner, and nothing else
/// in the world.
#[tokio::test]
async fn a_notice_against_a_scene_disables_it_for_everyone_and_only_it() {
    let state = test_app_state();
    let w = a_world(&state);

    let case = submit_takedown_notice_impl(&state, notice_against(w.contested))
        .await
        .expect("a notice can be filed against a scene");
    assert_eq!(case.entity_type, ModerationEntityType::Scene);
    assert_eq!(case.world_id, w.world, "the case records the scene's world");
    assert_eq!(case.current_status, ModerationActionType::ContentDisabled);

    // The world's owner is its DM: the list and the detail both lose it.
    let listed = scenes_impl(&state, w.owner, false, w.world)
        .await
        .expect("scenes load");
    let ids: Vec<Uuid> = listed.iter().map(|s| s.scene_id).collect();
    assert!(
        !ids.contains(&w.contested),
        "a taken-down scene is excluded from the list, owner or not"
    );
    assert!(
        ids.contains(&w.untouched),
        "FR-010: its neighbour is untouched"
    );
    assert!(
        scene_impl(&state, w.owner, false, w.contested)
            .await
            .expect("scene query answers")
            .is_none(),
        "the detail read is refused to the owner too"
    );
    // A site admin is not an exception either.
    assert!(
        scene_impl(&state, Uuid::now_v7(), true, w.contested)
            .await
            .expect("scene query answers")
            .is_none(),
    );

    // The sync plan and the canvas byte route ask `scene_visibility`, and a DM
    // short-circuit there would leave the background fetchable by id.
    let mut conn = state.db_pool.get().expect("connection");
    let plan_scenes = visible_scene_ids(&mut conn, true, w.world).expect("visible scenes");
    assert!(!plan_scenes.contains(&w.contested));
    assert!(plan_scenes.contains(&w.untouched));
    assert!(
        !asset_scene_visible(&mut conn, true, Some(w.contested)).expect("asset visibility"),
        "an image attached to a taken-down scene is withheld without its own notice"
    );
    assert!(asset_scene_visible(&mut conn, true, Some(w.untouched)).expect("asset visibility"));

    // FR-005 / spec 039 FR-028: the owner is told, and it is a strike.
    assert_eq!(
        strikes_of(&mut conn, w.owner, 365).expect("strikes").len(),
        1
    );
    drop(conn);
    let told: Vec<String> = crate::notices::for_account(&state, w.owner, 20)
        .await
        .expect("notices readable")
        .into_iter()
        .map(|n| n.kind)
        .collect();
    assert!(
        told.iter()
            .any(|k| k == crate::notices::kind::STRIKE_RECORDED),
        "the scene's owner is told of the takedown: {told:?}"
    );
}

/// FR-007: a counter-notice forwards, keeps the scene dark through the waiting
/// period, and restores it on the first read afterwards — returning it to every
/// read path it left, and taking the strike away.
#[tokio::test]
async fn a_counter_notice_restores_a_scene_after_the_waiting_period() {
    let state = test_app_state();
    let w = a_world(&state);

    let case = submit_takedown_notice_impl(&state, notice_against(w.contested))
        .await
        .expect("notice accepted");
    let forwarded = submit_counter_notice_impl(
        &state,
        w.owner,
        false,
        SubmitCounterNoticeInput {
            case_id: case.case_id,
            removed_material_description: "My own hand-drawn map".into(),
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "owner@example.test".into(),
            signature: "The Owner".into(),
        },
    )
    .await
    .expect("the scene's owner may counter-notice");
    assert_eq!(
        forwarded.current_status,
        ModerationActionType::CounterNoticeForwarded
    );
    assert_eq!(
        scene_status(&state, w.contested).await.as_deref(),
        Some(action_type::CONTENT_DISABLED),
        "still dark while the claimant has time to act"
    );

    {
        let mut conn = state.db_pool.get().expect("connection");
        elapse_waiting_period(&mut conn, case.case_id);
        // Asked through the sync path first, on purpose: the restoration must
        // materialise whichever path reads first.
        assert!(
            visible_scene_ids(&mut conn, true, w.world)
                .expect("visible scenes")
                .contains(&w.contested),
            "the sync plan sees it again"
        );
    }

    assert_eq!(scene_status(&state, w.contested).await, None);
    assert!(
        scene_impl(&state, w.owner, false, w.contested)
            .await
            .expect("scene query answers")
            .is_some(),
        "the owner reads it again, with nothing rebuilt"
    );
    let mut conn = state.db_pool.get().expect("connection");
    let restored: i64 = content_moderation_actions::table
        .filter(content_moderation_actions::case_id.eq(case.case_id))
        .filter(content_moderation_actions::action_type.eq(action_type::CONTENT_RESTORED))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(restored, 1, "restored once, as a durable event, not twice");
    assert!(
        strikes_of(&mut conn, w.owner, 365)
            .expect("strikes")
            .is_empty(),
        "a restored case is not a strike"
    );
}

/// FR-008 / FR-009: scene takedowns count toward the repeat-infringer record
/// like any other — three upheld cases flag the account.
#[tokio::test]
async fn scene_takedowns_count_toward_the_repeat_infringer_record() {
    let state = test_app_state();
    let (owner, scenes) = {
        let mut conn = state.db_pool.get().expect("connection");
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        let scenes: Vec<Uuid> = (0..crate::moderation::repeat_infringer_threshold())
            .map(|n| insert_test_scene_named(&mut conn, world, owner, &format!("Map {n}")))
            .collect();
        (owner, scenes)
    };

    for scene in &scenes {
        submit_takedown_notice_impl(&state, notice_against(*scene))
            .await
            .expect("notice accepted");
    }

    let mut conn = state.db_pool.get().expect("connection");
    let strikes = strikes_of(&mut conn, owner, 365).expect("strikes");
    assert_eq!(strikes.len(), scenes.len());
    assert!(strikes.iter().all(|s| s.entity_type == "scene"));
    drop(conn);

    let flagged = repeat_infringer_flags_impl(&state)
        .await
        .expect("flags readable");
    assert!(
        flagged.contains(&owner),
        "an account at the threshold on scene takedowns alone is flagged"
    );
}
