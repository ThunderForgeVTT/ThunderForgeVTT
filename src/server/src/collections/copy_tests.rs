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

#[path = "copy_fidelity_tests.rs"]
mod fidelity;

#[path = "copy_moderation_tests.rs"]
mod moderation;

#[path = "copy_asset_and_link_tests.rs"]
mod assets_and_links;
