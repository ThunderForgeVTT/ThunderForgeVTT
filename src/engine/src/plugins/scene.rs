use bevy::prelude::*;
use crate::resources::scene_data::{SceneData, GridType};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_scene_system);
    }
}

/// 🎮 Phase 4.7.A1: Load scene data on startup
///
/// For Phase 4.7.A1 (MVP), hardcode a test scene.
/// In Phase 4.7.A1.1 (future enhancement), integrate GraphQL to query from server.
fn load_scene_system(mut commands: Commands) {
    let scene_data = SceneData::new(
        "scene-test-1".to_string(),
        "world-test-1".to_string(),
        GridType::Square,
        32.0,
        100,
        100,
        None,  // No background image for MVP
    );

    // Phase 4.7: Scene loaded
    let msg = format!(
        "Loaded scene: {} ({}x{} cells, {}px grid)",
        scene_data.scene_id, scene_data.width, scene_data.height, scene_data.grid_size
    );
    // TODO: Integrate with Bevy logging system

    commands.insert_resource(scene_data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_plugin_builds() {
        let mut app = App::new();
        app.add_plugins(ScenePlugin);
        // If we get here, the plugin built successfully
    }
}
