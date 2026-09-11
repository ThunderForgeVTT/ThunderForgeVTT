//! Tests for `mutations_item_shares`.
//!
//! Moved out of that file when spec 039's takedown reach took it past the
//! 1000-line gate, as its three siblings were before it. Pure movement — the
//! module is still `mod tests` and still reaches its parent through
//! `use super::*`.

use super::*;
use crate::graphql::mutations_collection_shares::publishing_gate::publishable_instance;
use crate::graphql::mutations_items::{
    CreateItemInput, ItemEffectInput, add_item_effect_impl, create_item_impl,
};
use crate::graphql::types::ItemEffectType;
use crate::publishing::an_agreement;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

/// A distinct caller per test, so the shared rate limiter's window cannot
/// leak between them: they run concurrently in one process, and a limiter
/// keyed on a constant would make passing depend on test order.
fn a_caller() -> String {
    format!("test-{}", Uuid::new_v4())
}

/// Spec 039 FR-012/FR-013 on the item path. One shape, four files — and the
/// SDL guard in `publishing_gate` is what makes the *fifth* file inherit it
/// without anybody writing a fifth copy of this test.
#[tokio::test]
async fn an_unknown_version_refuses_and_mints_no_item_link() {
    use crate::schema::world_item_shares;

    // Publishable, so the refusal below can only be the agreement's — without
    // it, a missing notice contact refuses first and this passes for the
    // wrong reason.
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Unattested Blade".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");

    let real = an_agreement(&state).await;
    let refusal = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &crate::publishing::AttestationInput {
            terms_version_id: "sharing-terms@ffffffffffffffff".to_string(),
        },
    )
    .await
    .expect_err("a version this instance never archived must be refused");

    let message = refusal.message;
    // The gate's own refusal, not the foreign key's: the key would also
    // stop an unknown version, so without this the test passes with the
    // gate gone.
    assert!(message.contains("current sharing agreement"), "{message}");
    assert!(!message.contains(&real.terms_version_id), "{message}");
    assert!(!message.contains('@'), "{message}");

    let mut conn = state.db_pool.get().unwrap();
    let links: i64 = world_item_shares::table
        .filter(world_item_shares::item_id.eq(item.id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(links, 0, "a refused publish must leave no share link");
}

/// FR-022: only an Owner-level member (including the DM's implicit
/// access) may generate a share link.
#[tokio::test]
async fn create_share_link_requires_owner_level() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Longsword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let denied = create_item_share_link_impl(
        &state,
        outsider_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await;
    assert!(
        denied.is_err(),
        "a non-Owner-level caller must not be able to share the item"
    );

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the DM (implicit Owner) should be able to share the item");
    assert!(!link.revoked);
}

/// FR-025/026: a copy is a fully independent item with cloned effects
/// and an empty ownership block; destination DM access is re-checked.
#[tokio::test]
async fn copy_produces_independent_item_with_cloned_effects() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let source_owner_id = insert_test_user(&mut conn);
    let source_world_id = insert_test_world(&mut conn, source_owner_id);

    let dest_owner_id = insert_test_user(&mut conn);
    let dest_world_id = insert_test_world(&mut conn, dest_owner_id);

    let uninvolved_id = insert_test_user(&mut conn);
    drop(conn);

    let source_item = create_item_impl(
        &state,
        source_owner_id,
        false,
        CreateItemInput {
            world_id: source_world_id,
            name: "Longsword".to_string(),
            description: Some("A fine blade".to_string()),
        },
    )
    .await
    .expect("source DM should create item");

    add_item_effect_impl(
        &state,
        source_owner_id,
        false,
        source_item.id,
        ItemEffectInput {
            effect_type: ItemEffectType::Damage,
            formula: "2d8".to_string(),
            target: "Hit Points".to_string(),
            trigger_kind: None,
            sort_order: Some(0),
        },
    )
    .await
    .expect("effect should be added");

    let link = create_item_share_link_impl(
        &state,
        source_owner_id,
        false,
        source_item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("source DM should be able to share the item");

    let denied = copy_shared_item_to_world_impl(
        &state,
        uninvolved_id,
        false,
        CopySharedItemInput {
            share_code: link.share_code.clone(),
            destination_world_id: dest_world_id,
        },
    )
    .await;
    assert!(
        denied.is_err(),
        "a caller without DM access on the destination must be rejected"
    );

    let (copy, effects) = copy_shared_item_to_world_impl(
        &state,
        dest_owner_id,
        false,
        CopySharedItemInput {
            share_code: link.share_code,
            destination_world_id: dest_world_id,
        },
    )
    .await
    .expect("destination DM should be able to copy the shared item");

    assert_ne!(copy.id, source_item.id, "the copy must have a new identity");
    assert_eq!(copy.world_id, dest_world_id);
    assert_eq!(copy.name, "Longsword");
    assert_eq!(effects.len(), 1, "effects must be cloned onto the copy");
    assert_ne!(
        effects[0].item_id, source_item.id,
        "cloned effect must belong to the copy, not the source"
    );
}

/// Spec 015: a moderation-disabled item's share link must not leak
/// its real content — the share link must not be a takedown bypass.
#[tokio::test]
async fn shared_item_is_unavailable_once_moderation_disabled() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Infringing Sword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("owner should be able to share the item");

    // Sanity: share works before any takedown.
    assert!(
        shared_item_impl(&state, &a_caller(), link.share_code.clone())
            .await
            .is_ok()
    );

    crate::graphql::mutations_moderation::submit_takedown_notice_impl(
        &state,
        crate::graphql::mutations_moderation::SubmitTakedownNoticeInput {
            entity_type: crate::graphql::types::ModerationEntityType::WorldItem,
            entity_id: item.id,
            claimant_name: "Acme".to_string(),
            claimant_contact: "legal@acme.example".to_string(),
            copyrighted_work_description: "Acme Sourcebook".to_string(),
            infringing_material_location: item.id.to_string(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Jane".to_string(),
        },
    )
    .await
    .expect("valid notice should succeed");

    let result = shared_item_impl(&state, &a_caller(), link.share_code).await;
    assert!(
        result.is_err(),
        "a disabled item's share link must stop serving real content"
    );
}

/// ADR-071: with no account required, the refusal must not distinguish an
/// unknown code from a revoked share. Distinguishing them is a probe, and
/// the probe is now free.
#[tokio::test]
async fn a_revoked_share_is_indistinguishable_from_a_code_that_never_existed() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Longsword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    revoke_item_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let revoked = shared_item_impl(&state, &a_caller(), link.share_code)
        .await
        .expect_err("a revoked code must not resolve");
    let unknown = shared_item_impl(&state, &a_caller(), "NOTAREALCODEATALL0".to_string())
        .await
        .expect_err("an unknown code must not resolve");

    assert_eq!(
        revoked.message, unknown.message,
        "the two refusals must be one sentence, or the difference is a probe"
    );
    assert_eq!(revoked.message, UNAVAILABLE);
}

/// ADR-071: the read resolves with no session at all. `shared_item_impl`
/// takes a caller only to rate-limit it, and is reached by a resolver that
/// never calls `authenticated_user` — this asserts the core behaves that
/// way rather than asserting the absence of a line of code.
#[tokio::test]
async fn the_read_needs_no_account() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Longsword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let preview = shared_item_impl(&state, "an-anonymous-visitor-item", link.share_code)
        .await
        .expect("a valid code must resolve for a caller with no account");
    assert_eq!(preview.name, "Longsword");
}

/// ADR-071: an unguessable code is unguessable only while the guesses are
/// bounded, and the account requirement that used to bound them is gone.
#[tokio::test]
async fn the_anonymous_read_is_rate_limited() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Longsword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let caller = a_caller();
    let mut refused = None;
    for _ in 0..200 {
        if let Err(e) = shared_item_impl(&state, &caller, link.share_code.clone()).await {
            refused = Some(e);
            break;
        }
    }

    let error = refused.expect("a caller must eventually be rate limited");
    assert!(
        error.message.contains("Too many requests"),
        "got: {}",
        error.message
    );
    assert_ne!(
        error.message, UNAVAILABLE,
        "being throttled must not read as the code being invalid"
    );
}

/// ADR-071's second half: revoking must not depend on still having the page
/// that minted the link. Before this read path, closing the tab lost the
/// code permanently.
#[tokio::test]
async fn the_owner_can_recover_the_share_code_after_closing_the_page() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let item = create_item_impl(
        &state,
        owner_id,
        false,
        CreateItemInput {
            world_id,
            name: "Longsword".to_string(),
            description: None,
        },
    )
    .await
    .expect("DM should create item");

    let link = create_item_share_link_impl(
        &state,
        owner_id,
        false,
        item.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let recovered = item_share_link_impl(&state, owner_id, false, item.id)
        .await
        .expect("the owner may read back their own share link")
        .expect("an active share must be found");
    assert_eq!(recovered.share_code, link.share_code);
    assert_eq!(recovered.id, link.id);

    revoke_item_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let after = item_share_link_impl(&state, owner_id, false, item.id)
        .await
        .expect("reading back after revocation is not an error");
    assert!(
        after.is_none(),
        "a revoked share is not an active share link"
    );
}
