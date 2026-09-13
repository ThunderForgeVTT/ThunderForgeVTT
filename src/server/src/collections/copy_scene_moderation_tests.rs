//! Spec 015 T042 inside a collection: a scene taken down is withheld from the
//! shared read and from the copy, returns when a counter-notice restores it,
//! and a copy somebody already took goes dark with it.
//!
//! Notices are filed through `submit_takedown_notice_impl`, as a claimant's
//! would be, because the gap T042 closed was that no notice *could* name a
//! scene — a hand-written moderation row would have passed before the fix.

use super::*;
use crate::graphql::mutations_collection_shares::shared_collection_impl;
use crate::graphql::mutations_moderation::{
    SubmitCounterNoticeInput, SubmitTakedownNoticeInput, submit_counter_notice_impl,
    submit_takedown_notice_impl,
};
use crate::graphql::types::ModerationEntityType;
use crate::moderation::action_type;

fn notice_against(scene_id: Uuid) -> SubmitTakedownNoticeInput {
    SubmitTakedownNoticeInput {
        entity_type: ModerationEntityType::Scene,
        entity_id: scene_id,
        claimant_name: "Map Publisher".into(),
        claimant_contact: "rights@example.test".into(),
        copyrighted_work_description: "A published battle map".into(),
        infringing_material_location: "A scene in a shared collection".into(),
        good_faith_statement: true,
        accuracy_statement: true,
        signature: "Map Publisher".into(),
    }
}

async fn counter_notice(state: &AppState, owner: Uuid, case_id: Uuid) {
    submit_counter_notice_impl(
        state,
        owner,
        false,
        SubmitCounterNoticeInput {
            case_id,
            removed_material_description: "My own map".into(),
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "owner@example.test".into(),
            signature: "The Owner".into(),
        },
    )
    .await
    .expect("the owner may counter-notice");
}

/// The waiting period, elapsed, on a case and every copy's child case.
fn elapse(state: &AppState, case_id: Uuid) {
    use crate::schema::content_moderation_actions as cma;
    let mut conn = state.db_pool.get().expect("connection");
    diesel::update(
        cma::table
            .filter(cma::case_id.eq(case_id).or(cma::parent_case_id.eq(case_id)))
            .filter(cma::action_type.eq(action_type::COUNTER_NOTICE_FORWARDED)),
    )
    .set(cma::restoration_due_at.eq(Some(chrono::Utc::now() - chrono::Duration::days(1))))
    .execute(&mut conn)
    .expect("waiting period moved");
}

fn scene_name(state: &AppState, scene_id: Uuid) -> String {
    use crate::schema::scenes;
    let mut conn = state.db_pool.get().expect("connection");
    scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::name)
        .first(&mut conn)
        .expect("the scene still exists — disabled, never deleted")
}

/// FR-021 to FR-025 for a scene, end to end: withheld from the link and the
/// copy, unnamed, the rest unaffected, and returned by a counter-notice with
/// nothing rebuilt.
#[tokio::test]
async fn a_taken_down_scene_is_withheld_from_the_link_and_the_copy_until_restored() {
    let s = source();
    let code = share_of(
        &s,
        &[("scene", s.scene_id), ("item", s.item_id)],
        "A map and a relic",
    )
    .await;
    let name = scene_name(&s.state, s.scene_id);

    let before = shared_collection_impl(&s.state, "203.0.113.20", code.clone())
        .await
        .expect("opens");
    assert_eq!(before.members.len(), 2);
    assert_eq!(before.withheld_count, 0);

    let case = submit_takedown_notice_impl(&s.state, notice_against(s.scene_id))
        .await
        .expect("a notice can name a scene");

    // FR-021 / FR-022 / FR-023 on the read path.
    let during = shared_collection_impl(&s.state, "203.0.113.20", code.clone())
        .await
        .expect("the collection survives its scene being taken down");
    assert_eq!(during.withheld_count, 1);
    assert_eq!(during.members.len(), 1);
    assert!(during.members.iter().all(|m| m.member_type == "item"));
    assert!(
        !during.members.iter().any(|m| m.name == name),
        "FR-022: the withheld scene is never named"
    );

    // FR-021 on the copy path, asserted on the copy rather than inferred.
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code.clone(),
        s.destination_world_id,
    )
    .await
    .expect("the rest still copies");
    assert!(
        !receipt.created.iter().any(|c| c.member_type == "scene"),
        "a taken-down scene is not created by a copy: {:?}",
        receipt.created
    );
    assert_eq!(receipt.created.len(), 1);
    assert!(!receipt.fidelity_notes.join(" ").contains(&name));

    // Still dark while forwarded; back once the period elapses (FR-025).
    counter_notice(&s.state, s.owner_id, case.case_id).await;
    let forwarded = shared_collection_impl(&s.state, "203.0.113.20", code.clone())
        .await
        .expect("opens");
    assert_eq!(
        forwarded.withheld_count, 1,
        "withheld through the waiting period"
    );

    elapse(&s.state, case.case_id);
    let after = shared_collection_impl(&s.state, "203.0.113.20", code)
        .await
        .expect("opens");
    assert_eq!(after.withheld_count, 0);
    assert!(
        after
            .members
            .iter()
            .any(|m| m.member_type == "scene" && m.name == name),
        "the scene returns to the link without the owner rebuilding anything"
    );
}

/// ADR-079 for a scene: a copy taken before the notice is recorded as an
/// adoption, so the takedown reaches it — disabled as a copy, nobody's strike
/// but the sharer's — and the counter-notice brings it back with its source.
///
/// Proved by behaviour, not by reading the adoption record: nothing outside
/// `moderation::reach` may name that table (`adoption_surface_tests.rs`), and
/// the claim that matters is that the walk finds the copy.
#[tokio::test]
async fn a_scene_copied_before_its_takedown_goes_dark_with_it_and_returns_with_it() {
    let s = source();
    let code = share_of(&s, &[("scene", s.scene_id)], "Just a map").await;

    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");
    let copy = receipt
        .created
        .iter()
        .find(|c| c.member_type == "scene")
        .expect("the scene was copied")
        .id;

    let case = submit_takedown_notice_impl(&s.state, notice_against(s.scene_id))
        .await
        .expect("notice accepted");

    assert_eq!(
        crate::moderation::effective_status(&s.state, "scene", copy)
            .await
            .expect("status")
            .as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
        "the adopted scene must go dark with its source"
    );
    // And in the adopter's own world, not merely in moderation's answer.
    assert!(
        crate::graphql::queries::scene::scene_impl(&s.state, s.recipient_id, false, copy)
            .await
            .expect("scene query answers")
            .is_none(),
        "the adopter cannot read the disabled copy"
    );
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        assert!(
            crate::moderation::strikes_of(&mut conn, s.recipient_id, 365)
                .expect("strikes")
                .is_empty(),
            "FR-023b: the adopter holds no strike for somebody else's upload"
        );
        assert_eq!(
            crate::moderation::strikes_of(&mut conn, s.owner_id, 365)
                .expect("strikes")
                .len(),
            1
        );
    }

    counter_notice(&s.state, s.owner_id, case.case_id).await;
    elapse(&s.state, case.case_id);
    assert_eq!(
        crate::moderation::effective_status(&s.state, "scene", copy)
            .await
            .expect("status"),
        None,
        "the copy comes back with its source"
    );
    assert!(
        crate::graphql::queries::scene::scene_impl(&s.state, s.recipient_id, false, copy)
            .await
            .expect("scene query answers")
            .is_some()
    );
}
