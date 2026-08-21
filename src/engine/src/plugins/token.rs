use bevy::prelude::*;

pub struct TokenPlugin;

impl Plugin for TokenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, initialize_tokens);
    }
}

#[derive(Component)]
pub struct TokenRenderer;

fn initialize_tokens(mut commands: Commands) {
    // Initialize with empty cache (will be populated by RxDB)
    // SceneData is loaded by ScenePlugin before Update systems run
    commands.insert_resource(crate::systems::token_loader::TokenCache(vec![]));
}
