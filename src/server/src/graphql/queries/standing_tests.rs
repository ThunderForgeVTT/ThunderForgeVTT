//! A takedown becomes a strike the person can see, and a consequence they meet.
//!
//! Driven through the real intake path — `submit_takedown_notice_impl`, the
//! same function the public form reaches — so what is asserted is what a real
//! notice does, not what a hand-built row does.

use super::*;
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::mutations_moderation::{
    SubmitTakedownNoticeInput, submit_takedown_notice_impl,
};
use crate::test_support::{insert_test_item, insert_test_user, insert_test_world, test_app_state};
use async_graphql::Request;

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn as_user(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
    }
}

fn a_valid_notice(item_id: Uuid) -> SubmitTakedownNoticeInput {
    SubmitTakedownNoticeInput {
        entity_type: ModerationEntityType::WorldItem,
        entity_id: item_id,
        claimant_name: "Jane Claimant".to_string(),
        claimant_contact: "jane@realdomain.org".to_string(),
        copyrighted_work_description: "An original work, registered copyright.".to_string(),
        infringing_material_location: "An item in a ThunderForge world.".to_string(),
        good_faith_statement: true,
        accuracy_statement: true,
        signature: "Jane Claimant".to_string(),
    }
}

/// US5 end to end, at the server: two valid takedowns against somebody's
/// items make two strikes, the person can see both and was told of both, and
/// publishing is refused in terms they can act on (FR-018, FR-021, FR-028,
/// FR-029).
#[tokio::test]
async fn two_takedowns_are_two_strikes_the_person_sees_and_a_share_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let first = insert_test_item(&mut conn, world, owner);
    let second = insert_test_item(&mut conn, world, owner);
    drop(conn);

    for item in [first, second] {
        submit_takedown_notice_impl(&state, a_valid_notice(item))
            .await
            .expect("a valid notice is accepted");
    }

    let response = schema(state.clone())
        .execute(
            Request::new(
                "{ myStanding { strikeCount mayPublish warned threshold suspendPublishingAt \
                   strikes { entityType entityId agesOutAt } } \
                   myNotices { kind payload } }",
            )
            .data(as_user(owner)),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");

    let standing = &data["myStanding"];
    assert_eq!(standing["strikeCount"], 2);
    assert_eq!(standing["mayPublish"], false);
    assert_eq!(standing["warned"], true);
    let strikes = standing["strikes"].as_array().expect("strikes");
    assert_eq!(strikes.len(), 2);
    assert!(
        strikes
            .iter()
            .all(|s| s["entityType"] == "WORLD_ITEM" && s["agesOutAt"].is_string()),
        "each strike says what it was and when it stops counting (FR-029)",
    );

    let kinds: Vec<&str> = data["myNotices"]
        .as_array()
        .expect("notices")
        .iter()
        .map(|n| n["kind"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "strike_recorded").count(),
        2,
        "told of each strike (FR-028): {kinds:?}",
    );
    assert!(
        kinds.contains(&"publishing_suspended"),
        "and told that sharing has stopped: {kinds:?}",
    );

    // The consequence, at the gate every publishing path shares.
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let refused = crate::publishing::require_attestation(
        &state,
        owner,
        crate::attestation::PublishableKind::Item,
        first,
        Some(world),
        &crate::publishing::AttestationInput {
            terms_version_id: crate::legal::sharing_terms().version_id,
        },
    )
    .await
    .expect_err("an account at the suspension rung may not publish");
    assert_eq!(
        refused.message,
        crate::publishing::PUBLISHING_SUSPENDED,
        "refused for its standing, not for its agreement — a person told to \
         reload would reload forever",
    );
}

/// T056 / FR-019, FR-038: losing the ability to publish is not losing access.
///
/// A player holds grants on a Game Master's content in the GM's world. Their
/// own items, in their own world, are taken down twice — suspending them — and
/// every grant they held is still there. Nothing on the strike path reaches
/// into permissions, and this is what notices if something ever does.
#[tokio::test]
async fn a_suspended_account_keeps_every_permission_it_had() {
    use crate::test_support::{
        count_content_permissions, grant_all_content_permissions, insert_test_ability,
        insert_test_actor, insert_test_lore_entry, insert_test_scene,
    };

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let gm = insert_test_user(&mut conn);
    let tables = insert_test_world(&mut conn, gm);
    let scene = insert_test_scene(&mut conn, tables, gm);
    let actor = insert_test_actor(&mut conn, tables, scene, gm);
    let item = insert_test_item(&mut conn, tables, gm);
    let lore = insert_test_lore_entry(&mut conn, tables, gm);
    let ability = insert_test_ability(&mut conn, tables, gm);

    let player = insert_test_user(&mut conn);
    grant_all_content_permissions(&mut conn, player, actor, item, lore, ability, "Editor");
    let before = count_content_permissions(&mut conn, tables, player);
    assert_eq!(before, (1, 1, 1, 1), "the fixture grants one of each");

    let own_world = insert_test_world(&mut conn, player);
    let own_items = [
        insert_test_item(&mut conn, own_world, player),
        insert_test_item(&mut conn, own_world, player),
    ];
    drop(conn);
    for own in own_items {
        submit_takedown_notice_impl(&state, a_valid_notice(own))
            .await
            .expect("notice");
    }

    let standing = crate::moderation::standing::standing_of(&state, player)
        .await
        .expect("standing");
    assert!(!standing.may_publish, "the fixture must actually suspend");

    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(
        count_content_permissions(&mut conn, tables, player),
        before,
        "suspension costs publishing and nothing else (FR-019)",
    );
}

/// T061's SDL guard: the queries exist in the contracted shapes.
#[test]
fn standing_is_readable_in_the_contracted_shape() {
    let sdl = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl();

    for declaration in [
        "myStanding: Standing!",
        "accountStanding(accountId: UUID!): Standing!",
        "myNotices(limit: Int): [AccountNotice!]!",
        "mayPublish: Boolean!",
        "agesOutAt: String!",
        "entityType: ModerationEntityType!",
    ] {
        assert!(
            sdl.contains(declaration),
            "the schema must declare `{declaration}`",
        );
    }
}
