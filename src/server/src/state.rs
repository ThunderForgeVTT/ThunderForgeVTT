use crate::config::{Config, Directories};
use crate::models::WorldEvent;
use axum::extract::FromRef;
use thunderforge_pg_sockets::SharedWorldRouter;
use tokio::sync::broadcast::Sender;
use tower_cookies::Key;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub directories: Directories,
    /// Per-world event fan-out (`thunderforge_pg_sockets::WorldRouter`).
    ///
    /// Was a single `Sender<WorldEvent>` that every subscriber in the process
    /// shared, filtering by world itself — which delivered every event to
    /// every connected client and cost O(total connections) per event. The
    /// router delivers to one world's subscribers and nobody else.
    pub world_events: SharedWorldRouter<WorldEvent>,
    pub presence_sender: Sender<serde_json::Value>, // Phase 4.9.B.3: Presence changes
    /// Who is at each table right now.
    ///
    /// In memory rather than in Postgres. Every connected client beats every
    /// five seconds, and writing that to a table meant ~1,200 writes per
    /// second at a thousand tables — six times what play itself generates, all
    /// of it describing a fact that is worthless one beat later. See
    /// `thunderforge_presence` for why in-memory is also the more correct
    /// answer rather than merely the cheaper one.
    pub presence: std::sync::Arc<thunderforge_presence::PresenceRegistry>,
    pub key: Key,
    pub db_pool: DbPool,
    // Spec 024, ADR-047: which `SessionAdjudicator` implementation is active
    // (`LocalAdjudicator` by default, or `RemoteAdjudicator` when
    // `CRUCIBLE_MODE=remote` — selected once at startup in `main.rs`).
    pub adjudicator: std::sync::Arc<dyn thunderforge_crucible::SessionAdjudicator + Send + Sync>,
    /// Spec 040 US4: where a message goes when this instance sends one.
    ///
    /// Empty in production — the transport is built from the `mail.*` settings
    /// as they resolve for each send, so correcting an SMTP host takes effect
    /// without a restart (FR-007). A test puts one implementation in it and
    /// asserts what would have been sent without sending it. See
    /// `mail::MailSeam` for why this is not an `Arc<dyn MailTransport>` fixed
    /// at startup.
    pub mail: crate::mail::MailSeam,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}
