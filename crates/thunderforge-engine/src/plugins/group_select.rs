//! Several things selected at once (spec 085): the group, the box that makes
//! one, and the move and delete that act on every member.

use bevy::prelude::*;

use crate::plugins::authoring_mode::AuthoringMode;
use crate::resources::{
    GroupSelection, IsGameMaster, SelectedLight, SelectedShape, SelectedToken, SelectedWall,
};
use crate::systems::box_select::{
    BoxDrag, draw_box, follow_single_selection, handle_box_select, publish_selection,
};
use crate::systems::group_move::{
    GroupDrag, GroupStamps, delete_group_selection, drive_group_drag, start_group_drag,
};
use crate::systems::token::handle_token_drag;

pub struct GroupSelectPlugin;

impl Plugin for GroupSelectPlugin {
    fn build(&self, app: &mut App) {
        // The singles are each their own tool's, and initialising one that
        // already exists keeps it.
        app.init_resource::<GroupSelection>()
            .init_resource::<BoxDrag>()
            .init_resource::<GroupDrag>()
            .init_resource::<GroupStamps>()
            .init_resource::<crate::payloads::ActiveWorld>()
            .init_resource::<IsGameMaster>()
            .init_resource::<SelectedToken>()
            .init_resource::<SelectedWall>()
            .init_resource::<SelectedLight>()
            .init_resource::<SelectedShape>()
            .add_systems(
                Update,
                handle_box_select
                    .run_if(in_state(AuthoringMode::Select))
                    // Before the token drag, so a press the group owns is
                    // decided before the drag picks anything up.
                    .before(handle_token_drag)
                    .after(crate::app::ExternalCommandsApplied),
            )
            .add_systems(
                Update,
                (start_group_drag, drive_group_drag, delete_group_selection)
                    .chain()
                    .run_if(in_state(AuthoringMode::Select))
                    .after(handle_box_select)
                    .before(handle_token_drag),
            )
            // After every tool has had its say this frame.
            .add_systems(
                PostUpdate,
                (
                    crate::systems::token::release_disowned_tokens,
                    follow_single_selection,
                    publish_selection,
                )
                    .chain(),
            );

        if app.is_plugin_added::<bevy::gizmos::GizmoPlugin>() {
            app.add_systems(Update, draw_box.run_if(in_state(AuthoringMode::Select)));
        }
    }
}
