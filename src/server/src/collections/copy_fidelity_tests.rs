use super::*;

/// SC-003: every member arrives, and the copier owns them (FR-017a).
#[tokio::test]
async fn every_member_type_arrives_owned_by_the_copier() {
    let s = source();
    let code = share_of(&s, &everything(&s), "All five").await;

    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("the copy succeeds");

    assert_eq!(receipt.created.len(), 5, "all five arrived: {receipt:?}");

    let mut kinds: Vec<&str> = receipt
        .created
        .iter()
        .map(|c| c.member_type.as_str())
        .collect();
    kinds.sort();
    assert_eq!(kinds, vec!["ability", "actor", "item", "lore", "scene"]);

    // FR-017a: the copier is the owner, in every table that records one.
    let mut conn = s.state.db_pool.get().expect("connection");
    for record in &receipt.created {
        let owner: Uuid = match record.member_type.as_str() {
            "ability" => crate::schema::world_abilities::table
                .filter(crate::schema::world_abilities::id.eq(record.id))
                .select(crate::schema::world_abilities::created_by)
                .first(&mut conn)
                .expect("ability"),
            "item" => crate::schema::world_items::table
                .filter(crate::schema::world_items::id.eq(record.id))
                .select(crate::schema::world_items::created_by)
                .first(&mut conn)
                .expect("item"),
            "actor" => crate::schema::world_actors::table
                .filter(crate::schema::world_actors::id.eq(record.id))
                .select(crate::schema::world_actors::owned_by)
                .first(&mut conn)
                .expect("actor"),
            "lore" => crate::schema::world_lore_entries::table
                .filter(crate::schema::world_lore_entries::id.eq(record.id))
                .select(crate::schema::world_lore_entries::created_by)
                .first(&mut conn)
                .expect("lore"),
            "scene" => crate::schema::scenes::table
                .filter(crate::schema::scenes::scene_id.eq(record.id))
                .select(crate::schema::scenes::owner_id)
                .first(&mut conn)
                .expect("scene"),
            other => panic!("unexpected member type {other}"),
        };
        assert_eq!(
            owner, s.recipient_id,
            "the copier must own the copied {}",
            record.member_type
        );
    }
}

/// SC-004, across every member type rather than sampled: editing a copy
/// changes nothing at the source, and the reverse.
#[tokio::test]
async fn editing_a_copy_never_reaches_the_source_for_any_type() {
    use crate::schema::{scenes, world_abilities, world_actors, world_items, world_lore_entries};

    let s = source();
    let code = share_of(&s, &everything(&s), "Independence").await;

    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let mut conn = s.state.db_pool.get().expect("connection");

    for record in &receipt.created {
        match record.member_type.as_str() {
            "ability" => {
                diesel::update(world_abilities::table.filter(world_abilities::id.eq(record.id)))
                    .set(world_abilities::name.eq("Renamed copy"))
                    .execute(&mut conn)
                    .expect("rename");
                let source_name: String = world_abilities::table
                    .filter(world_abilities::id.eq(s.ability_id))
                    .select(world_abilities::name)
                    .first(&mut conn)
                    .expect("source");
                assert_ne!(source_name, "Renamed copy");
            }
            "item" => {
                diesel::update(world_items::table.filter(world_items::id.eq(record.id)))
                    .set(world_items::name.eq("Renamed copy"))
                    .execute(&mut conn)
                    .expect("rename");
                let source_name: String = world_items::table
                    .filter(world_items::id.eq(s.item_id))
                    .select(world_items::name)
                    .first(&mut conn)
                    .expect("source");
                assert_ne!(source_name, "Renamed copy");
            }
            "actor" => {
                diesel::update(world_actors::table.filter(world_actors::id.eq(record.id)))
                    .set(world_actors::label.eq("Renamed copy"))
                    .execute(&mut conn)
                    .expect("rename");
                let source_name: String = world_actors::table
                    .filter(world_actors::id.eq(s.actor_id))
                    .select(world_actors::label)
                    .first(&mut conn)
                    .expect("source");
                assert_ne!(source_name, "Renamed copy");
            }
            "lore" => {
                diesel::update(
                    world_lore_entries::table.filter(world_lore_entries::id.eq(record.id)),
                )
                .set(world_lore_entries::content.eq("Rewritten by the recipient"))
                .execute(&mut conn)
                .expect("rewrite");
                let source_content: String = world_lore_entries::table
                    .filter(world_lore_entries::id.eq(s.lore_id))
                    .select(world_lore_entries::content)
                    .first(&mut conn)
                    .expect("source");
                assert_ne!(source_content, "Rewritten by the recipient");
            }
            "scene" => {
                diesel::update(scenes::table.filter(scenes::scene_id.eq(record.id)))
                    .set(scenes::description.eq(Some("Redecorated")))
                    .execute(&mut conn)
                    .expect("redecorate");
                let source_description: Option<String> = scenes::table
                    .filter(scenes::scene_id.eq(s.scene_id))
                    .select(scenes::description)
                    .first(&mut conn)
                    .expect("source");
                assert_ne!(source_description.as_deref(), Some("Redecorated"));
            }
            other => panic!("unexpected member type {other}"),
        }
    }
}

/// FR-014: an actor that knows an included ability knows the **copy** of
/// it, not the original.
#[tokio::test]
async fn an_intra_collection_reference_points_at_the_copy() {
    use crate::schema::world_actor_abilities;

    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let name: String = crate::schema::world_abilities::table
            .filter(crate::schema::world_abilities::id.eq(s.ability_id))
            .select(crate::schema::world_abilities::name)
            .first(&mut conn)
            .expect("ability name");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actor_abilities::table)
            .values((
                world_actor_abilities::id.eq(Uuid::now_v7()),
                world_actor_abilities::actor_id.eq(s.actor_id),
                world_actor_abilities::ability_id.eq(Some(s.ability_id)),
                world_actor_abilities::ability_name_snapshot.eq(name),
                world_actor_abilities::created_at.eq(now),
                world_actor_abilities::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("the actor knows the ability");
    }

    let code = share_of(
        &s,
        &[("ability", s.ability_id), ("actor", s.actor_id)],
        "Actor and its ability",
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
        .expect("an actor was copied");
    let copied_ability = receipt
        .created
        .iter()
        .find(|c| c.member_type == "ability")
        .expect("an ability was copied");

    let mut conn = s.state.db_pool.get().expect("connection");
    let linked: Option<Uuid> = world_actor_abilities::table
        .filter(world_actor_abilities::actor_id.eq(copied_actor.id))
        .select(world_actor_abilities::ability_id)
        .first(&mut conn)
        .expect("the copied actor's ability link");

    assert_eq!(
        linked,
        Some(copied_ability.id),
        "the copy must know the copied ability, not the source's"
    );
    assert_ne!(linked, Some(s.ability_id), "and not the original");
}

/// FR-015: a reference to something outside the collection is declared,
/// never silently dropped.
#[tokio::test]
async fn a_reference_outside_the_collection_is_declared_as_a_loss() {
    use crate::schema::world_actor_abilities;

    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actor_abilities::table)
            .values((
                world_actor_abilities::id.eq(Uuid::now_v7()),
                world_actor_abilities::actor_id.eq(s.actor_id),
                world_actor_abilities::ability_id.eq(Some(s.ability_id)),
                world_actor_abilities::ability_name_snapshot.eq("Forgotten Word"),
                world_actor_abilities::created_at.eq(now),
                world_actor_abilities::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("the actor knows an ability");
    }

    // The actor goes in; the ability it knows does not.
    let code = share_of(&s, &[("actor", s.actor_id)], "Actor alone").await;

    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    assert!(
        receipt
            .fidelity_notes
            .iter()
            .any(|n| n.contains("Forgotten Word")),
        "the loss must be declared to the recipient: {:?}",
        receipt.fidelity_notes
    );
}

/// FR-017: copying twice produces two independent sets, not a merge.
/// Exercises the scene-name and lore-slug uniqueness paths, which is where
/// a second copy would otherwise collide.
#[tokio::test]
async fn copying_twice_produces_two_independent_sets() {
    let s = source();
    let code = share_of(&s, &everything(&s), "Twice").await;

    let first = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code.clone(),
        s.destination_world_id,
    )
    .await
    .expect("first copy");

    let second = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("second copy must not collide with the first");

    assert_eq!(first.created.len(), second.created.len());
    for a in &first.created {
        for b in &second.created {
            assert_ne!(a.id, b.id, "two copies, not one shared record");
        }
    }
}

/// The edge case the uniqueness handling exists for: a collection copied
/// back into the world it came from is legitimate — duplicating one's own
/// work — and must not collide with the originals.
#[tokio::test]
async fn a_collection_can_be_copied_into_its_own_world() {
    let s = source();
    let code = share_of(&s, &everything(&s), "Back home").await;

    let receipt =
        copy_shared_collection_to_world_impl(&s.state, s.owner_id, false, code, s.world_id)
            .await
            .expect("copying into the source world is legitimate");

    assert_eq!(receipt.created.len(), 5);
    for record in &receipt.created {
        assert_ne!(record.id, s.scene_id);
        assert_ne!(record.id, s.item_id);
        assert_ne!(record.id, s.ability_id);
        assert_ne!(record.id, s.actor_id);
        assert_ne!(record.id, s.lore_id);
    }
}

/// FR-016: authority in the destination is required.
#[tokio::test]
async fn a_recipient_without_authority_cannot_copy() {
    let s = source();
    let code = share_of(&s, &everything(&s), "Not yours").await;

    let stranger = {
        let mut conn = s.state.db_pool.get().expect("connection");
        insert_test_user(&mut conn)
    };

    copy_shared_collection_to_world_impl(&s.state, stranger, false, code, s.destination_world_id)
        .await
        .expect_err("a stranger must not copy into someone else's world");
}

/// FR-013 / SC-006: a revoked link stops a copy, and leaves nothing behind.
#[tokio::test]
async fn a_revoked_link_copies_nothing_at_all() {
    use crate::graphql::mutations_collection_shares::revoke_collection_share_link_impl;
    use crate::schema::world_items;

    let s = source();
    let collection = create_collection_impl(
        &s.state,
        s.owner_id,
        false,
        CreateCollectionInput {
            world_id: s.world_id,
            name: "Revoked before copying".to_string(),
            description: None,
        },
    )
    .await
    .expect("created");
    add_collection_member_impl(
        &s.state,
        s.owner_id,
        false,
        AddCollectionMemberInput {
            collection_id: collection.id,
            member_type: "item".to_string(),
            member_id: s.item_id,
        },
    )
    .await
    .expect("added");
    let share = create_collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
        .await
        .expect("shared");

    let mut conn = s.state.db_pool.get().expect("connection");
    let before: i64 = world_items::table
        .filter(world_items::world_id.eq(s.destination_world_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    drop(conn);

    revoke_collection_share_link_impl(&s.state, s.owner_id, false, share.id)
        .await
        .expect("revoked");

    copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        share.share_code,
        s.destination_world_id,
    )
    .await
    .expect_err("a revoked link must not copy");

    let mut conn = s.state.db_pool.get().expect("connection");
    let after: i64 = world_items::table
        .filter(world_items::world_id.eq(s.destination_world_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(before, after, "nothing may be left behind");
}

/// SC-008 / FR-019: copying a scene with a background adds an asset **row**
/// but no new `storage_path`. This is the assertion `dedupe.rs`'s whole
/// argument rests on.
#[tokio::test]
async fn copying_a_scene_shares_stored_bytes_rather_than_duplicating_them() {
    use crate::schema::{canvas_image_assets, scenes};

    let s = source();

    let storage_path = format!("test-collections/{}.webp", Uuid::new_v4());
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let asset_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(canvas_image_assets::table)
            .values((
                canvas_image_assets::asset_id.eq(asset_id),
                canvas_image_assets::world_id.eq(s.world_id),
                canvas_image_assets::scene_id.eq(Some(s.scene_id)),
                canvas_image_assets::owner_user_id.eq(s.owner_id),
                canvas_image_assets::storage_path.eq(&storage_path),
                canvas_image_assets::original_format.eq("webp"),
                canvas_image_assets::width_px.eq(1024),
                canvas_image_assets::height_px.eq(768),
                canvas_image_assets::byte_size.eq(123_456i64),
                canvas_image_assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Background),
                canvas_image_assets::created_by.eq(s.owner_id),
                canvas_image_assets::updated_by.eq(s.owner_id),
                canvas_image_assets::created_at.eq(now),
                canvas_image_assets::updated_at.eq(now),
                // Lowercase hex SHA-256 or nothing: the table's check
                // constraint enforces the shape, so a short stand-in is
                // rejected outright.
                canvas_image_assets::content_hash.eq(Some("de".repeat(32))),
            ))
            .execute(&mut conn)
            .expect("insert the background asset");

        diesel::update(scenes::table.filter(scenes::scene_id.eq(s.scene_id)))
            .set((
                scenes::background_asset_id.eq(Some(asset_id)),
                scenes::background_image_path.eq(Some("/canvas/background.webp")),
            ))
            .execute(&mut conn)
            .expect("attach the background");
    }

    let distinct_paths_before: i64 = {
        let mut conn = s.state.db_pool.get().expect("connection");
        canvas_image_assets::table
            .select(diesel::dsl::count(canvas_image_assets::storage_path).aggregate_distinct())
            .first(&mut conn)
            .expect("count distinct paths")
    };

    let code = share_of(&s, &[("scene", s.scene_id)], "A place with a floor").await;
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let mut conn = s.state.db_pool.get().expect("connection");

    let distinct_paths_after: i64 = canvas_image_assets::table
        .select(diesel::dsl::count(canvas_image_assets::storage_path).aggregate_distinct())
        .first(&mut conn)
        .expect("count distinct paths");
    assert_eq!(
        distinct_paths_before, distinct_paths_after,
        "copying a scene must add no new stored object (SC-008)"
    );

    let rows_on_that_path: i64 = canvas_image_assets::table
        .filter(canvas_image_assets::storage_path.eq(&storage_path))
        .count()
        .get_result(&mut conn)
        .expect("count rows");
    assert_eq!(
        rows_on_that_path, 2,
        "a second row on the same path is exactly what dedupe.rs describes"
    );

    // SC-008a: the copied scene renders with its background.
    let copied_scene = receipt
        .created
        .iter()
        .find(|c| c.member_type == "scene")
        .expect("a scene was copied");
    let (background_asset, background_path): (Option<Uuid>, Option<String>) = scenes::table
        .filter(scenes::scene_id.eq(copied_scene.id))
        .select((scenes::background_asset_id, scenes::background_image_path))
        .first(&mut conn)
        .expect("the copied scene");
    assert!(background_asset.is_some(), "the copy has its own asset row");
    assert!(background_path.is_some(), "and its background path");
}

/// SC-008a: walls and lighting come with the place.
#[tokio::test]
async fn a_copied_scene_brings_its_walls_and_lighting() {
    use crate::schema::{light_sources, walls};

    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(walls::table)
            .values((
                walls::wall_id.eq(Uuid::now_v7()),
                walls::scene_id.eq(s.scene_id),
                walls::x1.eq(0.0f64),
                walls::y1.eq(0.0f64),
                walls::x2.eq(10.0f64),
                walls::y2.eq(0.0f64),
                walls::blocks_vision.eq(true),
                walls::blocks_movement.eq(true),
                walls::created_by.eq(s.owner_id),
                walls::updated_by.eq(s.owner_id),
                walls::created_at.eq(now),
                walls::updated_at.eq(now),
                walls::door_state.eq("none"),
                walls::locked.eq(false),
                walls::secret.eq(false),
            ))
            .execute(&mut conn)
            .expect("a wall");

        diesel::insert_into(light_sources::table)
            .values((
                light_sources::light_id.eq(Uuid::now_v7()),
                light_sources::scene_id.eq(s.scene_id),
                light_sources::x.eq(5.0f64),
                light_sources::y.eq(5.0f64),
                light_sources::radius.eq(30.0f64),
                light_sources::intensity.eq(0.8f64),
                light_sources::casts_shadows.eq(true),
                light_sources::created_by.eq(s.owner_id),
                light_sources::updated_by.eq(s.owner_id),
                light_sources::created_at.eq(now),
                light_sources::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("a light");
    }

    let code = share_of(&s, &[("scene", s.scene_id)], "A lit, walled room").await;
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let copied_scene = receipt
        .created
        .iter()
        .find(|c| c.member_type == "scene")
        .expect("a scene");

    let mut conn = s.state.db_pool.get().expect("connection");
    let wall_count: i64 = walls::table
        .filter(walls::scene_id.eq(copied_scene.id))
        .count()
        .get_result(&mut conn)
        .expect("walls");
    let light_count: i64 = light_sources::table
        .filter(light_sources::scene_id.eq(copied_scene.id))
        .count()
        .get_result(&mut conn)
        .expect("lights");

    assert_eq!(wall_count, 1, "walls come with the place (SC-008a)");
    assert_eq!(light_count, 1, "lighting comes with the place (SC-008a)");
}

/// Tokens are a game in progress, not a place. They stay behind, and the
/// recipient is told rather than left to notice.
#[tokio::test]
async fn tokens_stay_behind_and_the_omission_is_declared() {
    use crate::schema::tokens;

    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(Uuid::now_v7()),
                tokens::scene_id.eq(s.scene_id),
                tokens::x.eq(1.0f64),
                tokens::y.eq(1.0f64),
                tokens::rotation.eq(0.0f64),
                tokens::scale.eq(1.0f64),
                tokens::owner_user_id.eq(Some(s.owner_id)),
                tokens::is_primary.eq(false),
                tokens::token_type.eq("npc"),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("a token");
    }

    let code = share_of(&s, &[("scene", s.scene_id)], "A room mid-game").await;
    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("copied");

    let copied_scene = receipt
        .created
        .iter()
        .find(|c| c.member_type == "scene")
        .expect("a scene");

    let mut conn = s.state.db_pool.get().expect("connection");
    let token_count: i64 = tokens::table
        .filter(tokens::scene_id.eq(copied_scene.id))
        .count()
        .get_result(&mut conn)
        .expect("tokens");
    assert_eq!(token_count, 0, "tokens are not part of the place");

    // The note has to say they were *not* copied, not merely mention tokens.
    //
    // T052's mutation pass caught this: `contains("token")` was satisfied by a
    // note reading "brought its 3 tokens along", which declares the opposite of
    // what happened. The behaviour assertion above still failed on the real
    // mutation, so the test was never wrong — but half of it was measuring
    // nothing, and a declared loss that declares an arrival is exactly the
    // failure FR-015 exists to prevent.
    assert!(
        receipt
            .fidelity_notes
            .iter()
            .any(|n| n.contains("token") && n.contains("not copied")),
        "the omission must be declared as an omission: {:?}",
        receipt.fidelity_notes
    );
}
