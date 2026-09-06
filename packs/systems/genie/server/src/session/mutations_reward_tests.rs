use super::*;

#[tokio::test]
async fn per_segment_reward_grants_exactly_once_per_advance_not_a_lump_sum() {
    let state = test_app_state();
    let (world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let smith = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let dagger = insert_test_item(&mut conn, world_id, owner_id, "Dagger");
    drop(conn);

    let clock = create_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        session_id,
        "Forge Daggers".to_string(),
        3,
    )
    .await
    .unwrap();

    for segment in 1..=3 {
        configure_puzzle_clock_reward_impl(
            &state,
            owner_id,
            false,
            clock.id,
            segment,
            None,
            None,
            Some(dagger),
            Some(1),
            GenieRewardRecipientMode::TriggeringActor,
        )
        .await
        .unwrap();
    }

    for _ in 0..3 {
        advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 1, Some(smith))
            .await
            .unwrap();
    }

    let mut conn = state.db_pool.get().unwrap();
    let dagger_count = load_stock_quantity(&mut conn, smith, dagger).unwrap();
    assert_eq!(
        dagger_count, 3,
        "one dagger granted per advance, not a lump sum"
    );
}

#[tokio::test]
async fn single_final_segment_reward_grants_once_split_across_party() {
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let pc_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let pc_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    drop(conn);

    let clock = create_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        session_id,
        "Recover the Sealed Lamp".to_string(),
        4,
    )
    .await
    .unwrap();
    configure_puzzle_clock_reward_impl(
        &state,
        owner_id,
        false,
        clock.id,
        4,
        Some("favor".to_string()),
        Some(2),
        None,
        None,
        GenieRewardRecipientMode::WholeParty,
    )
    .await
    .unwrap();

    advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 4, None)
        .await
        .unwrap();

    let mut conn = state.db_pool.get().unwrap();
    let a_favor = load_holding_quantity(&mut conn, session_id, pc_a, "favor").unwrap();
    let b_favor = load_holding_quantity(&mut conn, session_id, pc_b, "favor").unwrap();
    assert_eq!(
        a_favor + b_favor,
        2,
        "the full configured amount is split across the party, none lost"
    );
    assert!(
        a_favor >= 1 && b_favor >= 1,
        "both party members should receive a share of an even split"
    );
}

#[tokio::test]
async fn zero_configured_rewards_clock_behaves_unchanged() {
    let state = test_app_state();
    let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;
    let clock = create_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        session_id,
        "Plain Clock".to_string(),
        2,
    )
    .await
    .unwrap();

    let resolved = advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 2, None)
        .await
        .unwrap();
    assert!(
        resolved.resolved_at.is_some(),
        "a zero-reward clock still resolves normally"
    );
}

#[tokio::test]
async fn triggering_actor_reward_falls_back_to_whole_party_when_no_actor_id_supplied() {
    // FR-006a: a plain advancePuzzleClock call with no actorId hits a
    // triggering_actor-mode reward — it must fall back to whole-party
    // split rather than failing or crediting no one.
    let state = test_app_state();
    let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
    let mut conn = state.db_pool.get().unwrap();
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let pc_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
    let pc_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
    drop(conn);

    let clock = create_puzzle_clock_impl(
        &state,
        owner_id,
        false,
        session_id,
        "Untended Forge".to_string(),
        2,
    )
    .await
    .unwrap();
    configure_puzzle_clock_reward_impl(
        &state,
        owner_id,
        false,
        clock.id,
        1,
        Some("essence".to_string()),
        Some(2),
        None,
        None,
        GenieRewardRecipientMode::TriggeringActor,
    )
    .await
    .unwrap();

    // No actorId supplied — plain GM "Advance" click.
    advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 1, None)
        .await
        .unwrap();

    let mut conn = state.db_pool.get().unwrap();
    let a_essence = load_holding_quantity(&mut conn, session_id, pc_a, "essence").unwrap();
    let b_essence = load_holding_quantity(&mut conn, session_id, pc_b, "essence").unwrap();
    assert_eq!(
        a_essence + b_essence,
        2,
        "an unattributed triggering_actor reward must fall back to a whole-party grant, not be dropped"
    );
}
