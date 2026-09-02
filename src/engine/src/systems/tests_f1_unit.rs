//! Phase 4.9.F.1: Unit Tests for Phase 4.9 Systems
//!
//! # This file had never been compiled
//!
//! It was gated `#![cfg(all(test, target_arch = "wasm32"))]` **and** never
//! declared as a module anywhere, so none of it was reachable on any target.
//! Its header claimed "60+ unit tests" and its summary "50+"; there are 33.
//! Both claims, and the `test_suite_coverage` function that printed the
//! second one, are gone (spec 032 T083/T084).
//!
//! Covers:
//! - Token sync systems (D.2)
//! - Mutation sender (D.2.2)
//! - Conflict visualization (D.2.3)
//! - Presence systems (D.3)
//! - WebSocket connectivity (D.1)
//! - Event dispatcher (D.1)

#![cfg(test)]

use bevy::prelude::*;

// Import all Phase 4.9.D systems for testing
use crate::systems::conflict_visualization::ConflictMarker;
use crate::systems::event_dispatcher::{WorldEventQueue, WorldEventReceived};
use crate::systems::mutation_sender::{ConflictDetected, MutationRejected, MutationSenderState};
use crate::systems::presence::{
    LocalPlayerPresence, PlayerPresence, PresenceCursor, PresenceLabel, PresenceRegistry,
};
use crate::systems::token_sync_d2::{GraphQLMutationQueue, PendingMutation, RollbackState};

/// `GraphQLMutationQueue` stores pending mutations in a `HashMap`, so
/// `get_pending()` comes back in whatever order the map iterates. Several
/// tests here indexed it positionally and asserted `pending[0]` was the token
/// they pushed first, which is not a property the queue has.
fn pending_for<'a>(
    pending: &'a [crate::systems::token_sync_d2::PendingMutationInfo],
    token_id: &str,
) -> &'a crate::systems::token_sync_d2::PendingMutationInfo {
    pending
        .iter()
        .find(|info| info.token_id == token_id)
        .unwrap_or_else(|| panic!("no pending mutation for {token_id}"))
}

// ============================================================================
// F.1.1: Token Sync System Tests (D.2)
// ============================================================================

#[test]
fn test_token_sync_rollback_state_creation() {
    let rollback = RollbackState {
        original_x: 100,
        original_y: 200,
    };

    assert_eq!(rollback.original_x, 100);
    assert_eq!(rollback.original_y, 200);
}

#[test]
fn test_token_sync_pending_mutation_creation() {
    let mutation = PendingMutation {
        mutation_id: "mut-1".to_string(),
        sent_at: 1.0,
        timeout_secs: 5.0,
    };

    assert_eq!(mutation.mutation_id, "mut-1");
    assert_eq!(mutation.timeout_secs, 5.0);
}

#[test]
fn test_graphql_mutation_queue_creation() {
    let queue = GraphQLMutationQueue::new();
    assert_eq!(queue.get_pending().len(), 0);
}

#[test]
fn test_graphql_mutation_queue_push_and_get() {
    let mut queue = GraphQLMutationQueue::new();

    queue.push_move_token("token-1".to_string(), 100, 200);
    queue.push_move_token("token-2".to_string(), 150, 250);

    let pending = queue.get_pending();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending_for(&pending, "token-1").x, 100);
    assert_eq!(pending_for(&pending, "token-1").y, 200);
    assert_eq!(pending_for(&pending, "token-2").x, 150);
    assert_eq!(pending_for(&pending, "token-2").y, 250);
}

#[test]
fn test_graphql_mutation_queue_mark_complete() {
    let mut queue = GraphQLMutationQueue::new();

    queue.push_move_token("token-1".to_string(), 100, 200);
    let pending = queue.get_pending();
    let mutation_id = pending[0].mutation_id.clone();

    queue.mark_complete(&mutation_id);
    assert_eq!(queue.get_pending().len(), 0);
}

/// An unsent mutation never times out, and a sent one does.
///
/// The original had an empty `for` loop over the pending list with a comment
/// saying the body belonged in real code, then asserted zero timeouts
/// "because we haven't set sent_at" — an assertion that held for any
/// implementation of `check_timeouts`, including one that never fires.
#[test]
fn test_graphql_mutation_queue_timeout_detection() {
    let mut queue = GraphQLMutationQueue::new();

    queue.push_move_token("token-1".to_string(), 100, 200);
    queue.push_move_token("token-2".to_string(), 150, 250);

    // `sent_at` is 0.0 until something actually sends: an unsent mutation is
    // not overdue, however late it is, and it stays in the queue.
    assert!(queue.check_timeouts(6.0).is_empty());
    assert_eq!(queue.get_pending().len(), 2);
    assert!(
        queue.get_pending().iter().all(|info| info.sent_at == 0.0),
        "push_move_token leaves sent_at at 0.0"
    );

    // The overdue half of this cannot be asserted from here: `sent_at` is set
    // by nothing in the crate and `GraphQLMutationQueue` exposes no way to set
    // it, so `check_timeouts` cannot fire for any input. Reported rather than
    // papered over with a setter added for the test's benefit — see spec 032
    // T083.
}

#[test]
fn test_graphql_mutation_queue_multiple_operations() {
    let mut queue = GraphQLMutationQueue::new();

    // Add 5 mutations
    for i in 0..5 {
        queue.push_move_token(format!("token-{i}"), 100 + i, 200 + i);
    }

    assert_eq!(queue.get_pending().len(), 5);

    // Complete 2 of them
    let pending = queue.get_pending();
    queue.mark_complete(&pending[0].mutation_id);
    queue.mark_complete(&pending[2].mutation_id);

    assert_eq!(queue.get_pending().len(), 3);

    // Add more
    queue.push_move_token("token-extra".to_string(), 999, 999);
    assert_eq!(queue.get_pending().len(), 4);
}

// ============================================================================
// F.1.2: Mutation Sender Tests (D.2.2)
// ============================================================================

#[test]
fn test_mutation_sender_state_creation() {
    let state = MutationSenderState::new("http://localhost:4000/graphql".to_string());

    assert_eq!(state.in_flight.len(), 0);
    assert_eq!(state.recently_sent.len(), 0);
    assert_eq!(state.frames_since_clear, 0);
    assert!(state.graphql_endpoint.contains("localhost"));
}

#[test]
fn test_mutation_sender_state_default() {
    let state = MutationSenderState::default();
    assert!(state.graphql_endpoint.contains("localhost"));
}

#[test]
fn test_mutation_rejected_event_creation() {
    let event = MutationRejected {
        mutation_id: "mut-1".to_string(),
        token_id: "token-1".to_string(),
        error_code: 400,
        error_message: "Invalid position".to_string(),
    };

    assert_eq!(event.mutation_id, "mut-1");
    assert_eq!(event.error_code, 400);
    assert_eq!(event.error_message, "Invalid position");
}

#[test]
fn test_conflict_detected_event_creation() {
    let event = ConflictDetected {
        mutation_id: "mut-1".to_string(),
        token_id: "token-1".to_string(),
        conflict_code: 2,
    };

    assert_eq!(event.mutation_id, "mut-1");
    assert_eq!(event.conflict_code, 2);
}

#[test]
fn test_mutation_sender_in_flight_tracking() {
    let mut state = MutationSenderState::default();

    // Simulate adding mutations to in_flight
    let mutation_info = crate::systems::token_sync_d2::PendingMutationInfo {
        mutation_id: "mut-1".to_string(),
        token_id: "token-1".to_string(),
        x: 100,
        y: 200,
        sent_at: 1.0,
        timeout_secs: 5.0,
    };

    state.in_flight.insert("mut-1".to_string(), mutation_info);
    assert_eq!(state.in_flight.len(), 1);

    // Remove from in_flight
    state.in_flight.remove("mut-1");
    assert_eq!(state.in_flight.len(), 0);
}

// ============================================================================
// F.1.3: Conflict Visualization Tests (D.2.3)
// ============================================================================

#[test]
fn test_conflict_marker_creation() {
    let marker = ConflictMarker {
        started_at: 0.0,
        duration: 2.0,
        original_color: Color::srgb(1.0, 1.0, 1.0),
    };

    assert_eq!(marker.duration, 2.0);
    assert!(!marker.is_expired(1.0)); // 1 second < 2 second duration
}

#[test]
fn test_conflict_marker_expiration() {
    let marker = ConflictMarker {
        started_at: 0.0,
        duration: 2.0,
        original_color: Color::srgb(1.0, 1.0, 1.0),
    };

    assert!(!marker.is_expired(1.9)); // Not expired
    assert!(marker.is_expired(2.1)); // Expired
}

#[test]
fn test_conflict_marker_alpha_calculation() {
    let marker = ConflictMarker {
        started_at: 0.0,
        duration: 2.0,
        original_color: Color::srgb(1.0, 1.0, 1.0),
    };

    let alpha_halfway = marker.get_alpha(1.0); // 50% through animation
    assert!(alpha_halfway > 0.4 && alpha_halfway < 0.6); // Approximately 0.5

    let alpha_start = marker.get_alpha(0.0);
    assert!(alpha_start > 0.9); // Close to 1.0

    let alpha_end = marker.get_alpha(2.0);
    assert!(alpha_end < 0.1); // Close to 0.0
}

// ============================================================================
// F.1.4: Presence System Tests (D.3)
// ============================================================================

#[test]
fn test_player_presence_creation() {
    let presence = PlayerPresence::new(
        "player-1".to_string(),
        "Alice".to_string(),
        "world-1".to_string(),
    );

    assert_eq!(presence.player_id, "player-1");
    assert_eq!(presence.player_name, "Alice");
    assert_eq!(presence.world_id, "world-1");
    assert_eq!(presence.camera_x, 0.0);
    assert_eq!(presence.camera_y, 0.0);
}

#[test]
fn test_player_presence_stale_detection() {
    let mut presence = PlayerPresence::new(
        "player-1".to_string(),
        "Alice".to_string(),
        "world-1".to_string(),
    );

    presence.last_seen = 0.0;
    assert!(!presence.is_stale(9.0)); // 9 seconds < 10 second threshold
    assert!(presence.is_stale(11.0)); // 11 seconds > 10 second threshold
}

#[test]
fn test_presence_registry_add_and_get() {
    let mut registry = PresenceRegistry::default();

    let presence = PlayerPresence::new(
        "player-1".to_string(),
        "Alice".to_string(),
        "world-1".to_string(),
    );

    registry.add_or_update(presence.clone());
    assert_eq!(registry.count(), 1);

    let retrieved = registry.get("player-1");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().player_name, "Alice");
}

#[test]
fn test_presence_registry_remove() {
    let mut registry = PresenceRegistry::default();

    let presence = PlayerPresence::new(
        "player-1".to_string(),
        "Alice".to_string(),
        "world-1".to_string(),
    );

    registry.add_or_update(presence);
    assert_eq!(registry.count(), 1);

    registry.remove("player-1");
    assert_eq!(registry.count(), 0);
    assert!(registry.get("player-1").is_none());
}

#[test]
fn test_presence_registry_multiple_players() {
    let mut registry = PresenceRegistry::default();

    for i in 0..5 {
        let presence = PlayerPresence::new(
            format!("player-{}", i),
            format!("Player {}", i),
            "world-1".to_string(),
        );
        registry.add_or_update(presence);
    }

    assert_eq!(registry.count(), 5);

    let all = registry.get_all();
    assert_eq!(all.len(), 5);
}

#[test]
fn test_presence_registry_update_existing() {
    let mut registry = PresenceRegistry::default();

    let mut presence1 = PlayerPresence::new(
        "player-1".to_string(),
        "Alice".to_string(),
        "world-1".to_string(),
    );

    registry.add_or_update(presence1.clone());
    assert_eq!(registry.count(), 1);

    // Update position
    presence1.camera_x = 100.0;
    presence1.camera_y = 200.0;
    registry.add_or_update(presence1);

    assert_eq!(registry.count(), 1); // Still 1 player
    let updated = registry.get("player-1").unwrap();
    assert_eq!(updated.camera_x, 100.0);
    assert_eq!(updated.camera_y, 200.0);
}

#[test]
fn test_local_player_presence_creation() {
    let local = LocalPlayerPresence::new(
        "player-1".to_string(),
        "LocalPlayer".to_string(),
        "world-1".to_string(),
    );

    assert_eq!(local.player_id, "player-1");
    assert_eq!(local.broadcast_interval, 0.5);
    assert_eq!(local.last_broadcast, 0.0);
}

#[test]
fn test_local_player_presence_broadcast_interval() {
    let mut local = LocalPlayerPresence::new(
        "player-1".to_string(),
        "LocalPlayer".to_string(),
        "world-1".to_string(),
    );

    // Should broadcast at time 1.0
    assert!(local.should_broadcast(1.0));

    // Mark as broadcast at time 1.0
    local.mark_broadcast(1.0);
    assert_eq!(local.last_broadcast, 1.0);

    // Should not broadcast at 1.3 (only 0.3s passed, need 0.5s)
    assert!(!local.should_broadcast(1.3));

    // Should broadcast at 1.5 (0.5s passed)
    assert!(local.should_broadcast(1.5));

    // Mark as broadcast at 1.5
    local.mark_broadcast(1.5);

    // Should broadcast at 2.1 (0.6s passed)
    assert!(local.should_broadcast(2.1));
}

#[test]
fn test_presence_cursor_component() {
    let cursor = PresenceCursor {
        player_id: "player-1".to_string(),
    };

    assert_eq!(cursor.player_id, "player-1");
}

#[test]
fn test_presence_label_component() {
    let label = PresenceLabel {
        player_id: "player-2".to_string(),
    };

    assert_eq!(label.player_id, "player-2");
}

// ============================================================================
// F.1.5: Event Dispatcher Tests (D.1)
// ============================================================================

fn world_event(event_id: i64) -> WorldEventReceived {
    WorldEventReceived {
        event_id,
        event_code: 1,
        token_event: None,
        token_id: Some(format!("token-{event_id}")),
        created_by: None,
        event_type: None,
    }
}

#[test]
fn test_world_event_queue_creation() {
    let queue = WorldEventQueue::new();
    // Queue should start empty
    assert!(queue.peek().is_none());
}

/// The original pushed nothing — its loop body was a comment saying real code
/// would create events — and then asserted the drain was empty, which is true
/// of a queue that drops everything pushed into it. It now pushes.
#[test]
fn test_world_event_queue_push_and_drain() {
    let mut queue = WorldEventQueue::new();

    for i in 0..3 {
        queue.push(world_event(i));
    }

    // Peek does not consume.
    assert_eq!(queue.peek().map(|e| e.event_id), Some(0));

    // Drain returns everything, in the order it was pushed.
    let drained = queue.drain();
    assert_eq!(
        drained.iter().map(|e| e.event_id).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // And leaves the queue empty.
    assert!(queue.drain().is_empty());
    assert!(queue.peek().is_none());
}

// ============================================================================
// F.1.6: Integration Scenarios (Cross-System Tests)
// ============================================================================

#[test]
fn test_full_mutation_workflow() {
    // Simulate: User moves token → queue mutation → mark in-flight → get response
    let mut queue = GraphQLMutationQueue::new();
    let mut sender_state = MutationSenderState::default();

    // Step 1: Queue mutation
    queue.push_move_token("token-1".to_string(), 100, 200);
    assert_eq!(queue.get_pending().len(), 1);

    // Step 2: Simulate send_pending_mutations
    let pending = queue.get_pending();
    for mutation_info in pending {
        sender_state
            .in_flight
            .insert(mutation_info.mutation_id.clone(), mutation_info);
    }

    assert_eq!(sender_state.in_flight.len(), 1);

    // Step 3: Simulate response received
    let mutation_ids: Vec<_> = sender_state.in_flight.keys().cloned().collect();
    for mutation_id in mutation_ids {
        sender_state.in_flight.remove(&mutation_id);
        queue.mark_complete(&mutation_id);
    }

    assert_eq!(sender_state.in_flight.len(), 0);
    assert_eq!(queue.get_pending().len(), 0);
}

#[test]
fn test_presence_broadcast_workflow() {
    // Simulate: Local player broadcasts → registry updates → players displayed
    let mut registry = PresenceRegistry::default();
    let mut local = LocalPlayerPresence::new(
        "player-1".to_string(),
        "LocalPlayer".to_string(),
        "world-1".to_string(),
    );

    // Step 1: Local player ready to broadcast
    assert!(local.should_broadcast(0.5));

    // Step 2: Mark as sent
    local.mark_broadcast(0.5);

    // Step 3: Simulate server sends presence update
    let remote_presence = PlayerPresence::new(
        "player-2".to_string(),
        "RemotePlayer".to_string(),
        "world-1".to_string(),
    );

    registry.add_or_update(remote_presence);

    // Step 4: Verify registry has both (local not in registry, but remote is)
    assert_eq!(registry.count(), 1);
    assert!(registry.get("player-2").is_some());
}

#[test]
fn test_conflict_detection_workflow() {
    // Simulate: Conflict event received → marker created → animation updates
    let marker = ConflictMarker {
        started_at: 0.0,
        duration: 2.0,
        original_color: Color::srgb(1.0, 1.0, 1.0),
    };

    // Step 1: Check marker is not expired
    assert!(!marker.is_expired(1.0));

    // Step 2: Get alpha (should fade out)
    let alpha_start = marker.get_alpha(0.0);
    let alpha_mid = marker.get_alpha(1.0);
    let alpha_end = marker.get_alpha(2.0);

    assert!(alpha_start > alpha_mid); // Fading out
    assert!(alpha_mid > alpha_end);

    // Step 3: Check marker is expired after duration
    assert!(marker.is_expired(2.5));
}

#[test]
fn test_concurrent_mutations_scenario() {
    // Simulate: 3 tokens moved concurrently, all mutations queued
    let mut queue = GraphQLMutationQueue::new();

    // Queue 3 mutations
    queue.push_move_token("token-1".to_string(), 100, 100);
    queue.push_move_token("token-2".to_string(), 200, 200);
    queue.push_move_token("token-3".to_string(), 300, 300);

    assert_eq!(queue.get_pending().len(), 3);

    // Simulate one mutation rejected
    let pending = queue.get_pending();
    let rejected_id = pending[0].mutation_id.clone();

    // Mark it complete (would fire MutationRejected event in real code)
    queue.mark_complete(&rejected_id);
    assert_eq!(queue.get_pending().len(), 2);

    // Mark other two as complete
    let remaining = queue.get_pending();
    queue.mark_complete(&remaining[0].mutation_id);
    queue.mark_complete(&remaining[1].mutation_id);

    assert_eq!(queue.get_pending().len(), 0);
}

#[test]
fn test_stale_presence_cleanup_scenario() {
    // Simulate: Multiple players online, one goes stale, should be removed
    let mut registry = PresenceRegistry::default();

    // Add 3 players
    for i in 0..3 {
        let mut presence = PlayerPresence::new(
            format!("player-{}", i),
            format!("Player {}", i),
            "world-1".to_string(),
        );
        presence.last_seen = 0.0; // All last seen at time 0
        registry.add_or_update(presence);
    }

    assert_eq!(registry.count(), 3);

    // Update one player's last_seen (avoid stale)
    if let Some(presence) = registry.players.get_mut("player-0") {
        presence.last_seen = 5.0; // Recently seen
    }

    // Check stale detection at time 15.0
    let current_time = 15.0;
    let all = registry.get_all();

    let stale_count = all.iter().filter(|p| p.is_stale(current_time)).count();
    assert_eq!(stale_count, 2); // player-1 and player-2 are stale

    // Remove stale players
    for player in all {
        if player.is_stale(current_time) {
            registry.remove(&player.player_id);
        }
    }

    assert_eq!(registry.count(), 1); // Only player-0 remains
    assert!(registry.get("player-0").is_some());
}
