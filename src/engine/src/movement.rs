//! Simplified Movement System

use bevy::prelude::*;
use crate::components::*;
use crate::network::mutations::{execute_move_token_mutation, MutationTracker};
use crate::resources::{GridType, SceneData};
use crate::systems::optimistic::mark_mutation_pending;
use crate::sync_test::{CircularFlowTracer, FlowStage};

/// Token is player-controlled and can be moved interactively
#[derive(Component)]
pub struct PlayerControlled;

/// Handle keyboard input for testing movement
/// WASD or arrow keys to move first player-controlled token
/// Also queues mutations to the server
pub fn handle_keyboard_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut tracer: ResMut<CircularFlowTracer>,
    tracker: Res<MutationTracker>,
    mut query: Query<(Entity, &mut GridPosition, &TokenId, &PlayerControlled)>,
) {
    let mut direction = Vec2::ZERO;
    
    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    
    if direction == Vec2::ZERO {
        return;
    }
    
    let step_size = 32.0; // Move in 32-unit steps (typical grid cell)
    let delta = direction.normalize() * step_size;
    
    for (entity, mut grid_pos, token_id, _) in query.iter_mut() {
        let old_x = grid_pos.x;
        let old_y = grid_pos.y;
        let old_z = grid_pos.z;

        // 1. Update GridPosition optimistically
        grid_pos.x += delta.x;
        grid_pos.y += delta.y;
        
        eprintln!("🎮 Moved token {} to ({:.1}, {:.1})", token_id.0, grid_pos.x, grid_pos.y);
        tracer.trace(
            FlowStage::LocalInput,
            token_id.0.clone(),
            format!("Move input: ({:.1}, {:.1})", grid_pos.x, grid_pos.y),
            0.0,
        );

        // 2. Queue mutation to server
        let mutation_id = execute_move_token_mutation(
            &tracker,
            "localhost:8080",
            "default".to_string(),
            token_id.0.clone(),
            grid_pos.x,
            grid_pos.y,
            grid_pos.z,
        );

        // 3. Mark as pending (for correlation later)
        mark_mutation_pending(
            &mut commands,
            entity,
            mutation_id,
            GridPosition::new(old_x, old_y, old_z),
        );

        tracer.trace(
            FlowStage::MutationSent,
            token_id.0.clone(),
            format!("Mutation queued: id={}", mutation_id),
            0.0,
        );

        break; // Only move first player-controlled token
    }
}

/// Sync Transform from GridPosition
/// Bevy uses Transform for rendering, but GridPosition is the source of truth
/// This system keeps Transform in sync with GridPosition
pub fn sync_grid_to_transform(
    mut query: Query<(&GridPosition, &mut Transform), Changed<GridPosition>>,
) {
    for (grid_pos, mut transform) in query.iter_mut() {
        transform.translation.x = grid_pos.x;
        transform.translation.y = grid_pos.y;
        transform.translation.z = grid_pos.z;
    }
}

/// Sync GridPosition from Transform (if needed for external updates)
/// This is a fallback for if Transform is modified directly
pub fn sync_transform_to_grid(
    mut query: Query<(&Transform, &mut GridPosition), Changed<Transform>>,
) {
    for (transform, mut grid_pos) in query.iter_mut() {
        grid_pos.x = transform.translation.x;
        grid_pos.y = transform.translation.y;
        grid_pos.z = transform.translation.z;
    }
}

/// Apply grid snapping to token positions
/// This rounds positions to the nearest grid cell.
///
/// Gridless scenes (`GridType::Gridless`, e.g. Genie's Wish-Warped Zone)
/// have no fixed cell size to snap to, so this system is a no-op for them
/// — tokens keep whatever free-form position they were moved to. When
/// `SceneData` hasn't been inserted yet (see `plugins::grid`'s identical
/// `Option<Res<SceneData>>` degradation), snapping is skipped entirely
/// rather than assuming a topology.
pub fn apply_grid_snapping(
    scene: Option<Res<SceneData>>,
    mut query: Query<(&mut GridPosition, &RollbackCache), Changed<GridPosition>>,
) {
    let Some(scene) = scene else {
        return;
    };

    if scene.grid_type == GridType::Gridless {
        return;
    }

    let grid_size = 32.0; // Default grid size in pixels

    for (mut grid_pos, cache) in query.iter_mut() {
        if !cache.is_pending {
            // Snap to grid (only if not waiting for server)
            grid_pos.x = (grid_pos.x / grid_size).round() * grid_size;
            grid_pos.y = (grid_pos.y / grid_size).round() * grid_size;
        }
    }
}
