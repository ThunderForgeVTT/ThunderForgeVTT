//! Spec 013: Actor inventory management (`addItemToInventory`,
//! `adjustInventoryQuantity`, `removeInventoryEntry`). Permissioned
//! against the ACTOR's ownership block, not the item's (FR-013,
//! spec Assumptions). See contracts/graphql-inventory.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLInventoryEntry};
use crate::graphql::{app_state, authenticated_user};
use crate::models::ActorInventoryEntry;
use crate::schema::{world_actor_inventory, world_items};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct AddItemToInventoryInput {
    pub actor_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i32,
}

#[derive(InputObject, Debug, Clone)]
pub struct AdjustInventoryQuantityInput {
    pub inventory_entry_id: Uuid,
    pub quantity: i32,
}

/// Testable core of `InventoryMutation::add_item_to_inventory`. Requires
/// Editor/Owner on the ACTOR (not the item). Repeated adds of the same
/// item merge into the existing entry's quantity (unique `(actor_id,
/// item_id)` + `ON CONFLICT ... DO UPDATE`, research.md §2) rather than
/// creating a duplicate row.
pub async fn add_item_to_inventory_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AddItemToInventoryInput,
) -> GraphQLResult<ActorInventoryEntry> {
    if input.quantity < 1 {
        return Err(Error::new("Quantity must be at least 1 when adding an item"));
    }

    require_actor_permission(state, user_id, is_admin, input.actor_id, ActorPermissionLevel::Editor).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_id = input.actor_id;
    let item_id = input.item_id;
    let quantity = input.quantity;

    tokio::task::spawn_blocking(move || {
        let item_name = world_items::table
            .filter(world_items::id.eq(item_id))
            .select(world_items::name)
            .first::<String>(&mut conn)
            .map_err(|_| "Item not found".to_string())?;

        diesel::insert_into(world_actor_inventory::table)
            .values((
                world_actor_inventory::actor_id.eq(actor_id),
                world_actor_inventory::item_id.eq(item_id),
                world_actor_inventory::item_name_snapshot.eq(item_name.clone()),
                world_actor_inventory::quantity.eq(quantity),
            ))
            .on_conflict((world_actor_inventory::actor_id, world_actor_inventory::item_id))
            .do_update()
            .set((
                world_actor_inventory::quantity.eq(world_actor_inventory::quantity + quantity),
                world_actor_inventory::item_name_snapshot.eq(item_name),
                world_actor_inventory::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(ActorInventoryEntry::as_returning())
            .get_result::<ActorInventoryEntry>(&mut conn)
            .map_err(|e| format!("Failed to add item to inventory: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `InventoryMutation::adjust_inventory_quantity`. Sets
/// an absolute value (not a delta). A resulting quantity of 0 deletes the
/// row (FR-011) and this returns `Ok(None)`.
pub async fn adjust_inventory_quantity_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: AdjustInventoryQuantityInput,
) -> GraphQLResult<Option<ActorInventoryEntry>> {
    if input.quantity < 0 {
        return Err(Error::new("Quantity must not be negative"));
    }

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let entry_id = input.inventory_entry_id;
    let actor_id = tokio::task::spawn_blocking(move || {
        world_actor_inventory::table
            .filter(world_actor_inventory::id.eq(entry_id))
            .select(world_actor_inventory::actor_id)
            .first::<Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Inventory entry not found"))?;

    require_actor_permission(state, user_id, is_admin, actor_id, ActorPermissionLevel::Editor).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let quantity = input.quantity;

    if quantity == 0 {
        tokio::task::spawn_blocking(move || {
            diesel::delete(world_actor_inventory::table.filter(world_actor_inventory::id.eq(entry_id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to remove inventory entry"))?;
        return Ok(None);
    }

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(world_actor_inventory::table.filter(world_actor_inventory::id.eq(entry_id)))
            .set((
                world_actor_inventory::quantity.eq(quantity),
                world_actor_inventory::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(ActorInventoryEntry::as_returning())
            .get_result::<ActorInventoryEntry>(&mut conn)
            .map_err(|e| format!("Failed to adjust inventory quantity: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(Some(updated))
}

/// Testable core of `InventoryMutation::remove_inventory_entry`. Deletes
/// the row outright, regardless of quantity.
pub async fn remove_inventory_entry_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    inventory_entry_id: Uuid,
) -> GraphQLResult<bool> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_id = tokio::task::spawn_blocking(move || {
        world_actor_inventory::table
            .filter(world_actor_inventory::id.eq(inventory_entry_id))
            .select(world_actor_inventory::actor_id)
            .first::<Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Inventory entry not found"))?;

    require_actor_permission(state, user_id, is_admin, actor_id, ActorPermissionLevel::Editor).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_actor_inventory::table.filter(world_actor_inventory::id.eq(inventory_entry_id))).execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove inventory entry"))?;

    Ok(true)
}

#[derive(Default)]
pub struct InventoryMutation;

#[async_graphql::Object]
impl InventoryMutation {
    async fn add_item_to_inventory(
        &self,
        ctx: &Context<'_>,
        input: AddItemToInventoryInput,
    ) -> GraphQLResult<GraphQLInventoryEntry> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        add_item_to_inventory_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLInventoryEntry::from)
    }

    async fn adjust_inventory_quantity(
        &self,
        ctx: &Context<'_>,
        input: AdjustInventoryQuantityInput,
    ) -> GraphQLResult<Option<GraphQLInventoryEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let result = adjust_inventory_quantity_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        Ok(result.map(GraphQLInventoryEntry::from))
    }

    async fn remove_inventory_entry(&self, ctx: &Context<'_>, inventory_entry_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_inventory_entry_impl(state, auth_user.user_id, auth_user.is_admin, inventory_entry_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_actors::{create_actor_impl, CreateActorInput};
    use crate::graphql::mutations_items::{create_item_impl, CreateItemInput};
    use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    /// SC-002: repeated adds of the same item merge into a single entry's
    /// quantity, never a duplicate row.
    #[tokio::test]
    async fn adding_same_item_twice_merges_quantity() {
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
        .expect("first add should succeed");

        let entry = add_item_to_inventory_impl(
            &state,
            owner_id,
            false,
            AddItemToInventoryInput {
                actor_id: actor.id,
                item_id: item.id,
                quantity: 2,
            },
        )
        .await
        .expect("second add should merge");

        assert_eq!(entry.quantity, 5, "quantities must merge rather than duplicate rows");
    }

    /// FR-011: reducing quantity to 0 removes the entry.
    #[tokio::test]
    async fn adjusting_quantity_to_zero_removes_entry() {
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
                name: "Longsword".to_string(),
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

        let entry = add_item_to_inventory_impl(
            &state,
            owner_id,
            false,
            AddItemToInventoryInput {
                actor_id: actor.id,
                item_id: item.id,
                quantity: 1,
            },
        )
        .await
        .expect("add should succeed");

        let result = adjust_inventory_quantity_impl(
            &state,
            owner_id,
            false,
            AdjustInventoryQuantityInput {
                inventory_entry_id: entry.id,
                quantity: 0,
            },
        )
        .await
        .expect("adjust to 0 should succeed");
        assert!(result.is_none(), "reducing to 0 must remove the entry");
    }

    /// FR-013/Assumptions: inventory permission follows the ACTOR's
    /// ownership block, not the item's — a Viewer-only-on-the-item caller
    /// with Editor on the Actor can still add it.
    #[tokio::test]
    async fn inventory_permission_follows_actor_not_item() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
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
                label: "Player Character".to_string(),
                is_npc: false,
                actor_type: None,
                game_system_id: None,
                description: None,
            },
        )
        .await
        .expect("DM should create actor");

        crate::graphql::mutations_actor_permissions::set_actor_permission_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_actor_permissions::SetActorPermissionInput {
                actor_id: actor.id,
                user_id: player_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .expect("DM should grant Editor on the actor");

        // player_id has only default Viewer on the item, but Editor on the actor.
        let entry = add_item_to_inventory_impl(
            &state,
            player_id,
            false,
            AddItemToInventoryInput {
                actor_id: actor.id,
                item_id: item.id,
                quantity: 1,
            },
        )
        .await
        .expect("Editor-on-actor caller should be able to add an item they can only view");
        assert_eq!(entry.quantity, 1);
    }
}
