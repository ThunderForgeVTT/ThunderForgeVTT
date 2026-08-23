//! Spec 013: Actor inventory read query (`actorInventory`). See
//! contracts/graphql-inventory.md.

use async_graphql::Context;

use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLInventoryEntry};
use crate::graphql::*;
use crate::models::ActorInventoryEntry;
use crate::schema::world_actor_inventory;
use crate::state::AppState;

/// Testable core of `InventoryQuery::actor_inventory`. Requires at least
/// Viewer on the actor (FR-013's read half). Includes deleted-item rows
/// (rendered via `item_name_snapshot`).
pub async fn actor_inventory_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    actor_id: uuid::Uuid,
) -> GraphQLResult<Vec<ActorInventoryEntry>> {
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Viewer,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_actor_inventory::table
            .filter(world_actor_inventory::actor_id.eq(actor_id))
            .select(ActorInventoryEntry::as_select())
            .load::<ActorInventoryEntry>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor inventory"))
}

#[derive(Default)]
pub struct InventoryQuery;

#[async_graphql::Object]
impl InventoryQuery {
    async fn actor_inventory(
        &self,
        ctx: &Context<'_>,
        actor_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLInventoryEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows =
            actor_inventory_impl(state, auth_user.user_id, auth_user.is_admin, actor_id).await?;
        Ok(rows.into_iter().map(GraphQLInventoryEntry::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_actors::{CreateActorInput, create_actor_impl};
    use crate::graphql::mutations_inventory::{
        AddItemToInventoryInput, add_item_to_inventory_impl,
    };
    use crate::graphql::mutations_items::{CreateItemInput, create_item_impl};
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, test_app_state,
    };

    #[tokio::test]
    async fn actor_inventory_lists_entries_for_a_viewer() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Potion of Healing".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

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

        add_item_to_inventory_impl(
            &state,
            owner_id,
            false,
            AddItemToInventoryInput {
                actor_id: actor.id,
                item_id: item.id,
                quantity: 3,
            },
        )
        .await
        .expect("add should succeed");

        let entries = actor_inventory_impl(&state, owner_id, false, actor.id)
            .await
            .expect("owner should be able to view inventory");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].quantity, 3);
    }
}
