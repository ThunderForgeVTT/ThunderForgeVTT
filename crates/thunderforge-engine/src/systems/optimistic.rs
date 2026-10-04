//! Optimistic update system with correlation and rollback - Phase 4.6
//!
//! Handles entity-level mutation tracking, result correlation,
//! and automatic rollback on server rejection.

use bevy::prelude::*;

use crate::components::{GridPosition, RollbackCache, TokenId};
use crate::sync_test::CircularFlowTracer;

/// Component marking a token with a pending optimistic update
#[derive(Component, Clone, Debug)]
pub struct PendingMutation {
    /// Unique mutation ID (for correlation with server response)
    pub mutation_id: u64,
    /// Position before the mutation (for rollback)
    pub rollback_position: GridPosition,
}

/// Helper to mark a mutation as pending on an entity
pub fn mark_mutation_pending(
    commands: &mut Commands,
    entity: Entity,
    mutation_id: u64,
    current_position: GridPosition,
) {
    commands.entity(entity).insert(PendingMutation {
        mutation_id,
        rollback_position: current_position,
    });
}

/// System to process mutation results and apply confirmations/rollbacks
///
/// Phase 4.6: This system will be enhanced to:
/// 1. Listen for ServerEvent triggers
/// 2. Correlate results to entities via PendingMutation component
/// 3. Confirm or rollback GridPosition
pub fn process_mutation_results(
    mut _commands: Commands,
    mut _tracer: ResMut<CircularFlowTracer>,
    mut _query: Query<(
        Entity,
        &PendingMutation,
        &mut GridPosition,
        &TokenId,
        &RollbackCache,
    )>,
) {
    // Phase 4.6.1: Implement with ServerEvent listener
    // for (entity, pending, mut grid_pos, token_id, cache) in query.iter_mut() {
    //     // Correlate results and handle confirmation/rollback
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_mutation_creation() {
        let pending = PendingMutation {
            mutation_id: 42,
            rollback_position: GridPosition::new(10.0, 20.0, 0.0),
        };

        assert_eq!(pending.mutation_id, 42);
        assert_eq!(pending.rollback_position.x, 10.0);
    }
}
