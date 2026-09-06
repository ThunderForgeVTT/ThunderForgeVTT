use super::*;
use crate::graphql::mutations_collection_shares::create_collection_share_link_impl;
use crate::graphql::mutations_collections::{
    AddCollectionMemberInput, CreateCollectionInput, add_collection_member_impl,
    create_collection_impl,
};
use crate::test_support::*;
use diesel::expression_methods::AggregateExpressionMethods;

struct Source {
    state: AppState,
    owner_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    actor_id: Uuid,
    item_id: Uuid,
    ability_id: Uuid,
    lore_id: Uuid,
    recipient_id: Uuid,
    destination_world_id: Uuid,
}

/// A source world with one of each type, plus a recipient with a world of
/// their own to copy into.
fn source() -> Source {
    dotenvy::dotenv().ok();
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("connection");

    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let item_id = insert_test_item(&mut conn, world_id, owner_id);
    let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
    let lore_id = insert_test_lore_entry(&mut conn, world_id, owner_id);

    let recipient_id = insert_test_user(&mut conn);
    let destination_world_id = insert_test_world(&mut conn, recipient_id);
    // The destination needs a scene for an actor to live in when the
    // collection does not bring one.
    insert_test_scene(&mut conn, destination_world_id, recipient_id);

    Source {
        state,
        owner_id,
        world_id,
        scene_id,
        actor_id,
        item_id,
        ability_id,
        lore_id,
        recipient_id,
        destination_world_id,
    }
}

/// Gather the named types into a collection and share it.
async fn share_of(s: &Source, types: &[(&str, Uuid)], name: &str) -> String {
    let collection = create_collection_impl(
        &s.state,
        s.owner_id,
        false,
        CreateCollectionInput {
            world_id: s.world_id,
            name: name.to_string(),
            description: None,
        },
    )
    .await
    .expect("created");

    for (member_type, member_id) in types {
        add_collection_member_impl(
            &s.state,
            s.owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: (*member_type).to_string(),
                member_id: *member_id,
            },
        )
        .await
        .unwrap_or_else(|e| panic!("{member_type} must be addable: {e:?}"));
    }

    create_collection_share_link_impl(&s.state, s.owner_id, false, collection.id)
        .await
        .expect("shared")
        .share_code
}

fn everything(s: &Source) -> Vec<(&'static str, Uuid)> {
    vec![
        ("ability", s.ability_id),
        ("item", s.item_id),
        ("scene", s.scene_id),
        ("actor", s.actor_id),
        ("lore", s.lore_id),
    ]
}

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

/// Record a takedown against one artifact, the way spec 015 does.
///
/// Written directly rather than through `submit_takedown_notice_impl` because
/// these tests are about what a *collection* does with a disabled member, not
/// about notice validation — and going through the notice path would make them
/// fail for reasons that belong to spec 015.
fn disable(conn: &mut PgConnection, entity_type: &str, entity_id: Uuid, world_id: Uuid) -> Uuid {
    use crate::schema::content_moderation_actions;
    let case_id = Uuid::now_v7();
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::id.eq(Uuid::now_v7()),
            content_moderation_actions::case_id.eq(case_id),
            content_moderation_actions::action_type
                .eq(crate::moderation::action_type::CONTENT_DISABLED),
            content_moderation_actions::entity_type.eq(entity_type),
            content_moderation_actions::entity_id.eq(entity_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::claimant_name.eq("A Rights Holder"),
            content_moderation_actions::claimant_contact.eq("rights@example.test"),
            content_moderation_actions::copyrighted_work_description.eq("The work"),
            content_moderation_actions::infringing_material_location.eq("here"),
            content_moderation_actions::good_faith_statement.eq(true),
            content_moderation_actions::accuracy_statement.eq(true),
            content_moderation_actions::signature.eq("A Rights Holder"),
            content_moderation_actions::created_at.eq(chrono::Utc::now()),
        ))
        .execute(conn)
        .expect("record the takedown");
    case_id
}

/// Undo one, the way a reversed takedown does.
fn restore(
    conn: &mut PgConnection,
    case_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    world_id: Uuid,
) {
    use crate::schema::content_moderation_actions;
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::id.eq(Uuid::now_v7()),
            content_moderation_actions::case_id.eq(case_id),
            content_moderation_actions::action_type
                .eq(crate::moderation::action_type::CONTENT_RESTORED),
            content_moderation_actions::entity_type.eq(entity_type),
            content_moderation_actions::entity_id.eq(entity_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::claimant_name.eq("A Rights Holder"),
            content_moderation_actions::claimant_contact.eq("rights@example.test"),
            content_moderation_actions::copyrighted_work_description.eq("The work"),
            content_moderation_actions::infringing_material_location.eq("here"),
            content_moderation_actions::good_faith_statement.eq(true),
            content_moderation_actions::accuracy_statement.eq(true),
            content_moderation_actions::signature.eq("A Rights Holder"),
            content_moderation_actions::created_at.eq(chrono::Utc::now()),
        ))
        .execute(conn)
        .expect("record the restoration");
}

/// T039, FR-021/FR-023: a takedown reaches the copy path, not just the preview.
///
/// Asserted on the copy rather than inferred from the preview sharing a
/// resolver with it. The two call `resolve_member` independently, and "the
/// preview hides it" is not the claim FR-021 makes — the claim is that a copy
/// taken afterwards does not create it.
#[tokio::test]
async fn a_moderated_member_is_withheld_from_the_copy_and_the_rest_still_arrive() {
    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        disable(&mut conn, "world_item", s.item_id, s.world_id);
    }

    let code = share_of(
        &s,
        &[
            ("item", s.item_id),
            ("ability", s.ability_id),
            ("lore", s.lore_id),
        ],
        "One taken down, two not",
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
    .expect("the collection still copies");

    assert!(
        !receipt.created.iter().any(|c| c.member_type == "item"),
        "FR-021: a disabled member must not be created by a copy: {:?}",
        receipt.created
    );
    assert_eq!(
        receipt.created.len(),
        2,
        "FR-023: disabling one member must not withhold the others: {:?}",
        receipt.created
    );

    // FR-022: the absence is declared, and the artifact is never named.
    let notes = receipt.fidelity_notes.join(" ");
    assert!(
        notes.contains("unavailable"),
        "the withholding must be declared: {notes}"
    );
    let name = {
        let mut conn = s.state.db_pool.get().expect("connection");
        use crate::schema::world_items;
        world_items::table
            .filter(world_items::id.eq(s.item_id))
            .select(world_items::name)
            .first::<String>(&mut conn)
            .expect("the item still exists")
    };
    assert!(
        !notes.contains(&name),
        "FR-022 forbids naming the withheld artifact, but the notes say: {notes}"
    );
}

/// T042, FR-025: reversing a takedown returns the member with nothing rebuilt.
///
/// This passes only if no status was cached anywhere — `effective_status`
/// resolves from the latest event every time. That is the design working, and
/// it is worth asserting precisely because the obvious optimisation (a
/// `moderated` flag on the member row) would break it silently.
#[tokio::test]
async fn a_reversed_takedown_returns_the_member_without_rebuilding_the_collection() {
    let s = source();
    let case_id = {
        let mut conn = s.state.db_pool.get().expect("connection");
        disable(&mut conn, "world_item", s.item_id, s.world_id)
    };

    let code = share_of(&s, &[("item", s.item_id)], "Taken down then restored").await;

    let while_disabled = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.7",
        code.clone(),
    )
    .await;
    assert!(
        while_disabled.is_err(),
        "FR-024: a collection whose every member is withheld reports nothing available"
    );

    {
        let mut conn = s.state.db_pool.get().expect("connection");
        restore(&mut conn, case_id, "world_item", s.item_id, s.world_id);
    }

    // The same code, the same collection, nothing touched in between.
    let after = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.7",
        code,
    )
    .await
    .expect("the member returns without the owner rebuilding anything");
    assert_eq!(after.members.len(), 1);
    assert_eq!(after.withheld_count, 0);
}

/// T044, FR-001b, the direction the add-time check cannot cover.
///
/// `resolve.rs` already proves that restricting an ability after adding it
/// withholds it. The other half matters as much and is easier to get wrong:
/// lifting the restriction must return it, with no rebuild — which it does
/// only because nothing about the restriction was ever written down on the
/// member row.
#[tokio::test]
async fn lifting_a_restriction_returns_the_member_to_the_collection() {
    use crate::schema::world_abilities;
    let s = source();

    let set_gm_only = |flag: bool| {
        let mut conn = s.state.db_pool.get().expect("connection");
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(s.ability_id)))
            .set(world_abilities::gm_only.eq(flag))
            .execute(&mut conn)
            .expect("toggle gm_only");
    };

    let code = share_of(
        &s,
        &[("ability", s.ability_id), ("lore", s.lore_id)],
        "A restriction that comes and goes",
    )
    .await;

    set_gm_only(true);
    let restricted = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.8",
        code.clone(),
    )
    .await
    .expect("the collection still opens");
    assert_eq!(restricted.members.len(), 1, "the ability is withheld");
    assert_eq!(restricted.withheld_count, 1);

    set_gm_only(false);
    let lifted = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.8",
        code,
    )
    .await
    .expect("the collection still opens");
    assert_eq!(
        lifted.members.len(),
        2,
        "FR-001b's other direction: lifting the restriction returns the member"
    );
    assert_eq!(lifted.withheld_count, 0);
}

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
