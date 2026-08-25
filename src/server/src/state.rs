use crate::config::{Config, Directories};
use crate::models::WorldEvent;
use crate::system_hooks::SystemHookRegistry;
use axum::extract::FromRef;
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
    pub presence_sender: Sender<serde_json::Value>, // Phase 4.9.B.3: Presence changes
    pub key: Key,
    pub db_pool: DbPool,
    pub system_hooks: std::sync::Arc<tokio::sync::RwLock<SystemHookRegistry>>,
    // Spec 024, ADR-047: which `SessionAdjudicator` implementation is active
    // (`LocalAdjudicator` by default, or `RemoteAdjudicator` when
    // `CRUCIBLE_MODE=remote` — selected once at startup in `main.rs`).
    pub adjudicator: std::sync::Arc<dyn thunderforge_crucible::SessionAdjudicator + Send + Sync>,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}
