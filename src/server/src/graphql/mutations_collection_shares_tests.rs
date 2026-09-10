//! Tests for `mutations_collection_shares`.
//!
//! Moved out of that file when spec 039's attestation tests took it past the
//! 1000-line gate. Pure movement — the module is still `mod tests` and still
//! reaches its parent through `use super::*`.

use super::*;

use super::*;
use crate::graphql::mutations_collection_shares::publishing_gate::{
    InstancePublishing, publishable_instance,
};
use crate::graphql::mutations_collections::{
    AddCollectionMemberInput, CreateCollectionInput, add_collection_member_impl,
    create_collection_impl, delete_collection_impl,
};
use crate::publishing::an_agreement;
use crate::test_support::*;

struct Fixture {
    /// Every test in this module mints a share link, and spec 040 FR-026
    /// refuses that on an instance with no notice contact. Held for the
    /// length of the test, and for the length of the test *only*, so a
    /// readiness test asserting the opposite is not racing us.
    _publishing: InstancePublishing,
    state: AppState,
    owner_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    item_id: Uuid,
    ability_id: Uuid,
    world_name: String,
}

fn fixture() -> Fixture {
    let _publishing = publishable_instance();
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let item_id = insert_test_item(&mut conn, world_id, owner_id);
    let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

    let world_name: String = crate::schema::worlds::table
        .filter(crate::schema::worlds::id.eq(world_id))
        .select(crate::schema::worlds::name)
        .first(&mut conn)
        .expect("world name");

    Fixture {
        _publishing,
        state,
        owner_id,
        world_id,
        scene_id,
        item_id,
        ability_id,
        world_name,
    }
}

/// Deliberately **not** holding `publishable_instance()`.
///
/// `fixture()` takes that lock itself and drops it on return, and it is a
/// non-reentrant `std::sync::Mutex` shared with every settings test in the
/// crate. Holding it across a whole test — a fixture, a collection, a
/// member and a share, each with `.await` in it — serialised the entire
/// suite behind this one test and took a 0.7-second module past a
/// ten-minute timeout. The gate is satisfied by the seeded notice-contact
/// rows, which is how the sibling tests below manage it too.
///
/// Spec 039 FR-012/FR-013: a version this instance does not know refuses,
/// and **no share row is left behind**.
///
/// The second half is the one worth driving rather than reasoning about. The
/// gate runs before the insert, so a refusal cannot leave a link — but "runs
/// before" is an ordering somebody can change while all the other tests
/// still pass, and the symptom would be a live share link nobody agreed to.
#[tokio::test]
async fn an_unknown_version_refuses_and_mints_nothing() {
    use crate::schema::world_collection_shares;

    let f = fixture();
    let collection = create_collection_impl(
        &f.state,
        f.owner_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: "Unattested".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");
    add_collection_member_impl(
        &f.state,
        f.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "item".to_string(),
            member_id: f.item_id,
        },
    )
    .await
    .expect("added");

    // The current version, archived — and then a different identity offered.
    let real = an_agreement(&f.state).await;
    let refusal = create_collection_share_link_impl(
        &f.state,
        f.owner_id,
        false,
        collection.id,
        &crate::publishing::AttestationInput {
            terms_version_id: "sharing-terms@ffffffffffffffff".to_string(),
        },
    )
    .await
    .expect_err("a version this instance never archived must be refused");

    let message = refusal.message;
    assert!(
        !message.contains(&real.terms_version_id),
        "a refusal must not name a valid version identity (FR-013): {message}",
    );
    assert!(
        !message.contains('@'),
        "nor anything shaped like one: {message}",
    );

    let mut conn = f.state.db_pool.get().expect("conn");
    let links: i64 = world_collection_shares::table
        .filter(world_collection_shares::collection_id.eq(collection.id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(
        links, 0,
        "a refused publish must leave no share link — the gate runs before \
         the insert, and this is what notices if that ordering changes",
    );
}

/// The other side of the same act: a publish that succeeds leaves exactly
/// one agreement, pointing at the link it authorised.
///
/// One transaction, so this is really asserting that both halves committed —
/// the case where the share exists and the record does not is the one the
/// evidence cannot survive.
#[tokio::test]
async fn a_successful_publish_leaves_one_agreement_naming_its_link() {
    let f = fixture();
    let (collection_id, share) = shared_fixture(&f).await;

    let recorded = crate::attestation::for_content(
        &f.state,
        crate::attestation::CoveredKind::Collection,
        collection_id,
    )
    .await
    .expect("read the agreements");

    assert_eq!(recorded.len(), 1, "one publish, one agreement");
    let agreement = &recorded[0];
    assert_eq!(agreement.share_id, Some(share.id), "and it names the link");
    assert_eq!(agreement.world_id, Some(f.world_id));
    assert_eq!(
        agreement.terms_version_id,
        crate::legal::sharing_terms().version_id,
    );
    assert_eq!(
        agreement.subject_user_id, f.owner_id,
        "the person who published, not the world's owner or the instance",
    );
}

/// A collection holding an item, an ability and a scene, already shared.
async fn shared_fixture(f: &Fixture) -> (Uuid, CollectionShare) {
    let collection = create_collection_impl(
        &f.state,
        f.owner_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: "The Haunted Manor".to_string(),
            description: Some("Rooms, curses and the reasons why.".to_string()),
        },
    )
    .await
    .expect("created");

    for (member_type, member_id) in [
        ("item", f.item_id),
        ("ability", f.ability_id),
        ("scene", f.scene_id),
    ] {
        add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: member_type.to_string(),
                member_id,
            },
        )
        .await
        .expect("added");
    }

    let share = create_collection_share_link_impl(
        &f.state,
        f.owner_id,
        false,
        collection.id,
        &an_agreement(&f.state).await,
    )
    .await
    .expect("shared");
    (collection.id, share)
}

/// A rate-limit bucket nothing else in the process shares, so one test
/// exhausting its budget cannot fail another.
fn a_caller() -> String {
    format!("test-{}", Uuid::new_v4())
}

/// FR-008: the code is 20 uppercase hex characters, v4-derived.
#[tokio::test]
async fn a_share_link_carries_an_unguessable_code() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;
    assert_eq!(share.share_code.len(), 20);
    assert_eq!(share.share_code, share.share_code.to_uppercase());
    assert!(!share.revoked);
}

/// FR-009a: no session. This is the whole of ADR-070 in one assertion —
/// `shared_collection_impl` takes no user and asks for none.
#[tokio::test]
async fn a_collection_previews_without_any_account() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect("an anonymous caller may read a shared collection");

    assert_eq!(preview.name, "The Haunted Manor");
    assert_eq!(preview.members.len(), 3);
    assert_eq!(preview.withheld_count, 0);
}

/// US4 scenario 1: how many of each kind, before copying.
#[tokio::test]
async fn the_preview_says_how_many_of_each_kind() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect("preview");

    let mut kinds: Vec<(String, i32)> = preview
        .counts_by_type
        .into_iter()
        .map(|c| (c.member_type, c.count))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        vec![
            ("ability".to_string(), 1),
            ("item".to_string(), 1),
            ("scene".to_string(), 1),
        ]
    );
}

/// FR-009 / SC-007a: the preview reveals nothing about the source world.
///
/// Serialised and searched rather than field-by-field, so a field added
/// later that happens to carry a world identifier is caught by a test
/// nobody remembered to update.
#[tokio::test]
async fn the_preview_reveals_nothing_about_the_source_world() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect("preview");

    let rendered = format!("{preview:?}");
    assert!(
        !rendered.contains(&f.world_id.to_string()),
        "the world id must not appear in the preview: {rendered}"
    );
    assert!(
        !rendered.contains(&f.world_name),
        "the world name must not appear in the preview: {rendered}"
    );
    assert!(
        !rendered.contains(&f.owner_id.to_string()),
        "the owner's id must not appear in the preview: {rendered}"
    );
}

/// FR-010 + FR-009d: revoking makes the link unavailable, and the sentence
/// is the same one an unknown code produces.
#[tokio::test]
async fn a_revoked_share_is_indistinguishable_from_a_code_that_never_existed() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    shared_collection_impl(&f.state, &a_caller(), share.share_code.clone())
        .await
        .expect("works before revocation");

    assert!(
        revoke_collection_share_link_impl(&f.state, f.owner_id, false, share.id)
            .await
            .expect("revoked")
    );

    let revoked_error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect_err("a revoked link must not resolve");
    let unknown_error =
        shared_collection_impl(&f.state, &a_caller(), "NOTAREALCODEATALL0".to_string())
            .await
            .expect_err("an unknown code must not resolve");

    assert_eq!(
        revoked_error.message, unknown_error.message,
        "a revoked share and an unknown code must be indistinguishable (FR-009d)"
    );
    assert_eq!(revoked_error.message, UNAVAILABLE);
}

/// US2 scenario 4 + FR-009d: deleting the collection behaves as revoked,
/// and is likewise indistinguishable.
#[tokio::test]
async fn a_deleted_collection_reads_the_same_as_an_unknown_code() {
    let f = fixture();
    let (collection_id, share) = shared_fixture(&f).await;

    delete_collection_impl(&f.state, f.owner_id, false, collection_id)
        .await
        .expect("deleted");

    let error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect_err("a deleted collection's link must not resolve");
    assert_eq!(error.message, UNAVAILABLE);
}

/// FR-011: a revoked share leaves the collection and its artifacts alone.
/// Revocation ends the link, not the content.
#[tokio::test]
async fn revoking_touches_neither_the_collection_nor_its_artifacts() {
    use crate::schema::world_items;

    let f = fixture();
    let (collection_id, share) = shared_fixture(&f).await;

    revoke_collection_share_link_impl(&f.state, f.owner_id, false, share.id)
        .await
        .expect("revoked");

    let mut conn = f.state.db_pool.get().expect("connection");
    let collection_survives: i64 = world_collections::table
        .filter(world_collections::id.eq(collection_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(collection_survives, 1);

    let item_survives: i64 = world_items::table
        .filter(world_items::id.eq(f.item_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(item_survives, 1);
}

/// A collection may be shared, revoked and shared again with a new code —
/// which is why the share is a separate row from the collection.
#[tokio::test]
async fn a_collection_can_be_shared_again_after_revocation() {
    let f = fixture();
    let (collection_id, first) = shared_fixture(&f).await;

    revoke_collection_share_link_impl(&f.state, f.owner_id, false, first.id)
        .await
        .expect("revoked");

    let second = create_collection_share_link_impl(
        &f.state,
        f.owner_id,
        false,
        collection_id,
        &an_agreement(&f.state).await,
    )
    .await
    .expect("shared again");

    assert_ne!(first.share_code, second.share_code, "a new code each time");
    shared_collection_impl(&f.state, &a_caller(), second.share_code)
        .await
        .expect("the new link works");
    shared_collection_impl(&f.state, &a_caller(), first.share_code)
        .await
        .expect_err("the old link stays dead");
}

/// FR-001b at share time: a member restricted after being added blocks the
/// share, and says which one.
#[tokio::test]
async fn a_member_restricted_after_being_added_blocks_a_new_share() {
    use crate::schema::world_abilities;

    let f = fixture();
    let collection = create_collection_impl(
        &f.state,
        f.owner_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: "Restricted later".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");

    add_collection_member_impl(
        &f.state,
        f.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "ability".to_string(),
            member_id: f.ability_id,
        },
    )
    .await
    .expect("added while unrestricted");

    let mut conn = f.state.db_pool.get().expect("connection");
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
        .set(world_abilities::gm_only.eq(true))
        .execute(&mut conn)
        .expect("restrict it");
    drop(conn);

    let error = create_collection_share_link_impl(
        &f.state,
        f.owner_id,
        false,
        collection.id,
        &an_agreement(&f.state).await,
    )
    .await
    .expect_err("a restricted member must block the share");
    assert!(
        error.message.contains("Game Master"),
        "the refusal must name the reason, got: {}",
        error.message
    );
}

/// FR-024: every member withheld reports that nothing is available, rather
/// than presenting an empty collection as complete.
#[tokio::test]
async fn a_collection_whose_members_all_vanished_says_nothing_is_available() {
    use crate::schema::{scenes, world_abilities, world_items};

    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    let mut conn = f.state.db_pool.get().expect("connection");
    diesel::delete(world_items::table.filter(world_items::id.eq(f.item_id)))
        .execute(&mut conn)
        .expect("delete the item");
    diesel::delete(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
        .execute(&mut conn)
        .expect("delete the ability");
    diesel::delete(scenes::table.filter(scenes::scene_id.eq(f.scene_id)))
        .execute(&mut conn)
        .expect("delete the scene");
    drop(conn);

    let error = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect_err("an entirely withheld collection must not read as complete");
    assert!(error.message.contains("Nothing"), "got: {}", error.message);
}

/// FR-022: one member gone leaves the rest, and the absence is a count
/// rather than a name.
#[tokio::test]
async fn one_missing_member_is_counted_never_named() {
    use crate::schema::world_items;

    let f = fixture();
    let (_, share) = shared_fixture(&f).await;

    let item_name: String = {
        let mut conn = f.state.db_pool.get().expect("connection");
        let name = world_items::table
            .filter(world_items::id.eq(f.item_id))
            .select(world_items::name)
            .first::<String>(&mut conn)
            .expect("item name");
        diesel::delete(world_items::table.filter(world_items::id.eq(f.item_id)))
            .execute(&mut conn)
            .expect("delete the item");
        name
    };

    let preview = shared_collection_impl(&f.state, &a_caller(), share.share_code)
        .await
        .expect("the collection still opens");

    assert_eq!(preview.members.len(), 2, "the rest are still there");
    assert_eq!(preview.withheld_count, 1);
    assert!(
        !format!("{preview:?}").contains(&item_name),
        "the withheld member must not be named"
    );
}

/// FR-009c: the anonymous read is rate limited, and the refusal reveals
/// nothing about the code that was tried.
#[tokio::test]
async fn the_anonymous_read_is_rate_limited() {
    let f = fixture();
    let (_, share) = shared_fixture(&f).await;
    let caller = a_caller();

    let mut refused = None;
    for _ in 0..200 {
        if let Err(e) = shared_collection_impl(&f.state, &caller, share.share_code.clone()).await {
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
        "the rate-limit refusal must not masquerade as a bad code"
    );
}

/// An empty collection cannot be shared. A link to nothing is a link that
/// reads as broken.
#[tokio::test]
async fn an_empty_collection_cannot_be_shared() {
    let f = fixture();
    let collection = create_collection_impl(
        &f.state,
        f.owner_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: "Empty".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");

    create_collection_share_link_impl(
        &f.state,
        f.owner_id,
        false,
        collection.id,
        &an_agreement(&f.state).await,
    )
    .await
    .expect_err("an empty collection must not be shareable");
}
