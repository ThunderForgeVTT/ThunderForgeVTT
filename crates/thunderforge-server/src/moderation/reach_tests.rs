//! Spec 039 US6: what a takedown does to the copies people took (T040, T041).

use super::*;

use crate::graphql::mutations_moderation::{
    SubmitCounterNoticeInput, SubmitTakedownNoticeInput, resolve_moderation_case_impl,
    submit_counter_notice_impl, submit_takedown_notice_impl,
};
use crate::graphql::types::{ModerationActionType, ModerationEntityType};
use crate::moderation::{effective_status, strikes_of};
use crate::state::AppState;
use crate::test_support::{insert_test_item, insert_test_user, insert_test_world, test_app_state};

const CLAIMANT: &str = "Reach Test Claimant";

fn notice(entity_id: Uuid) -> SubmitTakedownNoticeInput {
    SubmitTakedownNoticeInput {
        entity_type: ModerationEntityType::WorldItem,
        entity_id,
        claimant_name: CLAIMANT.into(),
        claimant_contact: "claimant@example.test".into(),
        copyrighted_work_description: "A work the claimant owns".into(),
        infringing_material_location: "The shared item".into(),
        good_faith_statement: true,
        accuracy_statement: true,
        signature: CLAIMANT.into(),
    }
}

/// A sharer's item, one adopter's copy of it, and a neighbour the adopter made
/// themselves beside the copy.
struct Scene {
    sharer: Uuid,
    adopter: Uuid,
    source: Uuid,
    copy: Uuid,
    neighbour: Uuid,
}

fn one_adoption(state: &AppState) -> Scene {
    let mut conn = state.db_pool.get().expect("connection");
    let sharer = insert_test_user(&mut conn);
    let adopter = insert_test_user(&mut conn);
    let source_world = insert_test_world(&mut conn, sharer);
    let adopter_world = insert_test_world(&mut conn, adopter);
    let source = insert_test_item(&mut conn, source_world, sharer);
    let copy = insert_test_item(&mut conn, adopter_world, adopter);
    let neighbour = insert_test_item(&mut conn, adopter_world, adopter);
    record_adoption_sync(
        &mut conn,
        "world_item",
        source,
        copy,
        adopter_world,
        adopter,
    )
    .expect("adoption recorded");
    Scene {
        sharer,
        adopter,
        source,
        copy,
        neighbour,
    }
}

async fn status(state: &AppState, item: Uuid) -> Option<String> {
    effective_status(state, "world_item", item)
        .await
        .expect("status readable")
}

async fn kinds_for(state: &AppState, account: Uuid) -> Vec<String> {
    crate::notices::for_account(state, account, 20)
        .await
        .expect("notices readable")
        .into_iter()
        .map(|notice| notice.kind)
        .collect()
}

/// T040 / FR-023b. The copy goes dark, and nobody but the sharer holds a
/// strike for it. Give the child case the adopter's account and this fails.
#[tokio::test]
async fn a_copy_taken_down_with_its_source_is_nobodys_strike() {
    let state = test_app_state();
    let scene = one_adoption(&state);

    let case = submit_takedown_notice_impl(&state, notice(scene.source))
        .await
        .expect("notice accepted");

    assert_eq!(
        status(&state, scene.copy).await.as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
        "the copy must go dark with its source (FR-023)",
    );

    let mut conn = state.db_pool.get().expect("connection");
    let child_rows: Vec<(Option<Uuid>, Option<Uuid>, Uuid)> = content_moderation_actions::table
        .filter(content_moderation_actions::entity_id.eq(scene.copy))
        .select((
            content_moderation_actions::account_id,
            content_moderation_actions::parent_case_id,
            content_moderation_actions::case_id,
        ))
        .load(&mut conn)
        .expect("child rows");
    assert_eq!(child_rows.len(), 1, "one child case, opened once");
    let (account, parent, child_case) = child_rows[0];
    assert_eq!(account, None, "a child case carries no account (FR-023b)");
    assert_eq!(parent, Some(case.case_id), "and points back at the notice");
    assert_ne!(child_case, case.case_id, "in a case of its own");

    assert!(
        strikes_of(&mut conn, scene.adopter, 365)
            .expect("strikes")
            .is_empty(),
        "an adopter accrues no strike for somebody else's upload",
    );
    assert_eq!(
        strikes_of(&mut conn, scene.sharer, 365)
            .expect("strikes")
            .len(),
        1,
        "the sharer's one strike is unchanged by how many copies there were",
    );
}

/// FR-023a / FR-023c. Disabled, not deleted — and only the copy. The item the
/// adopter made beside it is not moderation's business.
#[tokio::test]
async fn the_adopters_own_work_is_untouched_and_nothing_is_deleted() {
    let state = test_app_state();
    let scene = one_adoption(&state);

    submit_takedown_notice_impl(&state, notice(scene.source))
        .await
        .expect("notice accepted");

    assert_eq!(status(&state, scene.neighbour).await, None);

    let mut conn = state.db_pool.get().expect("connection");
    use crate::schema::world_items;
    let still_there: i64 = world_items::table
        .filter(world_items::id.eq(scene.copy))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(still_there, 1, "a disabled copy is never deleted");
}

/// FR-023b, FR-024. The adopter is told, in a notice that names their copy and
/// nothing about the notice — no claimant, no source, no sharer. The sharer is
/// told the takedown reached copies.
#[tokio::test]
async fn the_adopter_is_told_without_being_told_who_or_what_from() {
    let state = test_app_state();
    let scene = one_adoption(&state);

    submit_takedown_notice_impl(&state, notice(scene.source))
        .await
        .expect("notice accepted");

    let told = crate::notices::for_account(&state, scene.adopter, 20)
        .await
        .expect("notices");
    let disabled: Vec<_> = told
        .iter()
        .filter(|n| n.kind == crate::notices::kind::ADOPTED_COPY_DISABLED)
        .collect();
    assert_eq!(
        disabled.len(),
        1,
        "one notice per adopter, however many copies"
    );

    let everything = format!("{:?} {:?}", disabled[0].payload, disabled[0].subject_ref);
    for leaked in [
        CLAIMANT.to_string(),
        scene.source.to_string(),
        scene.sharer.to_string(),
    ] {
        assert!(
            !everything.contains(&leaked),
            "the adopter's notice must not name `{leaked}`: {everything}",
        );
    }

    assert!(
        kinds_for(&state, scene.sharer)
            .await
            .iter()
            .any(|kind| kind == crate::notices::kind::SHARE_TAKEN_DOWN),
        "the sharer hears that the takedown reached copies (FR-024)",
    );
}

/// T041. A copy of a copy is reached, and a copy deleted in between does not
/// stop the walk — it has nothing to disable, and the one made from it does.
#[tokio::test]
async fn the_walk_is_transitive_and_passes_through_a_deleted_copy() {
    let state = test_app_state();
    let (source, middle, last) = {
        let mut conn = state.db_pool.get().expect("connection");
        let sharer = insert_test_user(&mut conn);
        let first = insert_test_user(&mut conn);
        let second = insert_test_user(&mut conn);
        let source_world = insert_test_world(&mut conn, sharer);
        let first_world = insert_test_world(&mut conn, first);
        let second_world = insert_test_world(&mut conn, second);
        let source = insert_test_item(&mut conn, source_world, sharer);
        let middle = insert_test_item(&mut conn, first_world, first);
        let last = insert_test_item(&mut conn, second_world, second);
        record_adoption_sync(&mut conn, "world_item", source, middle, first_world, first)
            .expect("first adoption");
        record_adoption_sync(&mut conn, "world_item", middle, last, second_world, second)
            .expect("second adoption");
        (source, middle, last)
    };

    submit_takedown_notice_impl(&state, notice(source))
        .await
        .expect("notice accepted");
    assert_eq!(
        status(&state, last).await.as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
        "a copy of a copy is reached",
    );
    assert_eq!(
        status(&state, middle).await.as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
    );

    // Again, with the middle copy deleted by its adopter first.
    let (source, last) = {
        let mut conn = state.db_pool.get().expect("connection");
        let sharer = insert_test_user(&mut conn);
        let first = insert_test_user(&mut conn);
        let second = insert_test_user(&mut conn);
        let source_world = insert_test_world(&mut conn, sharer);
        let first_world = insert_test_world(&mut conn, first);
        let second_world = insert_test_world(&mut conn, second);
        let source = insert_test_item(&mut conn, source_world, sharer);
        let middle = insert_test_item(&mut conn, first_world, first);
        let last = insert_test_item(&mut conn, second_world, second);
        record_adoption_sync(&mut conn, "world_item", source, middle, first_world, first)
            .expect("first adoption");
        record_adoption_sync(&mut conn, "world_item", middle, last, second_world, second)
            .expect("second adoption");
        use crate::schema::world_items;
        diesel::delete(world_items::table.filter(world_items::id.eq(middle)))
            .execute(&mut conn)
            .expect("middle copy deleted");
        (source, last)
    };

    submit_takedown_notice_impl(&state, notice(source))
        .await
        .expect("notice accepted");
    assert_eq!(
        status(&state, last).await.as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
        "a deleted copy in between must not hide the one made from it",
    );
}

/// FR-023d. A successful counter-notice brings the copy back with the source,
/// on the same date, without the adopter asking — and tells them.
#[tokio::test]
async fn a_counter_notice_brings_the_copies_back_with_the_source() {
    let state = test_app_state();
    let scene = one_adoption(&state);

    let case = submit_takedown_notice_impl(&state, notice(scene.source))
        .await
        .expect("notice accepted");
    submit_counter_notice_impl(
        &state,
        scene.sharer,
        false,
        SubmitCounterNoticeInput {
            case_id: case.case_id,
            removed_material_description: "The shared item".into(),
            good_faith_mistake_statement: true,
            consent_to_jurisdiction: true,
            contact_information: "sharer@example.test".into(),
            signature: "The Sharer".into(),
        },
    )
    .await
    .expect("counter-notice accepted");

    let mut conn = state.db_pool.get().expect("connection");
    let forwarded = |conn: &mut PgConnection| -> Vec<Option<DateTime<Utc>>> {
        content_moderation_actions::table
            .filter(
                content_moderation_actions::case_id
                    .eq(case.case_id)
                    .or(content_moderation_actions::parent_case_id.eq(case.case_id)),
            )
            .filter(
                content_moderation_actions::action_type.eq(action_type::COUNTER_NOTICE_FORWARDED),
            )
            .select(content_moderation_actions::restoration_due_at)
            .load(conn)
            .expect("forwarded rows")
    };
    let dates = forwarded(&mut conn);
    assert_eq!(dates.len(), 2, "forwarded on the source and on the copy");
    assert_eq!(dates[0], dates[1], "with the same restoration date");

    // The waiting period passes.
    diesel::update(
        content_moderation_actions::table
            .filter(
                content_moderation_actions::case_id
                    .eq(case.case_id)
                    .or(content_moderation_actions::parent_case_id.eq(case.case_id)),
            )
            .filter(
                content_moderation_actions::action_type.eq(action_type::COUNTER_NOTICE_FORWARDED),
            ),
    )
    .set(
        content_moderation_actions::restoration_due_at
            .eq(Some(Utc::now() - chrono::Duration::days(1))),
    )
    .execute(&mut conn)
    .expect("waiting period moved");
    drop(conn);

    assert_eq!(
        status(&state, scene.source).await,
        None,
        "the source is back"
    );
    assert_eq!(status(&state, scene.copy).await, None, "and so is the copy");
    assert!(
        kinds_for(&state, scene.adopter)
            .await
            .iter()
            .any(|kind| kind == crate::notices::kind::ADOPTED_COPY_RESTORED),
        "the adopter is told it came back",
    );
}

/// `resolveModerationCase` on the notice's case applies to its copies: restored
/// with the source, or kept dark with it — never the source alone.
#[tokio::test]
async fn an_administrators_resolution_reaches_the_copies() {
    let state = test_app_state();

    let restored = one_adoption(&state);
    let case = submit_takedown_notice_impl(&state, notice(restored.source))
        .await
        .expect("notice accepted");
    resolve_moderation_case_impl(
        &state,
        true,
        case.case_id,
        ModerationActionType::ContentRestored,
    )
    .await
    .expect("resolved");
    assert_eq!(status(&state, restored.copy).await, None);
    assert!(
        kinds_for(&state, restored.adopter)
            .await
            .iter()
            .any(|kind| kind == crate::notices::kind::ADOPTED_COPY_RESTORED),
    );

    let upheld = one_adoption(&state);
    let case = submit_takedown_notice_impl(&state, notice(upheld.source))
        .await
        .expect("notice accepted");
    resolve_moderation_case_impl(
        &state,
        true,
        case.case_id,
        ModerationActionType::ContentRemainsDisabled,
    )
    .await
    .expect("resolved");
    assert_eq!(
        status(&state, upheld.copy).await.as_deref(),
        Some(action_type::CONTENT_REMAINS_DISABLED),
    );
}

/// T047: the singleton copy path records the adoption — proved by behaviour,
/// through a real share link and a real copy, so the test needs no access to
/// the record itself.
#[tokio::test]
async fn a_copy_made_through_an_item_link_is_reached() {
    let _publishing =
        crate::graphql::mutations_collection_shares::publishing_gate::publishable_instance();
    let state = test_app_state();
    let (sharer, adopter, adopter_world, source) = {
        let mut conn = state.db_pool.get().expect("connection");
        let sharer = insert_test_user(&mut conn);
        let adopter = insert_test_user(&mut conn);
        let source_world = insert_test_world(&mut conn, sharer);
        let adopter_world = insert_test_world(&mut conn, adopter);
        let source = insert_test_item(&mut conn, source_world, sharer);
        (sharer, adopter, adopter_world, source)
    };

    let link = crate::graphql::mutations_item_shares::create_item_share_link_impl(
        &state,
        sharer,
        false,
        source,
        &crate::publishing::an_agreement(&state).await,
    )
    .await
    .expect("shared");
    let (copy, _effects) = crate::graphql::mutations_item_shares::copy_shared_item_to_world_impl(
        &state,
        adopter,
        false,
        crate::graphql::mutations_item_shares::CopySharedItemInput {
            share_code: link.share_code,
            destination_world_id: adopter_world,
        },
    )
    .await
    .expect("copied");

    submit_takedown_notice_impl(&state, notice(source))
        .await
        .expect("notice accepted");
    assert_eq!(
        status(&state, copy.id).await.as_deref(),
        Some(action_type::CONTENT_DISABLED_AS_COPY),
        "a copy made through an item link must be reachable by its source's takedown",
    );
}
