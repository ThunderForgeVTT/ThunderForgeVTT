//! Shop listings, and buying from one.

use super::*;

// ============================================================================
// createShopListing / purchaseFromShop (spec 020, FR-004/FR-005/FR-005a)
// ============================================================================

pub(crate) fn load_stock_quantity(
    conn: &mut PgConnection,
    actor_id: Uuid,
    item_id: Uuid,
) -> Result<i32, diesel::result::Error> {
    world_actor_inventory::table
        .filter(world_actor_inventory::actor_id.eq(actor_id))
        .filter(world_actor_inventory::item_id.eq(item_id))
        .select(world_actor_inventory::quantity)
        .first::<i32>(conn)
        .optional()
        .map(|v| v.unwrap_or(0))
}

pub(crate) fn build_graphql_shop_listing(
    row: GenieShopListing,
    stock_quantity: i32,
) -> GraphQLGenieShopListing {
    GraphQLGenieShopListing {
        id: row.id,
        actor_id: row.actor_id,
        item_id: row.item_id,
        price_kind: row.price_kind,
        price_resource_type: row.price_resource_type,
        price_resource_amount: row.price_resource_amount,
        price_item_id: row.price_item_id,
        price_item_quantity: row.price_item_quantity,
        stock_quantity,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_shop_listing_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    item_id: Uuid,
    price_kind: GenieShopPriceKind,
    price_resource_type: Option<String>,
    price_resource_amount: Option<i32>,
    price_item_id: Option<Uuid>,
    price_item_quantity: Option<i32>,
) -> GraphQLResult<GraphQLGenieShopListing> {
    let is_resource_price = price_resource_type.is_some() && price_resource_amount.is_some();
    let is_item_price = price_item_id.is_some() && price_item_quantity.is_some();
    if is_resource_price == is_item_price {
        return Err(Error::new(
            "Exactly one of a resource price or an item price must be configured",
        ));
    }
    match price_kind {
        GenieShopPriceKind::Resource if !is_resource_price => {
            return Err(Error::new(
                "priceKind is RESOURCE but no resource price was provided",
            ));
        }
        GenieShopPriceKind::Item if !is_item_price => {
            return Err(Error::new(
                "priceKind is ITEM but no item price was provided",
            ));
        }
        _ => {}
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| "Actor not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM may create a shop listing"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let listing = tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
        let new_listing = NewGenieShopListing {
            actor_id,
            item_id,
            price_kind: price_kind.as_db_str().to_string(),
            price_resource_type,
            price_resource_amount,
            price_item_id,
            price_item_quantity,
            created_by: user_id,
        };
        diesel::insert_into(world_genie_shop_listings::table)
            .values(&new_listing)
            .returning(GenieShopListing::as_returning())
            .get_result::<GenieShopListing>(&mut conn)
            .map_err(|e| format!("Failed to create shop listing: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let stock_quantity =
        tokio::task::spawn_blocking(move || load_stock_quantity(&mut conn, actor_id, item_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|e| Error::new(format!("Failed to load stock: {e}")))?;

    Ok(build_graphql_shop_listing(listing, stock_quantity))
}

/// FR-005/FR-005a: atomic purchase. Verifies the buyer can afford the
/// listing's configured price (resource balance or held item quantity),
/// deducts/transfers that price, transfers one unit of the listed item,
/// and performs a single atomic conditional stock decrement — all in one
/// transaction, so two buyers racing for the last unit can't both
/// succeed (the losing UPDATE affects 0 rows and the whole transaction
/// rolls back with a clean "out of stock" error).
pub async fn purchase_from_shop_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    listing_id: Uuid,
    buyer_actor_id: Uuid,
) -> GraphQLResult<GraphQLGenieShopListing> {
    require_caller_controls_actor(state, user_id, is_admin, buyer_actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let listing = tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
        world_genie_shop_listings::table
            .filter(world_genie_shop_listings::id.eq(listing_id))
            .select(GenieShopListing::as_select())
            .first::<GenieShopListing>(&mut conn)
            .map_err(|_| "Shop listing not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    // A resource-priced purchase needs an active Genie session to draw
    // holdings from — `load_holding_quantity`/`set_holding_quantity` are
    // keyed by session_id. An item-priced (barter) purchase touches only
    // world_actor_inventory and needs no session at all.
    let session_id = if listing.price_kind == "resource" {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let seller_actor_id = listing.actor_id;
        let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
            world_actors::table
                .filter(world_actors::id.eq(seller_actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(&mut conn)
                .map_err(|_| "Actor not found".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let session = tokio::task::spawn_blocking(move || -> Result<GenieSession, String> {
            world_genie_sessions::table
                .filter(world_genie_sessions::world_id.eq(world_id))
                .filter(world_genie_sessions::status.eq("active"))
                .select(GenieSession::as_select())
                .first::<GenieSession>(&mut conn)
                .map_err(|_| "There is no active Genie session for this world".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;
        Some(session.id)
    } else {
        None
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id_for_event = {
        let seller_actor_id = listing.actor_id;
        let mut lookup_conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
            world_actors::table
                .filter(world_actors::id.eq(seller_actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(&mut lookup_conn)
                .map_err(|_| "Actor not found".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?
    };

    let updated_listing =
        tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
            conn.transaction(|conn| -> Result<GenieShopListing, TxError> {
                // FR-005a: atomic conditional stock decrement first — if this
                // affects 0 rows, either the listing never had stock or a
                // concurrent purchase just took the last unit; either way,
                // fail cleanly with no other state touched.
                let decremented = diesel::update(
                    world_actor_inventory::table
                        .filter(world_actor_inventory::actor_id.eq(listing.actor_id))
                        .filter(world_actor_inventory::item_id.eq(listing.item_id))
                        .filter(world_actor_inventory::quantity.gt(0)),
                )
                .set((
                    world_actor_inventory::quantity.eq(world_actor_inventory::quantity - 1),
                    world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
                    world_actor_inventory::updated_by.eq(user_id),
                ))
                .execute(conn)?;

                if decremented == 0 {
                    return Err(TxError::Msg("This item is out of stock".to_string()));
                }

                // Pay the price.
                if listing.price_kind == "resource" {
                    let resource_type = listing.price_resource_type.clone().unwrap_or_default();
                    let amount = listing.price_resource_amount.unwrap_or(0);
                    let session_id = session_id
                        .expect("resource-priced listing always resolves a session_id above");
                    let current =
                        load_holding_quantity(conn, session_id, buyer_actor_id, &resource_type)
                            .map_err(TxError::Msg)?;
                    if current < amount {
                        return Err(TxError::Msg(
                            "You do not have enough of this resource to afford this purchase"
                                .to_string(),
                        ));
                    }
                    set_holding_quantity(
                        conn,
                        session_id,
                        buyer_actor_id,
                        &resource_type,
                        current - amount,
                    )
                    .map_err(TxError::Msg)?;
                } else {
                    let required_item_id = listing
                        .price_item_id
                        .expect("item-priced listing always has price_item_id");
                    let required_qty = listing.price_item_quantity.unwrap_or(0);
                    let held_qty = load_stock_quantity(conn, buyer_actor_id, required_item_id)?;
                    if held_qty < required_qty {
                        return Err(TxError::Msg(
                            "You do not hold the required item(s) to afford this purchase"
                                .to_string(),
                        ));
                    }
                    // Remove the traded-in item from the buyer, add it to the
                    // seller's inventory (the NPC "collects" what it's paid).
                    diesel::update(
                        world_actor_inventory::table
                            .filter(world_actor_inventory::actor_id.eq(buyer_actor_id))
                            .filter(world_actor_inventory::item_id.eq(required_item_id)),
                    )
                    .set((
                        world_actor_inventory::quantity
                            .eq(world_actor_inventory::quantity - required_qty),
                        world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
                        world_actor_inventory::updated_by.eq(user_id),
                    ))
                    .execute(conn)?;
                    grant_item_to_actor_in_tx(
                        conn,
                        listing.actor_id,
                        required_item_id,
                        required_qty,
                        user_id,
                    )?;
                }

                // Transfer the listed item to the buyer.
                grant_item_to_actor_in_tx(conn, buyer_actor_id, listing.item_id, 1, user_id)?;

                let _ = record_world_event(
                    conn,
                    world_id_for_event,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "purchase",
                        "listing_id": listing.id,
                        "buyer_actor_id": buyer_actor_id,
                        "seller_actor_id": listing.actor_id,
                        "item_id": listing.item_id,
                    })),
                    user_id,
                );

                Ok(listing.clone())
            })
            .map_err(String::from)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let stock_quantity = tokio::task::spawn_blocking(move || {
        load_stock_quantity(&mut conn, updated_listing.actor_id, updated_listing.item_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to load stock: {e}")))?;

    Ok(build_graphql_shop_listing(updated_listing, stock_quantity))
}
