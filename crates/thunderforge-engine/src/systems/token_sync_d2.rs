//! Phase 4.9.D.2: Token Position Sync System
//!
//! Handles real-time synchronization of token positions between:
//! - Local optimistic updates (immediate visual feedback)
//! - Server canonical state (authoritative position)
//! - Conflict resolution (Last-Write-Wins with rollback)

use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::components::Token;
use crate::systems::event_dispatcher::WorldEventQueue;

/// Counter for unique mutation IDs
static MUTATION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_mutation_id() -> String {
    let id = MUTATION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("mut-{}", id)
}

/// Component: Stores original position for rollback on conflict
#[derive(Component, Clone, Debug)]
pub struct RollbackState {
    pub original_x: i32,
    pub original_y: i32,
}

/// Component: Mutation is pending (waiting for server response)
#[derive(Component, Clone, Debug)]
pub struct PendingMutation {
    pub mutation_id: String,
    pub sent_at: f64,
    pub timeout_secs: f64,
}

/// System: Process incoming server events and sync token positions
pub fn sync_token_positions_from_server(
    mut query: Query<(&mut Token, &mut Transform, Option<&mut RollbackState>)>,
    mut queue: ResMut<WorldEventQueue>,
) {
    let events = queue.drain();

    for event in events {
        // Only process token events
        if event.event_code < 0 {
            eprintln!(
                "[Phase4.9.D✅] Ignoring rejected event: code={}",
                event.event_code
            );
            continue;
        }

        // Extract token_id and position from payload
        let token_id = match &event.token_id {
            Some(id) => id,
            None => continue,
        };

        let payload = match &event.token_event {
            Some(p) => p,
            None => continue,
        };

        // Extract x, y coordinates (i32)
        let x = payload
            .get("x")
            .or_else(|| payload.get("base_x"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let y = payload
            .get("y")
            .or_else(|| payload.get("base_y"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        eprintln!(
            "[Phase4.9.D📍] Syncing token {}: x={}, y={}",
            token_id, x, y
        );

        // Find token entity and update position
        for (mut token, mut transform, rollback_opt) in query.iter_mut() {
            if token.id == *token_id {
                // Handle conflict event
                if event.event_code == 2 {
                    eprintln!("[Phase4.9.D⚠️] Conflict detected for token: {}", token_id);

                    // Store original position for potential user revert
                    if let Some(mut rollback) = rollback_opt {
                        rollback.original_x = token.base_x;
                        rollback.original_y = token.base_y;
                    }

                    // LWW: Apply server position anyway (server is authoritative)
                }

                // Update token position from server
                token.base_x = x;
                token.base_y = y;

                // Update transform (convert i32 to f32 for rendering)
                transform.translation = Vec3::new(x as f32, y as f32, 0.0);

                eprintln!(
                    "[Phase4.9.D✅] Token {} synchronized: code={}",
                    token_id, event.event_code
                );

                break;
            }
        }
    }
}

/// System: Detect local token movement and queue mutations
pub fn handle_local_token_movement(
    mut query: Query<(&mut Token, &Transform), Changed<Transform>>,
    mut mutation_queue: ResMut<GraphQLMutationQueue>,
) {
    for (mut token, transform) in query.iter_mut() {
        // Convert transform translation to grid coordinates
        let new_x = transform.translation.x as i32;
        let new_y = transform.translation.y as i32;

        // Detect if position changed
        if token.base_x != new_x || token.base_y != new_y {
            eprintln!(
                "[Phase4.9.D🎮] Local movement detected: token={}, new_pos=({}, {})",
                token.id, new_x, new_y
            );

            // Queue mutation to server
            mutation_queue.push_move_token(token.id.clone(), new_x, new_y);

            // Update local cache
            token.base_x = new_x;
            token.base_y = new_y;
        }
    }
}

/// System: Check mutation timeouts and initiate rollback
pub fn check_mutation_timeouts(
    mut mutation_queue: ResMut<GraphQLMutationQueue>,
    mut query: Query<(&mut Transform, &RollbackState)>,
    time: Res<Time>,
) {
    let timed_out = mutation_queue.check_timeouts(time.elapsed_secs() as f64);

    for mutation_id in timed_out {
        eprintln!("[Phase4.9.D❌] Mutation timeout: {}", mutation_id);

        // Find corresponding token and rollback
        for (mut transform, rollback) in query.iter_mut() {
            transform.translation =
                Vec3::new(rollback.original_x as f32, rollback.original_y as f32, 0.0);

            eprintln!(
                "[Phase4.9.D🔄] Rolled back token to: ({}, {})",
                rollback.original_x, rollback.original_y
            );
        }
    }
}

/// Resource: Queue of mutations to send to server
#[derive(Resource, Default)]
pub struct GraphQLMutationQueue {
    pending: std::collections::HashMap<String, PendingMutationInfo>,
}

#[derive(Clone, Debug)]
pub struct PendingMutationInfo {
    pub mutation_id: String,
    pub token_id: String,
    pub x: i32,
    pub y: i32,
    pub sent_at: f64,
    pub timeout_secs: f64,
}

impl GraphQLMutationQueue {
    pub fn new() -> Self {
        Self {
            pending: std::collections::HashMap::new(),
        }
    }

    /// Queue a moveToken mutation
    pub fn push_move_token(&mut self, token_id: String, x: i32, y: i32) {
        let mutation_id = next_mutation_id();

        eprintln!(
            "[Phase4.9.D📤] Queued mutation: id={}, token={}, pos=({}, {})",
            mutation_id, token_id, x, y
        );

        self.pending.insert(
            mutation_id.clone(),
            PendingMutationInfo {
                mutation_id,
                token_id,
                x,
                y,
                sent_at: 0.0,
                timeout_secs: 5.0,
            },
        );
    }

    /// Get all pending mutations
    pub fn get_pending(&self) -> Vec<PendingMutationInfo> {
        self.pending.values().cloned().collect()
    }

    /// Mark mutation as complete (success or timeout)
    pub fn mark_complete(&mut self, mutation_id: &str) {
        if let Some(info) = self.pending.remove(mutation_id) {
            eprintln!(
                "[Phase4.9.D✅] Mutation completed: id={}, token={}",
                info.mutation_id, info.token_id
            );
        }
    }

    /// Check for timed-out mutations
    pub fn check_timeouts(&mut self, current_time: f64) -> Vec<String> {
        let mut timed_out = Vec::new();

        for (mutation_id, info) in self.pending.iter() {
            if info.sent_at > 0.0 && current_time - info.sent_at > info.timeout_secs {
                timed_out.push(mutation_id.clone());
            }
        }

        for mutation_id in &timed_out {
            self.mark_complete(mutation_id);
        }

        timed_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_state_creation() {
        let rollback = RollbackState {
            original_x: 100,
            original_y: 200,
        };

        assert_eq!(rollback.original_x, 100);
        assert_eq!(rollback.original_y, 200);
    }

    #[test]
    fn test_pending_mutation_creation() {
        let mutation = PendingMutation {
            mutation_id: "mut-1".to_string(),
            sent_at: 1.0,
            timeout_secs: 5.0,
        };

        assert_eq!(mutation.mutation_id, "mut-1");
        assert_eq!(mutation.timeout_secs, 5.0);
    }

    #[test]
    fn test_mutation_queue_operations() {
        let mut queue = GraphQLMutationQueue::new();

        queue.push_move_token("token-1".to_string(), 100, 200);
        queue.push_move_token("token-2".to_string(), 150, 250);

        // `pending` is a `HashMap` drained into a `Vec`, so it comes back in
        // map order. This used to index it positionally and assert
        // `pending[0]` was "token-1" — a property the queue does not have,
        // and the first thing to fail once the suite could actually run.
        let pending = queue.get_pending();
        assert_eq!(pending.len(), 2);
        let first = pending
            .iter()
            .find(|info| info.token_id == "token-1")
            .expect("token-1 queued");
        assert_eq!(first.x, 100);
        assert_eq!(
            pending
                .iter()
                .find(|info| info.token_id == "token-2")
                .expect("token-2 queued")
                .x,
            150
        );

        queue.mark_complete(&first.mutation_id);
        let remaining = queue.get_pending();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].token_id, "token-2");
    }

    #[test]
    fn test_mutation_id_generation() {
        let id1 = next_mutation_id();
        let id2 = next_mutation_id();

        assert_ne!(id1, id2);
        assert!(id1.starts_with("mut-"));
        assert!(id2.starts_with("mut-"));
    }
}
