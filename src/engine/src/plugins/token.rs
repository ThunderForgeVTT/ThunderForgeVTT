use bevy::prelude::*;

use crate::resources::{DraggingToken, IsGameMaster, SelectedToken};
use crate::systems::token::{
    handle_token_drag, handle_token_resize_drag, handle_token_resize_rotate_keyboard,
    handle_token_rotate_drag, init_token_systems_resources, sync_token_visuals,
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
            .init_resource::<IsGameMaster>();

        init_token_systems_resources(app);

        app.add_systems(Startup, initialize_tokens);

        app.add_systems(
            Update,
            (
                handle_token_resize_drag,
                handle_token_rotate_drag,
                handle_token_drag,
                handle_token_resize_rotate_keyboard,
                sync_token_visuals,
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
