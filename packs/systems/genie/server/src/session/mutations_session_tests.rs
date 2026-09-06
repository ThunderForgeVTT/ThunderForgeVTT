use super::*;

#[tokio::test]
async fn a_world_may_hold_only_one_active_session() {
    // The bug this guards: creating a Genie world already starts a
    // session, and `startGenieSession` inserted a second unconditionally.
    // `genieSession(worldId)` returns the newest active one, so
    // concluding it silently resurfaced the older — a GM saw the Doom
    // Clock they had just filled replaced by an untouched one, and no
    // "Session lost" banner at all.
    let state = test_app_state();
    let (world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

    let second = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 6,
        },
    )
    .await;
    assert!(
        second.is_err(),
        "a second concurrent session must be refused while one is active"
    );

    // Concluding the first frees the world for the next one — a new game
    // night is the whole point of the mutation.
    advance_doom_clock_impl(&state, owner_id, false, session_id, 4)
        .await
        .expect("filling the clock should conclude the session");
    let third = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 6,
        },
    )
    .await;
    assert!(
        third.is_ok(),
        "starting a session after the previous one ended must be allowed"
    );
}

#[tokio::test]
async fn only_gm_can_start_a_session() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let denied = start_genie_session_impl(
        &state,
        player_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 4,
        },
    )
    .await;
    assert!(
        denied.is_err(),
        "a non-GM caller must not be able to start a session"
    );
}

#[tokio::test]
async fn fresh_session_starts_with_three_wishes_and_zero_doom_segments() {
    let state = test_app_state();
    let (_world_id, _owner_id, _player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let session = load_session_row(&mut conn, session_id).unwrap();
    assert_eq!(
        session.wishes_remaining, 3,
        "FR-013: Session Wish Pool starts at 3"
    );
    assert_eq!(session.doom_clock_current, 0);
    assert_eq!(session.status, "active");
}

#[tokio::test]
async fn non_gm_cannot_spend_a_wish_or_advance_clocks() {
    let state = test_app_state();
    let (_world_id, _owner_id, player_id, session_id) = setup_active_session(&state).await;

    let result = spend_wish_impl(
        &state,
        player_id,
        false,
        session_id,
        "Undo the failed roll".to_string(),
    )
    .await;
    assert!(
        result.is_err(),
        "a non-GM caller must not be able to spend a wish"
    );

    let result = advance_doom_clock_impl(&state, player_id, false, session_id, 1).await;
    assert!(
        result.is_err(),
        "a non-GM caller must not be able to advance the Doom Clock"
    );

    let result = create_puzzle_clock_impl(
        &state,
        player_id,
        false,
        session_id,
        "Escape the vault".to_string(),
        4,
    )
    .await;
    assert!(
        result.is_err(),
        "a non-GM caller must not be able to create a Puzzle Clock"
    );
}

#[tokio::test]
async fn spend_wish_decrements_pool_and_rejects_when_empty() {
    let state = test_app_state();
    let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

    for i in (0..3).rev() {
        let session = spend_wish_impl(&state, owner_id, false, session_id, format!("Effect {i}"))
            .await
            .unwrap();
        assert_eq!(session.wishes_remaining, i);
    }

    let result = spend_wish_impl(
        &state,
        owner_id,
        false,
        session_id,
        "One too many".to_string(),
    )
    .await;
    assert!(
        result.is_err(),
        "spending a wish from an empty pool must be rejected"
    );
}

#[tokio::test]
async fn advance_doom_clock_sets_lost_when_it_fills() {
    let state = test_app_state();
    let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

    let session = advance_doom_clock_impl(&state, owner_id, false, session_id, 4)
        .await
        .unwrap();
    assert_eq!(session.doom_clock_current, 4);
    assert!(matches!(session.status, GenieSessionStatus::Lost));

    // Further advancement is rejected once the session has concluded.
    let result = advance_doom_clock_impl(&state, owner_id, false, session_id, 1).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn win_takes_precedence_over_loss_in_the_same_action_window() {
    // Edge case (spec.md): the last active Puzzle Clock resolving
    // takes precedence over a Doom Clock fill that would otherwise
    // be evaluated afterward.
    let state = test_app_state();
    let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

    let clock = create_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        session_id,
        "Only clock".to_string(),
        2,
    )
    .await
    .unwrap();

    // Resolve the only Puzzle Clock first — this must fire the win.
    let resolved = advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 2, None)
        .await
        .unwrap();
    assert!(resolved.resolved_at.is_some());

    let mut conn = state.db_pool.get().unwrap();
    let session = load_session_row(&mut conn, session_id).unwrap();
    assert_eq!(
        session.status, "won",
        "resolving the last Puzzle Clock must win the session"
    );
    drop(conn);

    // The Doom Clock is now moot: advancing it must be rejected since
    // the session already concluded (won), never flipping it to lost.
    let result = advance_doom_clock_impl(&state, owner_id, false, session_id, 4).await;
    assert!(result.is_err());
    let mut conn = state.db_pool.get().unwrap();
    let session = load_session_row(&mut conn, session_id).unwrap();
    assert_eq!(
        session.status, "won",
        "a win must not be overwritten by a later loss check"
    );
}

#[tokio::test]
async fn acceptresourcetrade_rejects_self_accept() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    // Seed actor_a with insight to trade away.
    set_holding_quantity(&mut conn, session_id, actor_a, "insight", 5).unwrap();
    drop(conn);

    let proposal = propose_resource_trade_impl(
        &state,
        owner_id,
        false,
        session_id,
        actor_a,
        "insight".to_string(),
        2,
        actor_b,
        "favor".to_string(),
        1,
    )
    .await
    .expect("actor_a's controller should be able to propose");

    let self_accept = accept_resource_trade_impl(&state, owner_id, false, proposal.id).await;
    assert!(
        self_accept.is_err(),
        "the proposer must not be able to accept their own proposal"
    );
}

#[tokio::test]
async fn decline_resource_trade_rejects_self_decline_and_succeeds_for_the_counterpart() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    drop(conn);

    let proposal = propose_resource_trade_impl(
        &state,
        owner_id,
        false,
        session_id,
        actor_a,
        "insight".to_string(),
        2,
        actor_b,
        "favor".to_string(),
        1,
    )
    .await
    .unwrap();

    let self_decline = decline_resource_trade_impl(&state, owner_id, false, proposal.id).await;
    assert!(
        self_decline.is_err(),
        "the proposer must not be able to decline their own proposal"
    );

    let declined = decline_resource_trade_impl(&state, player_id, false, proposal.id)
        .await
        .unwrap();
    assert_eq!(declined.status, "rejected");

    let re_decline = decline_resource_trade_impl(&state, player_id, false, proposal.id).await;
    assert!(
        re_decline.is_err(),
        "an already-declined proposal must not be declinable again"
    );

    let accept_after_decline =
        accept_resource_trade_impl(&state, player_id, false, proposal.id).await;
    assert!(
        accept_after_decline.is_err(),
        "a declined proposal must not be acceptable"
    );
}

#[tokio::test]
async fn accept_resource_trade_rejects_insufficient_holding_and_succeeds_when_funded() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    drop(conn);

    // actor_a proposes a trade it cannot afford (0 insight held).
    let proposal = propose_resource_trade_impl(
        &state,
        owner_id,
        false,
        session_id,
        actor_a,
        "insight".to_string(),
        3,
        actor_b,
        "favor".to_string(),
        1,
    )
    .await
    .unwrap();

    let underfunded = accept_resource_trade_impl(&state, player_id, false, proposal.id).await;
    assert!(
        underfunded.is_err(),
        "an insufficient holding must be rejected"
    );

    // Fund actor_a and actor_b, then retry with a fresh proposal
    // (the first attempt's proposal stays 'pending' since it never
    // committed — accept it again now that funds exist).
    let mut conn = state.db_pool.get().unwrap();
    set_holding_quantity(&mut conn, session_id, actor_a, "insight", 3).unwrap();
    set_holding_quantity(&mut conn, session_id, actor_b, "favor", 1).unwrap();
    drop(conn);

    let holdings = accept_resource_trade_impl(&state, player_id, false, proposal.id)
        .await
        .unwrap();
    assert!(!holdings.is_empty());

    let mut conn = state.db_pool.get().unwrap();
    let a_favor = load_holding_quantity(&mut conn, session_id, actor_a, "favor").unwrap();
    let b_insight = load_holding_quantity(&mut conn, session_id, actor_b, "insight").unwrap();
    assert_eq!(a_favor, 1, "actor_a should now hold the favor it received");
    assert_eq!(
        b_insight, 3,
        "actor_b should now hold the insight it received"
    );
}

#[tokio::test]
async fn spend_resource_on_puzzle_clock_rejects_insufficient_holding_and_advances_when_funded() {
    let state = test_app_state();
    let (world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    drop(conn);

    let clock =
        create_puzzle_clock_impl(&state, owner_id, false, session_id, "Vault".to_string(), 3)
            .await
            .unwrap();

    let insufficient = spend_resource_on_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        clock.id,
        actor_a,
        "essence".to_string(),
        2,
    )
    .await;
    assert!(
        insufficient.is_err(),
        "spending more than held must be rejected"
    );

    let mut conn = state.db_pool.get().unwrap();
    set_holding_quantity(&mut conn, session_id, actor_a, "essence", 2).unwrap();
    drop(conn);

    let updated = spend_resource_on_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        clock.id,
        actor_a,
        "essence".to_string(),
        2,
    )
    .await
    .unwrap();
    assert_eq!(updated.segments_current, 2);

    let mut conn = state.db_pool.get().unwrap();
    let remaining = load_holding_quantity(&mut conn, session_id, actor_a, "essence").unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn a_player_who_only_claimed_their_character_controls_it_for_session_resources() {
    // Spec 019 regression guard: caller_controls_actor previously
    // checked only world_actors.owned_by, which never changes on
    // claim (spec 017's real player-onboarding path — the GM creates
    // the actor, a player then claims it via world_actor_claims).
    // Found live: a claimed-not-owned player got "You do not control
    // this actor" on every Session Resource action for their own PC.
    use thunderforge_server::graphql::mutations_actor_claims::claim_actor_impl;

    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    // Owned by the GM, not the player — only a claim will follow.
    let claimed_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let other_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    diesel::update(
        thunderforge_server::schema::world_actors::table
            .filter(thunderforge_server::schema::world_actors::id.eq(claimed_actor)),
    )
    .set(thunderforge_server::schema::world_actors::available_for_claim.eq(true))
    .execute(&mut conn)
    .unwrap();
    drop(conn);

    claim_actor_impl(&state, player_id, world_id, claimed_actor)
        .await
        .expect("player should be able to claim an available actor");

    // Before the fix this failed with "You do not control this actor".
    let proposal = propose_resource_trade_impl(
        &state,
        player_id,
        false,
        session_id,
        claimed_actor,
        "insight".to_string(),
        1,
        other_actor,
        "favor".to_string(),
        1,
    )
    .await;
    assert!(
        proposal.is_ok(),
        "a player who claimed (not owns) their character should control it for Session Resource actions: {:?}",
        proposal.err()
    );
}

// ========================================================================
// Spec 020: grantSessionResource (FR-001), resource carryover (FR-003)
// ========================================================================

#[tokio::test]
async fn grant_session_resource_increases_holding_and_requires_gm_and_active_session() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    drop(conn);

    let holding = grant_session_resource_impl(
        &state,
        owner_id,
        false,
        session_id,
        actor,
        "essence".to_string(),
        3,
    )
    .await
    .expect("GM should be able to grant a resource");
    assert_eq!(holding.quantity, 3);

    // Non-GM caller rejected (Scenario 4).
    let denied = grant_session_resource_impl(
        &state,
        player_id,
        false,
        session_id,
        actor,
        "essence".to_string(),
        1,
    )
    .await;
    assert!(
        denied.is_err(),
        "a non-GM caller must not be able to grant a resource"
    );
}

#[tokio::test]
async fn grant_session_resource_rejects_when_no_active_session() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    drop(conn);

    // No session started at all — grant must be rejected (Scenario 3).
    let session = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 2,
        },
    )
    .await
    .unwrap();
    // Force the session to a concluded state so "no active session" is exercised.
    let mut conn = state.db_pool.get().unwrap();
    diesel::update(world_genie_sessions::table.filter(world_genie_sessions::id.eq(session.id)))
        .set(world_genie_sessions::status.eq("won"))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let denied = grant_session_resource_impl(
        &state,
        owner_id,
        false,
        session.id,
        actor,
        "essence".to_string(),
        1,
    )
    .await;
    assert!(
        denied.is_err(),
        "granting against a concluded session must be rejected"
    );
}

#[tokio::test]
async fn resource_carryover_copies_holdings_when_enabled_and_resets_when_disabled() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    drop(conn);

    let first_session = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 2,
        },
    )
    .await
    .unwrap();
    grant_session_resource_impl(
        &state,
        owner_id,
        false,
        first_session.id,
        actor,
        "favor".to_string(),
        5,
    )
    .await
    .unwrap();
    let mut conn = state.db_pool.get().unwrap();
    diesel::update(
        world_genie_sessions::table.filter(world_genie_sessions::id.eq(first_session.id)),
    )
    .set(world_genie_sessions::status.eq("won"))
    .execute(&mut conn)
    .unwrap();
    drop(conn);

    // Carryover disabled (default): the new session starts empty.
    let second_session = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 2,
        },
    )
    .await
    .unwrap();
    let mut conn = state.db_pool.get().unwrap();
    let qty = load_holding_quantity(&mut conn, second_session.id, actor, "favor").unwrap();
    assert_eq!(
        qty, 0,
        "carryover disabled by default — new session should start at 0"
    );
    diesel::update(
        world_genie_sessions::table.filter(world_genie_sessions::id.eq(second_session.id)),
    )
    .set(world_genie_sessions::status.eq("won"))
    .execute(&mut conn)
    .unwrap();
    drop(conn);

    // Enable carryover, end the second session, start a third — holdings should carry.
    let mut conn = state.db_pool.get().unwrap();
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::genie_resource_carryover_enabled.eq(true))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let third_session = start_genie_session_impl(
        &state,
        owner_id,
        false,
        StartGenieSessionInput {
            world_id,
            doom_clock_max: 2,
        },
    )
    .await
    .unwrap();
    let mut conn = state.db_pool.get().unwrap();
    let qty = load_holding_quantity(&mut conn, third_session.id, actor, "favor").unwrap();
    assert_eq!(
        qty, 0,
        "carryover should only copy the immediately prior session's holdings, which were 0"
    );
}

// ========================================================================
// Spec 020: createShopListing / purchaseFromShop (FR-004/FR-005/FR-005a)
// ========================================================================
