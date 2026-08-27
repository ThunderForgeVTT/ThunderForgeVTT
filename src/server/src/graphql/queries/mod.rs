//! GraphQL query modules organized by domain.
//!
//! Each module contains a Query struct that implements a set of related queries.
//! The main graphql.rs merges all queries into the QueryRoot for the GraphQL schema.

pub mod ability;
pub mod actor;
pub mod admin;
pub mod genie_session;
pub mod healthcheck;
pub mod inventory;
pub mod invite;
pub mod item;
pub mod lore;
pub mod moderation;
pub mod roll;
pub mod scene;
pub mod user;

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
