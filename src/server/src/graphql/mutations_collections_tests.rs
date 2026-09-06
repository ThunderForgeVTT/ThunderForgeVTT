use super::*;
use crate::test_support::*;

struct Fixture {
    state: AppState,
    owner_id: Uuid,
    player_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    actor_id: Uuid,
    item_id: Uuid,
    ability_id: Uuid,
    lore_id: Uuid,
}

/// One world with one of every member type, plus a non-DM member.
fn fixture() -> Fixture {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("connection");

    let owner_id = insert_test_user(&mut conn);
    let player_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");

    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let item_id = insert_test_item(&mut conn, world_id, owner_id);
    let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
    let lore_id = insert_test_lore_entry(&mut conn, world_id, owner_id);

    Fixture {
        state,
        owner_id,
        player_id,
        world_id,
        scene_id,
        actor_id,
        item_id,
        ability_id,
        lore_id,
    }
}

async fn a_collection(f: &Fixture, name: &str) -> Collection {
    create_collection_impl(
        &f.state,
        f.owner_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: name.to_string(),
            description: None,
        },
    )
    .await
    .expect("the DM may create a collection")
}

/// FR-001: a DM creates; a Player does not.
#[tokio::test]
async fn only_a_dm_may_create_a_collection() {
    let f = fixture();

    create_collection_impl(
        &f.state,
        f.player_id,
        false,
        CreateCollectionInput {
            world_id: f.world_id,
            name: "Player's attempt".to_string(),
            description: None,
        },
    )
    .await
    .expect_err("a Player must not create a collection");

    let collection = a_collection(&f, "The Haunted Manor").await;
    assert_eq!(collection.name, "The Haunted Manor");
    assert_eq!(collection.created_by, f.owner_id);
    assert_eq!(collection.updated_by, f.owner_id);
}

/// FR-002: all five member types go in.
#[tokio::test]
async fn a_collection_holds_every_member_type() {
    let f = fixture();
    let collection = a_collection(&f, "Everything").await;

    for (member_type, member_id) in [
        ("actor", f.actor_id),
        ("item", f.item_id),
        ("ability", f.ability_id),
        ("lore", f.lore_id),
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
        .unwrap_or_else(|e| panic!("{member_type} must be addable: {e:?}"));
    }

    let members = collection_members_impl(&f.state, f.owner_id, false, collection.id)
        .await
        .expect("members list");
    assert_eq!(members.len(), 5, "the collection lists exactly its members");
}

/// FR-003: a collection holds artifacts from its own world only.
#[tokio::test]
async fn an_artifact_from_another_world_is_refused() {
    let f = fixture();
    let mut conn = f.state.db_pool.get().expect("connection");
    let other_world = insert_test_world(&mut conn, f.owner_id);
    let foreign_item = insert_test_item(&mut conn, other_world, f.owner_id);
    drop(conn);

    let collection = a_collection(&f, "Cross-world attempt").await;

    let error = add_collection_member_impl(
        &f.state,
        f.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "item".to_string(),
            member_id: foreign_item,
        },
    )
    .await
    .expect_err("a foreign artifact must be refused");
    assert!(
        error.message.contains("own world"),
        "the refusal must say why, got: {}",
        error.message
    );
}

/// FR-001a: a GM-only ability is refused, and the refusal explains itself
/// rather than reading as a generic failure.
#[tokio::test]
async fn a_restricted_artifact_is_refused_with_its_reason() {
    use crate::schema::world_abilities;

    let f = fixture();
    let mut conn = f.state.db_pool.get().expect("connection");
    diesel::update(world_abilities::table.filter(world_abilities::id.eq(f.ability_id)))
        .set(world_abilities::gm_only.eq(true))
        .execute(&mut conn)
        .expect("hide the ability");
    drop(conn);

    let collection = a_collection(&f, "Restricted attempt").await;

    let error = add_collection_member_impl(
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
    .expect_err("a GM-only ability must be refused");
    assert!(
        error.message.contains("Game Master"),
        "the refusal must carry the reason, got: {}",
        error.message
    );
}

/// FR-004: removing a member deletes the membership, never the artifact.
#[tokio::test]
async fn removing_a_member_leaves_the_artifact_alone() {
    use crate::schema::world_items;

    let f = fixture();
    let collection = a_collection(&f, "Removable").await;

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

    assert!(
        remove_collection_member_impl(&f.state, f.owner_id, false, collection.id, f.item_id)
            .await
            .expect("removed")
    );

    let mut conn = f.state.db_pool.get().expect("connection");
    let still_there: i64 = world_items::table
        .filter(world_items::id.eq(f.item_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(still_there, 1, "the artifact must survive being removed");
}

/// FR-005a: the 101st member is refused, with the limit named.
///
/// Uses the same item repeatedly? No — the unique constraint forbids it,
/// so this inserts real rows. It is slower and it is the only way to
/// exercise the count the limit actually reads.
#[tokio::test]
async fn the_hundred_and_first_member_is_refused_with_the_limit_named() {
    let f = fixture();
    let collection = a_collection(&f, "At the limit").await;

    let mut conn = f.state.db_pool.get().expect("connection");
    let mut ids = Vec::new();
    for _ in 0..(MAX_MEMBERS + 1) {
        ids.push(insert_test_item(&mut conn, f.world_id, f.owner_id));
    }
    drop(conn);

    for (index, item_id) in ids.iter().enumerate().take(MAX_MEMBERS as usize) {
        add_collection_member_impl(
            &f.state,
            f.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: *item_id,
            },
        )
        .await
        .unwrap_or_else(|e| panic!("member {index} must be addable: {e:?}"));
    }

    let error = add_collection_member_impl(
        &f.state,
        f.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "item".to_string(),
            member_id: ids[MAX_MEMBERS as usize],
        },
    )
    .await
    .expect_err("the 101st must be refused");
    assert!(
        error.message.contains(&MAX_MEMBERS.to_string()),
        "the refusal must name the limit, got: {}",
        error.message
    );
}

/// Adding the same artifact twice is refused rather than duplicated.
#[tokio::test]
async fn the_same_artifact_cannot_be_added_twice() {
    let f = fixture();
    let collection = a_collection(&f, "No duplicates").await;

    let input = AddCollectionMemberInput {
        collection_id: collection.id,
        member_type: "item".to_string(),
        member_id: f.item_id,
    };

    add_collection_member_impl(&f.state, f.owner_id, false, input.clone())
        .await
        .expect("first add");
    let error = add_collection_member_impl(&f.state, f.owner_id, false, input)
        .await
        .expect_err("second add must be refused");
    assert!(error.message.contains("already"), "got: {}", error.message);
}

/// FR-020: a caller with no authority over the world learns nothing —
/// including whether the collection exists. The refusal is "not found",
/// not "not permitted", because the second answer is itself information.
#[tokio::test]
async fn a_non_dm_cannot_see_or_touch_a_collection() {
    let f = fixture();
    let collection = a_collection(&f, "Private").await;

    let error = collection_members_impl(&f.state, f.player_id, false, collection.id)
        .await
        .expect_err("a Player must not read a collection's members");
    assert!(
        error.message.contains("not found"),
        "the refusal must not confirm the collection exists, got: {}",
        error.message
    );

    world_collections_impl(&f.state, f.player_id, false, f.world_id)
        .await
        .expect_err("a Player must not list the world's collections");
}

/// US2 scenario 4: deleting a collection deletes no artifacts.
#[tokio::test]
async fn deleting_a_collection_deletes_no_artifacts() {
    use crate::schema::world_items;

    let f = fixture();
    let collection = a_collection(&f, "Doomed").await;
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

    assert!(
        delete_collection_impl(&f.state, f.owner_id, false, collection.id)
            .await
            .expect("deleted")
    );

    let mut conn = f.state.db_pool.get().expect("connection");
    let item_survives: i64 = world_items::table
        .filter(world_items::id.eq(f.item_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(item_survives, 1, "the artifact must survive the collection");
}

/// Every name `apps/web/src/api/collections.ts` sends, asserted against the
/// schema the server actually publishes.
///
/// The guard `mutations_lore_tree.rs` and `mutations_party.rs` keep, for the
/// same reason and one more. Theirs is that a mutation can compile without
/// ever being merged into the root, and then fails for the first Game Master
/// who tries it rather than for the suite.
///
/// The extra reason here is that the client half of this feature was written
/// by reading these Rust signatures and transcribing them into GraphQL
/// documents by hand. `member_count` becomes `memberCount`, `counts_by_type`
/// becomes `countsByType`, and a transcription slip in either direction
/// produces a query the server rejects at runtime and nothing rejects before
/// then. This test is where that transcription is checked.
#[test]
fn the_collection_surface_is_reachable_under_the_names_the_client_uses() {
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish();
    let sdl = schema.sdl();

    for name in [
        "worldCollections(",
        "collectionMembers(",
        "createCollection(",
        "updateCollection(",
        "deleteCollection(",
        "addCollectionMember(",
        "removeCollectionMember(",
        "createCollectionShareLink(",
        "revokeCollectionShareLink(",
        "copySharedCollectionToWorld(",
        "sharedCollection(",
        "collectionShareLink(",
    ] {
        assert!(sdl.contains(name), "{name} must be reachable from the root");
    }

    // The field names, camel-cased by async-graphql, that the client's
    // selection sets ask for by hand.
    for field in [
        "memberCount: Int!",
        "memberType: String!",
        "shareCode: String!",
        "countsByType: [CollectionTypeCount!]!",
        "withheldCount: Int!",
        "fidelityNotes: [String!]!",
    ] {
        assert!(sdl.contains(field), "the client selects `{field}`");
    }

    // The copy mutation takes two plain arguments rather than an input object,
    // unlike `copySharedAbilityToWorld`. The client has to match whichever it
    // is, so the shape is asserted and not assumed.
    assert!(
        sdl.contains("copySharedCollectionToWorld(shareCode: String!, destinationWorldId: UUID!)"),
        "the copy mutation's argument shape is part of the contract:\n{}",
        sdl.lines()
            .find(|l| l.contains("copySharedCollectionToWorld"))
            .unwrap_or("<not found>")
    );
}
