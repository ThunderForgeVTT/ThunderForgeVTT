use bevy::prelude::*;

pub struct TokenPlugin;

impl Plugin for TokenPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, initialize_tokens)
            .add_systems(Update, crate::systems::token_sync::sync_tokens_from_rxdb);
    }
}

#[derive(Component)]
pub struct TokenRenderer;

fn initialize_tokens(mut commands: Commands) {
    // Initialize with empty cache (will be populated by RxDB)
    // SceneData is loaded by ScenePlugin before Update systems run
    commands.insert_resource(crate::systems::token_loader::TokenCache(vec![]));
}
