//! Simplified Movement System

use bevy::prelude::*;
use crate::components::*;

/// Token is player-controlled and can be moved interactively
#[derive(Component)]
pub struct PlayerControlled;

/// Handle keyboard input for testing movement
/// WASD or arrow keys to move first player-controlled token
pub fn handle_keyboard_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut GridPosition, &PlayerControlled)>,
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
    
    for (mut grid_pos, _) in query.iter_mut() {
        grid_pos.x += delta.x;
        grid_pos.y += delta.y;
        eprintln!("Moved token to ({:.1}, {:.1})", grid_pos.x, grid_pos.y);
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
/// This rounds positions to the nearest grid cell
pub fn apply_grid_snapping(
    mut query: Query<(&mut GridPosition, &RollbackCache), Changed<GridPosition>>,
) {
    let grid_size = 32.0; // Default grid size in pixels
    
    for (mut grid_pos, cache) in query.iter_mut() {
        if !cache.is_pending {
            // Snap to grid (only if not waiting for server)
            grid_pos.x = (grid_pos.x / grid_size).round() * grid_size;
            grid_pos.y = (grid_pos.y / grid_size).round() * grid_size;
        }
    }
}
