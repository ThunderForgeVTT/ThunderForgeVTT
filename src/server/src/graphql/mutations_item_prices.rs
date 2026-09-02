//! Spec 031 (T071, US8/FR-037): the Game Master's note of what an item costs.
//!
//! # What this is not
//!
//! ADR-058. It is text with a number in it. Nothing here spends, deducts,
//! converts, validates against, or settles with the value, and nothing
//! should be added that does: pricing is already system-specific
//! (`world_genie_shop_listings` models a full economy, keyed per *vendor*),
//! and a second, generic economy alongside it would leave no rule about
//! which one is true. A system's view is free to show this, ignore it, or
//! override it.
//!
//! # Why its own table and one row per item
//!
//! Price is not a property every item in every ruleset has, which is why
//! Genie put its economy in its own table rather than on `world_items`. The
//! unique constraint on `item_id` says the rest: this is the Game Master's
//! single note about an item, not a price list — a per-vendor quantity is a
//! different thing and stays where Genie already keeps it.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::item_permissions::require_item_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLItemPrice};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewWorldItemPrice, WorldItemPrice};
use crate::schema::world_item_prices;
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct SetItemPriceInput {
    pub item_id: Uuid,
    pub amount: i32,
    /// Free text — this layer names no currency system.
    pub currency_label: Option<String>,
    /// "Roughly what it goes for" versus "this is the price". Intent, not
    /// behaviour: neither is enforced anywhere.
    pub is_suggested: Option<bool>,
}

/// FR-037. Editor-or-Owner on the item, the same gate `updateItem` uses —
/// Constitution Principle III: whether a caller may write this is settled
/// here, not by which screens happen to render the field.
///
/// An upsert rather than insert-or-update at the call site: the unique
/// constraint on `item_id` is the thing that guarantees one note per item,
/// so letting the database resolve the collision keeps two concurrent edits
/// from producing a second row that nothing would ever show.
pub async fn set_item_price_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: SetItemPriceInput,
) -> GraphQLResult<WorldItemPrice> {
    require_item_permission(
        state,
        user_id,
        is_admin,
        input.item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Blank is not a currency. A label of whitespace would render as a
    // trailing gap after the number, which reads as a bug rather than as a
    // Game Master who did not name a currency.
    let currency_label = input
        .currency_label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty());

    let new_price = NewWorldItemPrice {
        item_id: input.item_id,
        amount: input.amount,
        currency_label,
        is_suggested: input.is_suggested.unwrap_or(false),
        created_by: user_id,
        updated_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_item_prices::table)
            .values(&new_price)
            .on_conflict(world_item_prices::item_id)
            .do_update()
            .set((
                world_item_prices::amount.eq(new_price.amount),
                world_item_prices::currency_label.eq(new_price.currency_label.clone()),
                world_item_prices::is_suggested.eq(new_price.is_suggested),
                world_item_prices::updated_by.eq(user_id),
                world_item_prices::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldItemPrice::as_returning())
            .get_result::<WorldItemPrice>(&mut conn)
            .map_err(|e| format!("Failed to record item price: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Removes the note entirely, which is distinct from a price of zero: one
/// says the Game Master has not priced this, the other says it is free.
pub async fn clear_item_price_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<bool> {
    require_item_permission(
        state,
        user_id,
        is_admin,
        item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let deleted = tokio::task::spawn_blocking(move || {
        diesel::delete(world_item_prices::table.filter(world_item_prices::item_id.eq(item_id)))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to clear item price"))?;

    Ok(deleted > 0)
}

/// The note on one item, or `None` where the Game Master wrote none.
pub async fn item_price_impl(
    state: &AppState,
    item_id: Uuid,
) -> GraphQLResult<Option<WorldItemPrice>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_item_prices::table
            .filter(world_item_prices::item_id.eq(item_id))
            .select(WorldItemPrice::as_select())
            .first::<WorldItemPrice>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item price"))
}

#[derive(Default)]
pub struct ItemPriceMutation;

#[async_graphql::Object]
impl ItemPriceMutation {
    async fn set_item_price(
        &self,
        ctx: &Context<'_>,
        input: SetItemPriceInput,
    ) -> GraphQLResult<GraphQLItemPrice> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let price =
            set_item_price_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        Ok(price.into())
    }

    async fn clear_item_price(&self, ctx: &Context<'_>, item_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        clear_item_price_impl(state, auth_user.user_id, auth_user.is_admin, item_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_items::{CreateItemInput, create_item_impl};
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    async fn item_for_test(state: &AppState, owner_id: Uuid, world_id: Uuid) -> Uuid {
        create_item_impl(
            state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Silver Lamp".to_string(),
                description: None,
            },
        )
        .await
        .expect("the DM may create an item")
        .id
    }

    /// FR-037: the note is recorded as written, currency label and all.
    #[tokio::test]
    async fn set_item_price_records_the_note() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item_id = item_for_test(&state, owner_id, world_id).await;
        let price = set_item_price_impl(
            &state,
            owner_id,
            false,
            SetItemPriceInput {
                item_id,
                amount: 40,
                currency_label: Some("  gp  ".to_string()),
                is_suggested: Some(true),
            },
        )
        .await
        .unwrap();

        assert_eq!(price.amount, 40);
        assert_eq!(price.currency_label.as_deref(), Some("gp"));
        assert!(price.is_suggested);

        let mut conn = state.db_pool.get().unwrap();
        let stored = world_item_prices::table
            .filter(world_item_prices::item_id.eq(item_id))
            .select(WorldItemPrice::as_select())
            .first::<WorldItemPrice>(&mut conn)
            .expect("the row should exist");
        assert_eq!(stored.id, price.id);
        assert_eq!(stored.created_by, owner_id);
    }

    /// At most one note per item: re-pricing rewrites the same row.
    #[tokio::test]
    async fn set_item_price_replaces_rather_than_accumulates() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item_id = item_for_test(&state, owner_id, world_id).await;
        let first = set_item_price_impl(
            &state,
            owner_id,
            false,
            SetItemPriceInput {
                item_id,
                amount: 40,
                currency_label: Some("gp".to_string()),
                is_suggested: Some(true),
            },
        )
        .await
        .unwrap();
        let second = set_item_price_impl(
            &state,
            owner_id,
            false,
            SetItemPriceInput {
                item_id,
                amount: 55,
                currency_label: None,
                is_suggested: Some(false),
            },
        )
        .await
        .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.amount, 55);
        assert_eq!(second.currency_label, None);
        assert!(!second.is_suggested);

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_item_prices::table
            .filter(world_item_prices::item_id.eq(item_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 1);
    }

    /// Constitution Principle III: a Viewer is refused at the data boundary,
    /// whatever the interface offered them.
    #[tokio::test]
    async fn set_item_price_rejects_viewer_level_caller() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let viewer_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, viewer_id, "Player");
        drop(conn);

        let item_id = item_for_test(&state, owner_id, world_id).await;
        let result = set_item_price_impl(
            &state,
            viewer_id,
            false,
            SetItemPriceInput {
                item_id,
                amount: 1,
                currency_label: None,
                is_suggested: None,
            },
        )
        .await;
        assert!(result.is_err());

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_item_prices::table
            .filter(world_item_prices::item_id.eq(item_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0);
    }

    /// Clearing removes the note; unpriced and priced-at-zero stay distinct.
    #[tokio::test]
    async fn clear_item_price_removes_the_note() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item_id = item_for_test(&state, owner_id, world_id).await;
        set_item_price_impl(
            &state,
            owner_id,
            false,
            SetItemPriceInput {
                item_id,
                amount: 0,
                currency_label: None,
                is_suggested: None,
            },
        )
        .await
        .unwrap();
        assert!(item_price_impl(&state, item_id).await.unwrap().is_some());

        assert!(
            clear_item_price_impl(&state, owner_id, false, item_id)
                .await
                .unwrap()
        );
        assert!(item_price_impl(&state, item_id).await.unwrap().is_none());
    }
}
