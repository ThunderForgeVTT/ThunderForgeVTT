use crate::resources::{DraggingToken, IsGameMaster, SelectedToken};
use crate::systems::selection;
use bevy::prelude::*;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedToken>()
            .init_resource::<DraggingToken>()
            // Idempotent: WallPlugin/ShapePlugin also init this (see their
            // own comments) — whichever plugin builds first wins, matching
            // the existing graceful-multi-init convention.
            .init_resource::<IsGameMaster>()
            .add_systems(Update, selection::render_selection_feedback)
            .add_systems(Update, selection::handle_keyboard_token_movement); // Phase 4.7.E2

        // The selection ring. After every Update system, so it is drawn where
        // this frame's drag and snap left the token. Only where something
        // draws gizmos: a headless app built from `MinimalPlugins` has none,
        // and this plugin must still build there.
        if app.is_plugin_added::<bevy::gizmos::GizmoPlugin>() {
            app.add_systems(PostUpdate, selection::draw_selection_rings);
        }
    }
}
