//! GraphQL query modules organized by domain.
//!
//! Each module contains a Query struct that implements a set of related queries.
//! The main graphql.rs merges all queries into the QueryRoot for the GraphQL schema.

pub mod ability;
pub mod actor;
pub mod admin;
pub mod genie_session;
pub mod healthcheck;
pub mod interactives;
pub mod inventory;
pub mod invite;
pub mod item;
pub mod lore;
pub mod moderation;
pub mod roll;
pub mod scene;
pub mod token_attributes;
pub mod token_status;
pub mod user;
// The reconnect catch-up (`worldEventsSince`) — what a client missed while
// its socket was down, read from the durable record rather than the wire.
pub mod world_events_since;
// Spec 028: the client-cache delta-sync plan (`worldSyncPlan`).
pub mod world_sync_plan;

pub use ability::AbilityQuery;
pub use actor::ActorQuery;
pub use admin::AdminQuery;
pub use genie_session::GenieSessionQuery;
pub use healthcheck::HealthcheckQuery;
pub use inventory::InventoryQuery;
pub use invite::InviteQuery;
pub use item::ItemQuery;
pub use lore::LoreQuery;
pub use moderation::ModerationQuery;
pub use roll::RollQuery;
pub use scene::SceneQuery;
pub use user::UserQuery;
pub use world_events_since::WorldEventsSinceQuery;
pub use world_sync_plan::WorldSyncPlanQuery;
