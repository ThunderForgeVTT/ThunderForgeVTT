use crate::config::{Config, Directories};
use crate::system_hooks::SystemHookRegistry;
use axum::extract::FromRef;
use thunderforge_core::events::WorldEvent;
use tokio::sync::broadcast::Sender;
use tower_cookies::Key;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub directories: Directories,
    pub world_event_sender: Sender<WorldEvent>,
    pub key: Key,
    pub db_pool: DbPool,
    pub system_hooks: std::sync::Arc<tokio::sync::RwLock<SystemHookRegistry>>,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}
