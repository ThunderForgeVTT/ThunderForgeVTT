//! Several things selected at once (spec 085): the group, the box that makes
//! one, and the move and delete that act on every member.

use bevy::prelude::*;

use crate::resources::GroupSelection;

pub struct GroupSelectPlugin;

impl Plugin for GroupSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroupSelection>();
    }
}
