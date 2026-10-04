use crate::resources::CanvasLayers;
use bevy::prelude::*;

/// Registers the shared `CanvasLayers` resource (FR-016) that every other
/// canvas-authoring plugin (walls, lighting, shapes, map import's
/// background art) renders into. Must be added before those plugins so
/// the resource exists when they build; it has no systems of its own.
pub struct CanvasLayerPlugin;

impl Plugin for CanvasLayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CanvasLayers>();
    }
}
