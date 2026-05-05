use bevy::prelude::*;
use crate::resources::SelectedToken;
use crate::systems::selection;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SelectedToken>()
            .add_systems(Update, selection::handle_token_selection)
            .add_systems(Update, selection::render_selection_feedback)
            .add_systems(Update, selection::handle_keyboard_token_movement);  // Phase 4.7.E2
    }
}
