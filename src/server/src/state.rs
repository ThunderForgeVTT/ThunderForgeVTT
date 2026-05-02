use crate::config::{Config, Directories};
use axum::extract::FromRef;
use tokio::sync::broadcast::Sender;
use tower_cookies::Key;
use thunderforge_core::events::WorldEvent;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub directories: Directories,
    pub world_event_sender: Sender<WorldEvent>,
    pub key: Key,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}
