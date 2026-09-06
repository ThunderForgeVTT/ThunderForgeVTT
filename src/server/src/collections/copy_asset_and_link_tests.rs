use super::*;

/// FR-018: an actor's imagery and an item's icon travel with the copy.
///
/// Both are bare object-storage identifiers with no world scoping, so the copy
/// points at the same bytes. Asserted as *the same* asset id rather than
/// merely a present one — a copy that uploaded its own duplicate would satisfy
/// "the picture is there" and quietly double the stored bytes.
#[tokio::test]
async fn a_copy_carries_its_imagery_and_shares_the_stored_bytes() {
    use crate::schema::{world_actor_images, world_items};

    let s = source();
    let portrait = Uuid::now_v7();
    let icon = Uuid::now_v7();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actor_images::table)
            .values((
                world_actor_images::id.eq(Uuid::now_v7()),
                world_actor_images::actor_id.eq(s.actor_id),
                world_actor_images::role.eq("portrait"),
                world_actor_images::asset_id.eq(portrait),
                world_actor_images::created_by.eq(s.owner_id),
                world_actor_images::updated_by.eq(s.owner_id),
                world_actor_images::created_at.eq(now),
                world_actor_images::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("a portrait");
        diesel::update(world_items::table.filter(world_items::id.eq(s.item_id)))
            .set(world_items::icon_asset_id.eq(Some(icon)))
            .execute(&mut conn)
            .expect("an icon");
    }

    let code = share_of(
        &s,
        &[
            ("scene", s.scene_id),
            ("actor", s.actor_id),
            ("item", s.item_id),
        ],
        "A collection with pictures",
    )
    .await;
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let copied_actor = receipt
        .created
        .iter()
        .find(|c| c.member_type == "actor")
        .expect("an actor");
    let copied_item = receipt
        .created
        .iter()
        .find(|c| c.member_type == "item")
        .expect("an item");

    let mut conn = s.state.db_pool.get().expect("connection");
    let carried: Vec<(String, Uuid)> = world_actor_images::table
        .filter(world_actor_images::actor_id.eq(copied_actor.id))
        .select((world_actor_images::role, world_actor_images::asset_id))
        .load(&mut conn)
        .expect("the copy's imagery");
    assert_eq!(
        carried,
        vec![("portrait".to_string(), portrait)],
        "the portrait must travel, pointing at the same stored object"
    );

    let carried_icon: Option<Uuid> = world_items::table
        .filter(world_items::id.eq(copied_item.id))
        .select(world_items::icon_asset_id)
        .first(&mut conn)
        .expect("the copy's icon");
    assert_eq!(
        carried_icon,
        Some(icon),
        "the icon must travel, pointing at the same stored object"
    );

    assert!(
        !receipt
            .fidelity_notes
            .iter()
            .any(|n| n.contains("icon") || n.contains("portrait")),
        "nothing is lost any more, so nothing should be declared lost: {:?}",
        receipt.fidelity_notes
    );
}

/// FR-015a: a displaced actor lands in the destination world's active scene.
#[tokio::test]
async fn a_displaced_actor_lands_in_the_destination_worlds_active_scene() {
    use crate::schema::{world_actors, worlds};

    let s = source();
    // A second scene, made active. The world already has one from `source()`,
    // so "the active scene" and "whichever comes first" are different answers
    // — which is the whole point of the requirement.
    let active_scene = {
        let mut conn = s.state.db_pool.get().expect("connection");
        let scene = insert_test_scene_named(
            &mut conn,
            s.destination_world_id,
            s.recipient_id,
            "The scene they are looking at",
        );
        diesel::update(worlds::table.filter(worlds::id.eq(s.destination_world_id)))
            .set(worlds::active_scene_id.eq(Some(scene)))
            .execute(&mut conn)
            .expect("launch it");
        scene
    };

    // The actor's own scene is deliberately not in the collection.
    let code = share_of(&s, &[("actor", s.actor_id)], "An actor with no place").await;
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let copied = receipt
        .created
        .iter()
        .find(|c| c.member_type == "actor")
        .expect("an actor");

    let mut conn = s.state.db_pool.get().expect("connection");
    let landed: Uuid = world_actors::table
        .filter(world_actors::id.eq(copied.id))
        .select(world_actors::scene_id)
        .first(&mut conn)
        .expect("its scene");
    assert_eq!(
        landed, active_scene,
        "FR-015a: a displaced actor lands in the world's current scene"
    );
    assert!(
        receipt
            .fidelity_notes
            .iter()
            .any(|n| n.contains("current scene")),
        "the displacement must still be declared: {:?}",
        receipt.fidelity_notes
    );
}

/// FR-010a: an owner can find their collection's link again; a stranger cannot.
#[tokio::test]
async fn an_owner_can_retrieve_their_own_share_link_and_a_stranger_cannot() {
    use crate::graphql::mutations_collection_shares::collection_share_link_impl;

    let s = source();
    let collection = create_collection_impl(
        &s.state,
        s.owner_id,
        false,
        CreateCollectionInput {
            world_id: s.world_id,
            name: "Findable again".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");

    // Before sharing there is nothing to find, and that is not an error.
    assert!(
        collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
            .await
            .expect("no error")
            .is_none(),
        "an unshared collection has no link"
    );

    add_collection_member_impl(
        &s.state,
        s.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "lore".to_string(),
            member_id: s.lore_id,
        },
    )
    .await
    .expect("added");
    let share = create_collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
        .await
        .expect("shared");

    let found = collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
        .await
        .expect("no error")
        .expect("the owner finds their own link");
    assert_eq!(found.share_code, share.share_code);

    // FR-020: nobody else learns anything, including that it exists.
    let refused = collection_share_link_impl(&s.state, s.recipient_id, false, collection.id).await;
    assert!(refused.is_err(), "a stranger must not read the link");

    // And a revoked link stops being the active one.
    crate::graphql::mutations_collection_shares::revoke_collection_share_link_impl(
        &s.state, s.owner_id, false, share.id,
    )
    .await
    .expect("revoked");
    assert!(
        collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
            .await
            .expect("no error")
            .is_none(),
        "a revoked link is not an active link"
    );
}
