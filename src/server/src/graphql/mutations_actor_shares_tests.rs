//! Tests for `mutations_actor_shares`.
//!
//! Moved out of that file when spec 039's attestation tests took it past the
//! 1000-line gate. Pure movement — the module is still `mod tests` and still
//! reaches its parent through `use super::*`.

use super::*;
use crate::graphql::mutations_actors::{CreateActorInput, create_actor_impl};
use crate::graphql::mutations_collection_shares::publishing_gate::publishable_instance;
use crate::publishing::an_agreement;
use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, test_app_state};

/// A distinct caller per test, so the shared rate limiter's window cannot
/// leak between them: they run concurrently in one process, and a limiter
/// keyed on a constant would make passing depend on test order.
fn a_caller() -> String {
    format!("test-{}", Uuid::new_v4())
}

/// Spec 039 FR-012/FR-013, on the actor path — the one where a person
/// publishes a character sheet that may be a publisher's rules text typed
/// into a form, and which ADR-071 made readable without an account.
///
/// Same shape as the collection path's test: an unknown version refuses,
/// the message names no valid identity, and **no share row is left behind**.
#[tokio::test]
async fn an_unknown_version_refuses_and_mints_no_actor_link() {
    use crate::schema::world_actor_shares;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Unattested".to_string(),
            is_npc: true,
            actor_type: None,
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("created");

    let real = an_agreement(&state).await;
    let refusal = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
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
    let links: i64 = world_actor_shares::table
        .filter(world_actor_shares::actor_id.eq(actor.id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(links, 0, "a refused publish must leave no share link");
}

/// FR-023: only an Owner-level member (including the DM's implicit
/// access) may generate a share link.
#[tokio::test]
async fn create_share_link_requires_owner_level() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: None,
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    // A user with no relationship to this world at all (not even a
    // membership row) has no permission on the actor — default Viewer.
    let denied = create_actor_share_link_impl(
        &state,
        outsider_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await;
    assert!(
        denied.is_err(),
        "a non-Owner-level caller must not be able to share the actor"
    );

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the DM (implicit Owner) should be able to share the actor");
    assert!(!link.revoked);
    assert_eq!(link.actor_id, actor.id);
}

/// FR-024: `sharedActor` rejects a revoked code and never leaks the
/// source world/scene/owner identity.
#[tokio::test]
async fn shared_actor_rejects_revoked_and_scrubs_identity() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: Some("npc".to_string()),
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("DM should be able to share the actor");

    let preview = shared_actor_impl(&state, &a_caller(), link.share_code.clone())
        .await
        .expect("a valid share code should resolve");
    assert_eq!(preview.label, "Bo Jangles");

    revoke_actor_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("DM should be able to revoke");

    let after_revoke = shared_actor_impl(&state, &a_caller(), link.share_code).await;
    assert!(
        after_revoke.is_err(),
        "a revoked share code must no longer resolve"
    );

    let missing = shared_actor_impl(&state, &a_caller(), "DOES-NOT-EXIST".to_string()).await;
    assert!(missing.is_err(), "an unknown share code must not resolve");
}

/// FR-026/027/030: a copy is a fully independent actor with cloned
/// system data and an empty ownership block, and the destination DM
/// requirement is re-checked server-side.
#[tokio::test]
async fn copy_produces_independent_actor_and_rechecks_destination_access() {
    let _publishing = publishable_instance();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let source_owner_id = insert_test_user(&mut conn);
    let source_world_id = insert_test_world(&mut conn, source_owner_id);
    insert_test_scene(&mut conn, source_world_id, source_owner_id);

    let dest_owner_id = insert_test_user(&mut conn);
    let dest_world_id = insert_test_world(&mut conn, dest_owner_id);
    insert_test_scene(&mut conn, dest_world_id, dest_owner_id);

    let uninvolved_id = insert_test_user(&mut conn);
    drop(conn);

    let source_actor = create_actor_impl(
        &state,
        source_owner_id,
        false,
        CreateActorInput {
            world_id: source_world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: None,
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("source DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        source_owner_id,
        false,
        source_actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("source DM should be able to share the actor");

    // A user with no DM-level access anywhere near the destination
    // world must be rejected, even with a valid share code.
    let denied = copy_shared_actor_to_world_impl(
        &state,
        uninvolved_id,
        false,
        CopySharedActorInput {
            share_code: link.share_code.clone(),
            destination_world_id: dest_world_id,
        },
    )
    .await;
    assert!(
        denied.is_err(),
        "a caller without DM access on the destination must be rejected"
    );

    let copy = copy_shared_actor_to_world_impl(
        &state,
        dest_owner_id,
        false,
        CopySharedActorInput {
            share_code: link.share_code,
            destination_world_id: dest_world_id,
        },
    )
    .await
    .expect("destination DM should be able to copy the shared actor");

    assert_ne!(
        copy.id, source_actor.id,
        "the copy must have a new identity"
    );
    assert_eq!(copy.world_id, dest_world_id);
    assert_eq!(copy.label, "Bo Jangles");

    let copy_permissions = crate::graphql::mutations_actor_permissions::actor_permissions_impl(
        &state,
        dest_owner_id,
        false,
        copy.id,
    )
    .await
    .expect("destination DM should be able to view the copy's ownership block");
    assert!(
        copy_permissions.is_empty(),
        "a freshly copied actor must start with an empty ownership block (FR-030)"
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
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: Some("npc".to_string()),
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    revoke_actor_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let revoked = shared_actor_impl(&state, &a_caller(), link.share_code)
        .await
        .expect_err("a revoked code must not resolve");
    let unknown = shared_actor_impl(&state, &a_caller(), "NOTAREALCODEATALL0".to_string())
        .await
        .expect_err("an unknown code must not resolve");

    assert_eq!(
        revoked.message, unknown.message,
        "the two refusals must be one sentence, or the difference is a probe"
    );
    assert_eq!(revoked.message, UNAVAILABLE);
}

/// ADR-071: the read resolves with no session at all. `shared_actor_impl`
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
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: Some("npc".to_string()),
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let preview = shared_actor_impl(&state, "an-anonymous-visitor-actor", link.share_code)
        .await
        .expect("a valid code must resolve for a caller with no account");
    assert_eq!(preview.label, "Bo Jangles");
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
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: Some("npc".to_string()),
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let caller = a_caller();
    let mut refused = None;
    for _ in 0..200 {
        if let Err(e) = shared_actor_impl(&state, &caller, link.share_code.clone()).await {
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
    insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);

    let actor = create_actor_impl(
        &state,
        owner_id,
        false,
        CreateActorInput {
            world_id,
            label: "Bo Jangles".to_string(),
            is_npc: true,
            actor_type: Some("npc".to_string()),
            game_system_id: None,
            description: None,
        },
    )
    .await
    .expect("DM should create actor");

    let link = create_actor_share_link_impl(
        &state,
        owner_id,
        false,
        actor.id,
        &an_agreement(&state).await,
    )
    .await
    .expect("the owner may share");

    let recovered = actor_share_link_impl(&state, owner_id, false, actor.id)
        .await
        .expect("the owner may read back their own share link")
        .expect("an active share must be found");
    assert_eq!(recovered.share_code, link.share_code);
    assert_eq!(recovered.id, link.id);

    revoke_actor_share_link_impl(&state, owner_id, false, link.id)
        .await
        .expect("the owner may revoke");

    let after = actor_share_link_impl(&state, owner_id, false, actor.id)
        .await
        .expect("reading back after revocation is not an error");
    assert!(
        after.is_none(),
        "a revoked share is not an active share link"
    );
}
