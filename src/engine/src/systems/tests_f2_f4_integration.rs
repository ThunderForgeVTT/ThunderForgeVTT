//! Phase 4.9.F.2: Integration Tests for Multiplayer Workflows
//!
//! End-to-end tests simulating:
//! - Complete token movement workflow
//! - Conflict resolution scenarios
//! - Multi-player synchronization
//! - Presence broadcasting and reception
//! - Offline/reconnect scenarios
//!
//! These tests verify the full circular event-driven data flow.
//!
//! # This file had never been compiled
//!
//! Gated `#![cfg(all(test, target_arch = "wasm32"))]` and never declared as a
//! module, so none of it was reachable on any target (spec 032 T083). Two
//! things went with the ungating: `scenario_mutation_timeout_and_rollback`,
//! which had no assertions at all, and `test_suite_coverage_f2_f4`, which was
//! a wall of `eprintln!` announcing that the tests above it had passed
//! whether or not they had. See T084.

#![cfg(test)]

// Integration test scenarios
mod integration_tests_f2 {
    /// Test Scenario 1: Single user moves token locally
    /// Expected: Mutation queued → sent → server validates → position synced
    #[test]
    fn scenario_single_player_token_movement() {
        eprintln!("\n[SCENARIO 1] Single Player Token Movement");

        // 1. Initial state: token at (0, 0)
        let initial_x = 0;
        let initial_y = 0;
        eprintln!(
            "  1. Initial token position: ({}, {})",
            initial_x, initial_y
        );

        // 2. Player moves token to (100, 200)
        let target_x = 100;
        let target_y = 200;
        eprintln!("  2. Player moves to: ({}, {})", target_x, target_y);

        // 3. Queue mutation
        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();
        queue.push_move_token("token-1".to_string(), target_x, target_y);

        let pending = queue.get_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].token_id, "token-1");
        assert_eq!(pending[0].x, target_x);
        assert_eq!(pending[0].y, target_y);
        eprintln!("  3. Mutation queued: {:?}", pending[0].mutation_id);

        // 4. Mutation sent
        let mut sender_state = crate::systems::mutation_sender::MutationSenderState::default();
        let mutation_id = pending[0].mutation_id.clone();
        sender_state.in_flight.insert(
            mutation_id.clone(),
            crate::systems::token_sync_d2::PendingMutationInfo {
                mutation_id: mutation_id.clone(),
                token_id: "token-1".to_string(),
                x: target_x,
                y: target_y,
                sent_at: 1.0,
                timeout_secs: 5.0,
            },
        );
        eprintln!("  4. Mutation sent to server");

        // 5. Server response received
        sender_state.in_flight.remove(&mutation_id);
        queue.mark_complete(&mutation_id);
        eprintln!("  5. Server response received");

        // 6. Verify token position synced
        assert_eq!(queue.get_pending().len(), 0);
        eprintln!("  6. Token position synced: ({}, {})", target_x, target_y);
        eprintln!("  ✅ Scenario 1 passed");
    }

    /// Test Scenario 2: Two players move tokens simultaneously (no conflict)
    /// Expected: Both mutations queued → both sent → both synced independently
    #[test]
    fn scenario_two_players_concurrent_moves_no_conflict() {
        eprintln!("\n[SCENARIO 2] Two Players Concurrent Moves (No Conflict)");

        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();

        // Player 1 moves token-1
        eprintln!("  1. Player 1 moves token-1 to (100, 100)");
        queue.push_move_token("token-1".to_string(), 100, 100);

        // Player 2 moves token-2
        eprintln!("  2. Player 2 moves token-2 to (200, 200)");
        queue.push_move_token("token-2".to_string(), 200, 200);

        let pending = queue.get_pending();
        assert_eq!(pending.len(), 2);
        eprintln!("  3. Both mutations queued");

        // Simulate server processing both
        let mut mutations_completed = 0;
        for mutation_info in pending {
            if mutation_info.token_id == "token-1" {
                assert_eq!(mutation_info.x, 100);
                assert_eq!(mutation_info.y, 100);
            } else if mutation_info.token_id == "token-2" {
                assert_eq!(mutation_info.x, 200);
                assert_eq!(mutation_info.y, 200);
            }
            mutations_completed += 1;
        }

        assert_eq!(mutations_completed, 2);
        eprintln!("  4. Server processed both mutations");

        // Verify both synced
        queue.mark_complete(&queue.get_pending()[0].mutation_id);
        queue.mark_complete(&queue.get_pending()[0].mutation_id);
        eprintln!("  5. Both tokens synced");
        eprintln!("  ✅ Scenario 2 passed");
    }

    /// Test Scenario 3: Conflict detection (same token, concurrent moves)
    /// Expected: Both mutations sent → server detects LWW conflict → one wins
    #[test]
    fn scenario_conflict_detection_same_token() {
        eprintln!("\n[SCENARIO 3] Conflict Detection (Same Token, Concurrent Moves)");

        // Both players move same token
        eprintln!("  1. Player 1 moves token-1 to (100, 100)");
        eprintln!("  2. Player 2 moves token-1 to (200, 200)");

        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();

        // Simulate two mutations for same token
        // Positional indexing into `get_pending()` was an order assumption a
        // `HashMap` does not honour; the second mutation is the one that was
        // not there a moment ago.
        queue.push_move_token("token-1".to_string(), 100, 100);
        let mut_1 = queue.get_pending()[0].mutation_id.clone();

        queue.push_move_token("token-1".to_string(), 200, 200);
        let pending = queue.get_pending();
        assert_eq!(pending.len(), 2, "same token, two moves, two mutations");
        let mut_2 = pending
            .iter()
            .map(|info| info.mutation_id.clone())
            .find(|id| *id != mut_1)
            .expect("the second move gets its own mutation id");

        eprintln!("  3. Both mutations queued");

        // Server receives both, but mut_2 has later timestamp (LWW)
        // Server returns event_code=2 for mut_1 (conflict)
        eprintln!("  4. Server detects conflict (LWW)");
        eprintln!("  5. mut_1 rolled back, mut_2 wins");

        // Mark mut_1 as conflicted
        let conflict_event = crate::systems::mutation_sender::ConflictDetected {
            mutation_id: mut_1.clone(),
            token_id: "token-1".to_string(),
            conflict_code: 2,
        };

        assert_eq!(conflict_event.conflict_code, 2);
        assert_eq!(conflict_event.mutation_id, mut_1, "the loser is recorded");
        assert_ne!(mut_1, mut_2, "the winner is a different mutation");
        eprintln!("  6. Conflict recorded for audit trail");
        eprintln!("  ✅ Scenario 3 passed");
    }

    // Scenario 4 — "Mutation timeout and rollback" — is deliberately absent.
    //
    // It had no assertions: it bound `queue.check_timeouts(6.0)` to a
    // variable it never read, printed five progress lines and then
    // "✅ Scenario 4 passed" unconditionally. It could not have had a real
    // one either: `GraphQLMutationQueue::push_move_token` sets `sent_at: 0.0`,
    // nothing in the crate ever sets it to anything else, and
    // `check_timeouts` skips every mutation whose `sent_at` is 0.0 — so the
    // timeout path is unreachable, and the rollback it was named for never
    // happens. The reachable half of that (an unsent mutation does not time
    // out) is asserted in `tests_f1_unit::test_graphql_mutation_queue_timeout
    // _detection`. See spec 032 T084.

    /// Test Scenario 5: Player presence broadcast and reception
    /// Expected: Local player broadcasts every 500ms → remote receives → displays
    #[test]
    fn scenario_player_presence_broadcast() {
        eprintln!("\n[SCENARIO 5] Player Presence Broadcast and Reception");

        let mut local = crate::systems::presence::LocalPlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );

        eprintln!("  1. Local player initialized: Alice");

        // Check if ready to broadcast
        assert!(local.should_broadcast(0.5));
        eprintln!("  2. Ready to broadcast at t=0.5s");

        // Mark as sent
        local.mark_broadcast(0.5);
        eprintln!("  3. Broadcast sent at t=0.5s");

        // Check next broadcast
        assert!(!local.should_broadcast(0.9)); // Only 0.4s passed
        eprintln!("  4. Not ready at t=0.9s (< 0.5s interval)");

        assert!(local.should_broadcast(1.0)); // 0.5s passed
        eprintln!("  5. Ready at t=1.0s (>= 0.5s interval)");

        // Simulate receiving remote player presence
        let remote_presence = crate::systems::presence::PlayerPresence::new(
            "player-2".to_string(),
            "Bob".to_string(),
            "world-1".to_string(),
        );

        let mut registry = crate::systems::presence::PresenceRegistry::default();
        registry.add_or_update(remote_presence);

        eprintln!("  6. Received Bob's presence update");
        assert_eq!(registry.count(), 1);

        eprintln!("  7. Bob's cursor displayed on canvas");
        eprintln!("  ✅ Scenario 5 passed");
    }

    /// Test Scenario 6: Stale presence cleanup
    /// Expected: Player offline > 10s → presence marked stale → removed
    #[test]
    fn scenario_stale_presence_removal() {
        eprintln!("\n[SCENARIO 6] Stale Presence Cleanup");

        let mut registry = crate::systems::presence::PresenceRegistry::default();

        // Add two players
        let mut alice = crate::systems::presence::PlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );
        let mut bob = crate::systems::presence::PlayerPresence::new(
            "player-2".to_string(),
            "Bob".to_string(),
            "world-1".to_string(),
        );

        // Both online at t=0
        alice.last_seen = 0.0;
        bob.last_seen = 0.0;

        registry.add_or_update(alice);
        registry.add_or_update(bob);

        eprintln!("  1. Alice and Bob online at t=0.0s");
        assert_eq!(registry.count(), 2);

        // Alice sends update at t=5.0s
        let mut alice_updated = crate::systems::presence::PlayerPresence::new(
            "player-1".to_string(),
            "Alice".to_string(),
            "world-1".to_string(),
        );
        alice_updated.last_seen = 5.0;
        registry.add_or_update(alice_updated);

        eprintln!("  2. Alice sends presence update at t=5.0s");

        // At t=15.0s, check staleness
        let current_time = 15.0;
        let all = registry.get_all();

        let stale_players: Vec<_> = all
            .iter()
            .filter(|p| p.is_stale(current_time))
            .map(|p| p.player_id.clone())
            .collect();

        eprintln!("  3. At t=15.0s: Bob is stale (last_seen=0, > 10s)",);
        assert!(stale_players.contains(&"player-2".to_string()));

        // Remove stale players
        for player_id in stale_players {
            registry.remove(&player_id);
        }

        eprintln!("  4. Removed stale player: Bob");
        assert_eq!(registry.count(), 1);
        assert!(registry.get("player-1").is_some());

        eprintln!("  5. Only Alice remains on canvas");
        eprintln!("  ✅ Scenario 6 passed");
    }

    /// Test Scenario 7: Conflict marker animation
    /// Expected: Conflict detected → red flash appears → fades out over 2s
    #[test]
    fn scenario_conflict_marker_animation() {
        eprintln!("\n[SCENARIO 7] Conflict Marker Animation");

        let marker = crate::systems::conflict_visualization::ConflictMarker::new(0.0);

        eprintln!("  1. Conflict detected at t=0.0s");
        eprintln!("  2. Red indicator attached to token");

        // Animation: 0s → 0.5s → 1.0s → 1.5s → 2.0s
        let alpha_0_0 = marker.get_alpha(0.0);
        let alpha_0_5 = marker.get_alpha(0.5);
        let alpha_1_0 = marker.get_alpha(1.0);
        let alpha_1_5 = marker.get_alpha(1.5);
        let alpha_2_0 = marker.get_alpha(2.0);

        eprintln!(
            "  3. Alpha fade: {:.2} → {:.2} → {:.2} → {:.2} → {:.2}",
            alpha_0_0, alpha_0_5, alpha_1_0, alpha_1_5, alpha_2_0
        );

        // Verify fade-out
        assert!(alpha_0_0 > alpha_0_5);
        assert!(alpha_0_5 > alpha_1_0);
        assert!(alpha_1_0 > alpha_1_5);
        assert!(alpha_1_5 > alpha_2_0);

        eprintln!("  4. Animation complete at t=2.0s");
        eprintln!("  ✅ Scenario 7 passed");
    }

    /// Test Scenario 8: Multiple mutations from same token (rapid moves)
    /// Expected: Queue 3 moves rapidly → all queued → sent in batch
    #[test]
    fn scenario_rapid_token_movements() {
        eprintln!("\n[SCENARIO 8] Rapid Token Movements (Multiple Queued)");

        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();

        // Simulate rapid clicks
        eprintln!("  1. User rapidly moves token:");
        for i in 0..5 {
            let x = 100 + (i * 20);
            let y = 100 + (i * 20);
            queue.push_move_token("token-1".to_string(), x, y);
            eprintln!("     - Movement {}: ({}, {})", i + 1, x, y);
        }

        let pending = queue.get_pending();
        assert_eq!(pending.len(), 5);
        eprintln!("  2. All 5 mutations queued");

        // Server processes all
        for mutation in pending {
            eprintln!(
                "  3. Server processes: token={}, pos=({}, {})",
                mutation.token_id, mutation.x, mutation.y
            );
        }

        eprintln!("  ✅ Scenario 8 passed");
    }
}

#[cfg(test)]
mod performance_tests_f4 {
    /// Test: Simulate 10 concurrent players moving tokens
    /// Expected: All mutations queued and processed without error
    #[test]
    fn test_performance_10_concurrent_players() {
        eprintln!("\n[PERFORMANCE TEST] 10 Concurrent Players");

        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();
        let mut presence_registry = crate::systems::presence::PresenceRegistry::default();

        // Simulate 10 players
        for player_id in 0..10 {
            // Each player moves one token
            let token_id = format!("token-{}", player_id);
            let x = 100 + (player_id * 20);
            let y = 100 + (player_id * 20);

            queue.push_move_token(token_id.clone(), x, y);

            // Add player to presence registry
            let mut presence = crate::systems::presence::PlayerPresence::new(
                format!("player-{}", player_id),
                format!("Player {}", player_id),
                "world-1".to_string(),
            );
            presence.camera_x = x as f32;
            presence.camera_y = y as f32;

            presence_registry.add_or_update(presence);
        }

        // Verify all queued
        let pending = queue.get_pending();
        assert_eq!(pending.len(), 10);
        assert_eq!(presence_registry.count(), 10);

        eprintln!("  ✅ 10 concurrent players: OK");
    }

    /// Test: Simulate 100 rapid mutations in quick succession
    /// Expected: Queue handles all without overflow
    #[test]
    fn test_performance_100_rapid_mutations() {
        eprintln!("\n[PERFORMANCE TEST] 100 Rapid Mutations");

        let mut queue = crate::systems::token_sync_d2::GraphQLMutationQueue::new();

        // Queue 100 mutations
        for i in 0..100 {
            let token_id = format!("token-{}", i % 10); // 10 unique tokens
            let x = (i * 5) % 1000;
            let y = (i * 3) % 800;

            queue.push_move_token(token_id, x, y);
        }

        let pending = queue.get_pending();
        assert_eq!(pending.len(), 100);

        eprintln!("  ✅ 100 rapid mutations: OK");
    }

    /// Test: Stale presence check with 1000 players
    /// Expected: Efficient O(n) iteration without timeout
    #[test]
    fn test_performance_1000_presence_players() {
        eprintln!("\n[PERFORMANCE TEST] 1000 Presence Players");

        let mut registry = crate::systems::presence::PresenceRegistry::default();

        // Add 1000 players
        for i in 0..1000 {
            let mut presence = crate::systems::presence::PlayerPresence::new(
                format!("player-{}", i),
                format!("Player {}", i),
                "world-1".to_string(),
            );

            // Half are fresh, half are stale
            if i % 2 == 0 {
                presence.last_seen = 5.0; // Fresh
            } else {
                presence.last_seen = 0.0; // Stale
            }

            registry.add_or_update(presence);
        }

        assert_eq!(registry.count(), 1000);

        // Check staleness at t=15.0
        let stale_count = registry
            .get_all()
            .iter()
            .filter(|p| p.is_stale(15.0))
            .count();

        assert_eq!(stale_count, 500); // Half should be stale

        eprintln!("  ✅ 1000 presence players: OK ({} stale)", stale_count);
    }
}
