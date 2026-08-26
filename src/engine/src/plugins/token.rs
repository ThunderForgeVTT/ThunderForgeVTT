use bevy::prelude::*;

use crate::resources::{DraggingToken, GridSnapEnabled, IsGameMaster, SelectedToken};
use crate::systems::token::{
    handle_token_drag, handle_token_resize_drag, handle_token_resize_rotate_keyboard,
    handle_token_rotate_drag, init_token_systems_resources, sync_token_visuals,
};
use crate::systems::token_grid::{size_tokens_to_grid, snap_tokens_to_grid};
use crate::systems::token_move::{
    MovementPlan, SceneUnits, draw_movement_plan, handle_token_movement_input,
};

/// Wires up token authoring (spec 006, closing out spec 004 US2's
/// keyboard-shortcut stand-in): click-drag body movement, GM-only
/// resize/rotate canvas handles (drag-driven, mirroring `WallPlugin`'s
/// input/undo/sync shape per research.md §1), the legacy keyboard shortcuts
/// kept as a secondary path, and the handle-sprite sync render pass.
///
/// Independently addable/removable per Constitution Principle II, mirroring
/// `WallPlugin`'s own stated contract.
pub struct TokenPlugin;

impl Plugin for TokenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedToken>()
            .init_resource::<DraggingToken>()
            // Idempotent, same graceful-multi-init convention as
            // `SelectionPlugin`/`WallPlugin`/`ShapePlugin`.
            .init_resource::<IsGameMaster>()
            // Grid-locked by default; `set_grid_snap` turns it off.
            .init_resource::<GridSnapEnabled>()
            // Movement planning, and the units its cost is quoted in.
            .init_resource::<MovementPlan>()
            .init_resource::<SceneUnits>();

        init_token_systems_resources(app);

        app.add_systems(Startup, initialize_tokens);

        app.add_systems(
            Update,
            (
                handle_token_resize_drag,
                handle_token_rotate_drag,
                handle_token_drag,
                handle_token_resize_rotate_keyboard,
                // Before sizing/snapping, so a move resolves to its final cell
                // in the same frame the key was pressed.
                handle_token_movement_input,
                // Sizing before snapping: snapping depends on the footprint,
                // and both run after the input systems above so a drag is
                // resolved to its final cell within the same frame it ends.
                size_tokens_to_grid,
                snap_tokens_to_grid,
                sync_token_visuals,
                draw_movement_plan,
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct TokenRenderer;

fn initialize_tokens(mut commands: Commands) {
    // Initialize with empty cache (will be populated by RxDB)
    // SceneData is loaded by ScenePlugin before Update systems run
    commands.insert_resource(crate::systems::token_loader::TokenCache(vec![]));
}
