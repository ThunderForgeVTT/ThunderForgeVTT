//! Tests for `mutations_ability_shares`.
//!
//! Moved out of that file when spec 039's attestation tests took it past the
//! 1000-line gate. Pure movement — the module is still `mod tests` and still
//! reaches its parent through `use super::*`.

use super::*;
use crate::graphql::mutations_abilities::{
    AbilityEffectInput, CreateAbilityInput, add_ability_effect_impl, create_ability_impl,
    set_ability_gm_only_impl,
};
use crate::graphql::mutations_collection_shares::publishing_gate::publishable_instance;
use crate::graphql::types::AbilityEffectType;
use crate::publishing::an_agreement;
use crate::test_support::*;

/// A distinct caller per test, so the shared rate limiter's window cannot
/// leak between them: they run concurrently in one process, and a limiter
/// keyed on a constant would make passing depend on test order.
fn a_caller() -> String {
    format!("test-{}", Uuid::new_v4())
}

fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
    CreateAbilityInput {
        world_id,
        name: name.to_string(),
        description: Some("A source ability.".to_string()),
        classification: "spell".to_string(),
        grade: None,
        gm_only: None,
    }
}

/// Spec 039 FR-012/FR-013 on the ability path, the fourth of four.
#[tokio::test]
async fn an_unknown_version_refuses_and_mints_no_ability_link() {
    use crate::schema::world_ability_shares;

    // Publishable, so the refusal below can only be the agreement's — without
    // it, a missing notice contact refuses first and this passes for the
    // wrong reason.
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(
        &state,
        owner_id,
        false,
        ability_input(world_id, "Unattested Cantrip"),
    )
    .await
    .expect("created");

    let real = an_agreement(&state).await;
    let refusal = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &crate::publishing::AttestationInput {
            terms_version_id: "sharing-terms@ffffffffffffffff".to_string(),
        },
    )
    .await
    .expect_err("a version this instance never archived must be refused");

    let message = refusal.message;
    // The gate's own refusal, not the foreign key's: the key would also stop
    // an unknown version, so without this the test passes with the gate gone.
    assert!(message.contains("current sharing agreement"), "{message}");
    assert!(!message.contains(&real.terms_version_id), "{message}");
    assert!(!message.contains('@'), "{message}");

    let mut conn = state.db_pool.get().unwrap();
    let links: i64 = world_ability_shares::table
        .filter(world_ability_shares::ability_id.eq(ability.id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(links, 0, "a refused publish must leave no share link");
}

/// FR-032: Owner-level only.
#[tokio::test]
async fn create_ability_share_link_requires_owner_level() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let member_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_world_member(&mut conn, world_id, member_id, "Player");
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Shared"))
        .await
        .unwrap();

    create_ability_share_link_impl(
        &state,
        member_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect_err("a Viewer must not create a share link");

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the DM has implicit Owner and may share");
    assert!(!link.revoked);
    assert_eq!(link.share_code.len(), 20, "20-char code");
    assert_eq!(link.share_code, link.share_code.to_uppercase());
}

/// FR-035/SC-008: the copy is fully independent, and gm_only is preserved
/// rather than reset (fail closed).
#[tokio::test]
async fn copy_produces_independent_ability_with_cloned_effects() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let other_id = insert_test_user(&mut conn);
    let source_world = insert_test_world(&mut conn, owner_id);
    let dest_world = insert_test_world(&mut conn, other_id);
    drop(conn);

    let ability = create_ability_impl(
        &state,
        owner_id,
        false,
        ability_input(source_world, "Fireball"),
    )
    .await
    .unwrap();
    add_ability_effect_impl(
        &state,
        owner_id,
        false,
        ability.id,
        AbilityEffectInput {
            effect_type: AbilityEffectType::Damage,
            formula: "3d6".to_string(),
            target: "Hit Points".to_string(),
            trigger_kind: None,
            sort_order: Some(0),
        },
    )
    .await
    .unwrap();
    set_ability_gm_only_impl(&state, owner_id, false, ability.id, true)
        .await
        .unwrap();

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .unwrap();
    let input = CopySharedAbilityInput {
        share_code: link.share_code.clone(),
        destination_world_id: dest_world,
    };

    // Not a DM of the destination → rejected.
    copy_shared_ability_to_world_impl(&state, owner_id, false, input.clone())
        .await
        .expect_err("only a DM of the destination world may copy into it");

    let (copy, effects) = copy_shared_ability_to_world_impl(&state, other_id, false, input)
        .await
        .expect("the destination DM may copy");

    assert_ne!(copy.id, ability.id, "a new identity");
    assert_eq!(copy.world_id, dest_world);
    assert_eq!(copy.name, "Fireball");
    assert_eq!(copy.created_by, other_id);
    assert!(copy.gm_only, "gm_only is preserved on copy, not reset");
    assert_eq!(effects.len(), 1, "effects are cloned");
    assert_eq!(
        effects[0].ability_id, copy.id,
        "and re-parented to the copy"
    );
    assert_eq!(effects[0].formula, "3d6");

    // Independence: editing the source does not touch the copy.
    let mut conn = state.db_pool.get().unwrap();
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability.id)))
        .set(world_abilities::name.eq("Renamed Source"))
        .execute(&mut conn)
        .unwrap();
    let reloaded: String = world_abilities::table
        .filter(world_abilities::id.eq(copy.id))
        .select(world_abilities::name)
        .first(&mut conn)
        .unwrap();
    assert_eq!(
        reloaded, "Fireball",
        "the copy is unaffected by source edits"
    );
}

/// FR-036: revoking makes the link resolve to a distinct unavailable state.
#[tokio::test]
async fn revoked_share_link_is_unavailable() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Temp"))
        .await
        .unwrap();
    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .unwrap();

    shared_ability_impl(&state, &a_caller(), link.share_code.clone())
        .await
        .expect("an active link resolves");

    assert!(
        revoke_ability_share_link_impl(&state, owner_id, false, link.id)
            .await
            .unwrap()
    );

    let err = shared_ability_impl(&state, &a_caller(), link.share_code)
        .await
        .expect_err("a revoked link must not resolve");
    assert!(err.message.contains("no longer available"));
}

/// A share must never become a moderation bypass — the property the DMCA
/// determination's "takedown-effective" invariant names.
#[tokio::test]
async fn shared_ability_is_unavailable_once_moderation_disabled() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(
        &state,
        owner_id,
        false,
        ability_input(world_id, "Infringing"),
    )
    .await
    .unwrap();
    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .unwrap();
    shared_ability_impl(&state, &a_caller(), link.share_code.clone())
        .await
        .expect("resolves before moderation");

    crate::graphql::mutations_moderation::submit_takedown_notice_impl(
        &state,
        crate::graphql::mutations_moderation::SubmitTakedownNoticeInput {
            entity_type: crate::graphql::types::ModerationEntityType::WorldAbility,
            entity_id: ability.id,
            claimant_name: "Rights Holder".to_string(),
            claimant_contact: "rights@example.test".to_string(),
            copyrighted_work_description: "A published spell".to_string(),
            infringing_material_location: "this ability".to_string(),
            good_faith_statement: true,
            accuracy_statement: true,
            signature: "Rights Holder".to_string(),
        },
    )
    .await
    .expect("takedown submission");

    let err = shared_ability_impl(&state, &a_caller(), link.share_code)
        .await
        .expect_err("a moderated ability's share must stop resolving");
    assert!(err.message.contains("no longer available"));
}

/// FR-033: the preview must not let a viewer identify the source world or
/// its members. Enforced by the type carrying no such fields — this test
/// pins the shape so a later "convenience" addition is a visible break.
#[tokio::test]
async fn shared_ability_preview_omits_source_world_identity() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Quiet"))
        .await
        .unwrap();
    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .unwrap();

    let preview = shared_ability_impl(&state, &a_caller(), link.share_code)
        .await
        .unwrap();
    assert_eq!(preview.name, "Quiet");
    // Spec 033 FR-006: a share view names the type in the owning world's
    // words, resolved server-side because the viewer is deliberately not a
    // member of that world and cannot read its vocabulary. This world runs
    // no system, so the label is the application's own — but it is a
    // resolved label rather than an enum the client has to translate, and
    // it is never blank.
    assert!(!preview.classification_label.is_empty());

    // Destructured exhaustively: adding an id/world_id/created_by field to
    // SharedAbilityPreview breaks this line, which is the point.
    let SharedAbilityPreview {
        name: _,
        description: _,
        classification: _,
        classification_label: _,
        effects: _,
    } = preview;
}

/// ADR-071: with no account required, the refusal must not distinguish an
/// unknown code from a revoked share. Distinguishing them is a probe, and
/// the probe is now free.
#[tokio::test]
async fn a_revoked_share_is_indistinguishable_from_a_code_that_never_existed() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Shared"))
        .await
        .unwrap();

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    revoke_ability_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let revoked = shared_ability_impl(&state, &a_caller(), link.share_code)
        .await
        .expect_err("a revoked code must not resolve");
    let unknown = shared_ability_impl(&state, &a_caller(), "NOTAREALCODEATALL0".to_string())
        .await
        .expect_err("an unknown code must not resolve");

    assert_eq!(
        revoked.message, unknown.message,
        "the two refusals must be one sentence, or the difference is a probe"
    );
    assert_eq!(revoked.message, UNAVAILABLE);
}

/// ADR-071: the read resolves with no session at all. `shared_ability_impl`
/// takes a caller only to rate-limit it, and is reached by a resolver that
/// never calls `authenticated_user` — this asserts the core behaves that
/// way rather than asserting the absence of a line of code.
#[tokio::test]
async fn the_read_needs_no_account() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Shared"))
        .await
        .unwrap();

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let preview = shared_ability_impl(&state, "an-anonymous-visitor-ability", link.share_code)
        .await
        .expect("a valid code must resolve for a caller with no account");
    assert_eq!(preview.name, "Shared");
}

/// ADR-071: an unguessable code is unguessable only while the guesses are
/// bounded, and the account requirement that used to bound them is gone.
#[tokio::test]
async fn the_anonymous_read_is_rate_limited() {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Shared"))
        .await
        .unwrap();

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let caller = a_caller();
    let mut refused = None;
    for _ in 0..200 {
        if let Err(e) = shared_ability_impl(&state, &caller, link.share_code.clone()).await {
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
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Shared"))
        .await
        .unwrap();

    let link = create_ability_share_link_impl(
        &state,
        owner_id,
        false,
        ability.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let recovered = ability_share_link_impl(&state, owner_id, false, ability.id)
        .await
        .expect("the owner may read back their own share link")
        .expect("an active share must be found");
    assert_eq!(recovered.share_code, link.share_code);
    assert_eq!(recovered.id, link.id);

    revoke_ability_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let after = ability_share_link_impl(&state, owner_id, false, ability.id)
        .await
        .expect("reading back after revocation is not an error");
    assert!(
        after.is_none(),
        "a revoked share is not an active share link"
    );
}
