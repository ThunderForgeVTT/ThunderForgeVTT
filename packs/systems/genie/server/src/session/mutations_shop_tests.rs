use super::*;

#[tokio::test]
async fn purchase_from_shop_resource_priced_happy_path_and_insufficient_funds() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let buyer = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    let item = insert_test_item(&mut conn, world_id, owner_id, "Rusty Lantern");
    stock_item(&mut conn, npc, item, 1, owner_id);
    drop(conn);

    let listing = create_shop_listing_impl(
        &state,
        owner_id,
        false,
        npc,
        item,
        GenieShopPriceKind::Resource,
        Some("insight".to_string()),
        Some(2),
        None,
        None,
    )
    .await
    .expect("GM should be able to create a listing");
    assert_eq!(listing.stock_quantity, 1);

    // Insufficient funds (Scenario 2).
    let denied = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer).await;
    assert!(
        denied.is_err(),
        "buyer with insufficient Insight must be rejected"
    );

    // Fund the buyer, then purchase succeeds (Scenario 1).
    grant_session_resource_impl(
        &state,
        owner_id,
        false,
        session_id,
        buyer,
        "insight".to_string(),
        2,
    )
    .await
    .unwrap();
    let purchased = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer)
        .await
        .expect("funded buyer should be able to purchase");
    assert_eq!(
        purchased.stock_quantity, 0,
        "last unit purchased — stock decremented to 0"
    );

    let mut conn = state.db_pool.get().unwrap();
    let buyer_insight = load_holding_quantity(&mut conn, session_id, buyer, "insight").unwrap();
    assert_eq!(
        buyer_insight, 0,
        "2 Insight deducted for a 2-Insight purchase"
    );
    let buyer_stock = load_stock_quantity(&mut conn, buyer, item).unwrap();
    assert_eq!(
        buyer_stock, 1,
        "purchased item transferred into buyer's inventory"
    );
}

#[tokio::test]
async fn purchase_from_shop_barter_happy_path_and_missing_item() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, _session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let buyer = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    let lantern = insert_test_item(&mut conn, world_id, owner_id, "Rusty Lantern");
    let flask = insert_test_item(&mut conn, world_id, owner_id, "Sealed Flask");
    stock_item(&mut conn, npc, lantern, 1, owner_id);
    drop(conn);

    let listing = create_shop_listing_impl(
        &state,
        owner_id,
        false,
        npc,
        lantern,
        GenieShopPriceKind::Item,
        None,
        None,
        Some(flask),
        Some(1),
    )
    .await
    .expect("GM should be able to create a barter listing");

    // Buyer doesn't hold the flask yet (Scenario 4).
    let denied = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer).await;
    assert!(
        denied.is_err(),
        "buyer without the required barter item must be rejected"
    );

    let mut conn = state.db_pool.get().unwrap();
    stock_item(&mut conn, buyer, flask, 1, owner_id);
    drop(conn);

    purchase_from_shop_impl(&state, player_id, false, listing.id, buyer)
        .await
        .expect("buyer holding the barter item should be able to purchase");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        load_stock_quantity(&mut conn, buyer, flask).unwrap(),
        0,
        "flask traded away"
    );
    assert_eq!(
        load_stock_quantity(&mut conn, buyer, lantern).unwrap(),
        1,
        "lantern received"
    );
    assert_eq!(
        load_stock_quantity(&mut conn, npc, flask).unwrap(),
        1,
        "NPC collected the traded-in flask"
    );
}

#[tokio::test]
async fn purchase_from_shop_last_unit_race_only_one_buyer_succeeds() {
    // FR-005a: two buyers racing for the last unit — exactly one
    // succeeds, the other gets a clean "out of stock" error, no
    // partial state change on the loser.
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let player_two = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_two, "Player");
    let buyer_a = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    let buyer_b = insert_test_actor(&mut conn, world_id, scene_id, player_two);
    let item = insert_test_item(&mut conn, world_id, owner_id, "Sole Survivor Blade");
    stock_item(&mut conn, npc, item, 1, owner_id);
    drop(conn);

    let listing = create_shop_listing_impl(
        &state,
        owner_id,
        false,
        npc,
        item,
        GenieShopPriceKind::Resource,
        Some("insight".to_string()),
        Some(1),
        None,
        None,
    )
    .await
    .unwrap();

    grant_session_resource_impl(
        &state,
        owner_id,
        false,
        session_id,
        buyer_a,
        "insight".to_string(),
        1,
    )
    .await
    .unwrap();
    grant_session_resource_impl(
        &state,
        owner_id,
        false,
        session_id,
        buyer_b,
        "insight".to_string(),
        1,
    )
    .await
    .unwrap();

    let (result_a, result_b) = tokio::join!(
        purchase_from_shop_impl(&state, player_id, false, listing.id, buyer_a),
        purchase_from_shop_impl(&state, player_two, false, listing.id, buyer_b),
    );

    let successes = [&result_a, &result_b].iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        successes, 1,
        "exactly one of the two concurrent purchases should succeed"
    );

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        load_stock_quantity(&mut conn, npc, item).unwrap(),
        0,
        "stock never goes negative or double-decrements"
    );

    // The loser must have no partial state change.
    if result_a.is_err() {
        assert_eq!(
            load_holding_quantity(&mut conn, session_id, buyer_a, "insight").unwrap(),
            1
        );
        assert_eq!(load_stock_quantity(&mut conn, buyer_a, item).unwrap(), 0);
    } else {
        assert_eq!(
            load_holding_quantity(&mut conn, session_id, buyer_b, "insight").unwrap(),
            1
        );
        assert_eq!(load_stock_quantity(&mut conn, buyer_b, item).unwrap(), 0);
    }
}

// ========================================================================
// Spec 020: configurePuzzleClockReward / advancePuzzleClock actorId (FR-006/FR-006a)
// ========================================================================
