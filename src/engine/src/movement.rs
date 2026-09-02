//! Simplified Movement System

use crate::components::*;
use crate::network::mutations::{MutationTracker, execute_move_token_mutation};
use crate::resources::SceneGrid;
use crate::sync_test::{CircularFlowTracer, FlowStage};
use crate::systems::optimistic::mark_mutation_pending;
use bevy::prelude::*;

/// Token is player-controlled and can be moved interactively.
///
/// # There were two of these
///
/// `components.rs` declared a second `PlayerControlled`, and nothing used it —
/// `lib.rs`'s spawn, `token_move.rs`'s query and `sync_test.rs` all took this
/// one. In Bevy they are different component types, so a query on that one
/// would never have matched an entity tagged with this one: dead rather than
/// broken, but only by luck.
///
/// It survived because this module is browser-bound and this crate's tests
/// have never compiled on a host, so nothing that could see both was ever
/// built. Found while investigating why (spec 032 T083).
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
    grid: Option<Res<SceneGrid>>,
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

    // One cell per press, using the scene's real cell size rather than the
    // 32.0 this used to assume — on an imported 128px map that moved a token
    // a quarter of a cell at a time, and `apply_grid_snapping` then pulled it
    // back, so it looked like the keys did nothing.
    let step_size = grid.as_ref().map_or(32.0, |g| g.size);
    let delta = direction.normalize() * step_size;

    // `.next()` rather than a `for` that breaks at the bottom. The loop said
    // "for every token" and then contradicted itself twenty lines later,
    // which is why clippy called it a loop that never loops — and a reader
    // had to reach the `break` to learn the rule. Only the first
    // player-controlled token moves; this says so at the point it is decided.
    if let Some((entity, mut grid_pos, token_id, _)) = query.iter_mut().next() {
        let old_x = grid_pos.x;
        let old_y = grid_pos.y;
        let old_z = grid_pos.z;

        // 1. Update GridPosition optimistically
        grid_pos.x += delta.x;
        grid_pos.y += delta.y;

        eprintln!(
            "🎮 Moved token {} to ({:.1}, {:.1})",
            token_id.0, grid_pos.x, grid_pos.y
        );
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

/// Snaps token positions to the active grid.
///
/// Bug fix: this used a hardcoded `grid_size = 32.0` and consulted
/// `SceneData` only to ask whether the scene was gridless. So on a dd2vtt
/// import — which records the file's own `pixels_per_grid`, typically 128 —
/// tokens snapped to a 32-unit lattice that matched neither the visible grid
/// nor the map art beneath it. It now snaps through `SceneGrid`, the same
/// resource `plugins::grid` draws from, so what you see and what you snap to
/// are the same lattice by construction.
///
/// Gridless scenes are handled inside `GridSpec::snap`, which returns the
/// position untouched rather than inventing a lattice.
pub fn apply_grid_snapping(
    grid: Option<Res<SceneGrid>>,
    mut query: Query<(&mut GridPosition, &RollbackCache), Changed<GridPosition>>,
) {
    let Some(grid) = grid else {
        return;
    };

    for (mut grid_pos, cache) in query.iter_mut() {
        // A token awaiting server confirmation keeps its optimistic position:
        // snapping it now would fight the authoritative value on arrival.
        if cache.is_pending {
            continue;
        }

        let snapped = grid.snap(Vec2::new(grid_pos.x, grid_pos.y));
        grid_pos.x = snapped.x;
        grid_pos.y = snapped.y;
    }
}
