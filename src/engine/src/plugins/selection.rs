use bevy::prelude::*;
use crate::resources::{SelectedToken, DraggingToken, IsGameMaster};
use crate::systems::selection;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SelectedToken>()
            .init_resource::<DraggingToken>()
            // Idempotent: WallPlugin/ShapePlugin also init this (see their
            // own comments) — whichever plugin builds first wins, matching
            // the existing graceful-multi-init convention.
            .init_resource::<IsGameMaster>()
            .add_systems(Update, selection::render_selection_feedback)
            .add_systems(Update, selection::handle_keyboard_token_movement); // Phase 4.7.E2
    }
}
