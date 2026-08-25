//! GraphQL query modules organized by domain.
//!
//! Each module contains a Query struct that implements a set of related queries.
//! The main graphql.rs merges all queries into the QueryRoot for the GraphQL schema.

pub mod actor;
pub mod admin;
pub mod healthcheck;
pub mod lore;
pub mod scene;
pub mod user;
pub mod invite;
pub mod ability;
pub mod item;
pub mod inventory;
pub mod moderation;
pub mod roll;
pub mod genie_session;

pub use actor::ActorQuery;
pub use admin::AdminQuery;
pub use healthcheck::HealthcheckQuery;
pub use lore::LoreQuery;
pub use scene::SceneQuery;
pub use user::UserQuery;
pub use invite::InviteQuery;
pub use ability::AbilityQuery;
pub use item::ItemQuery;
pub use inventory::InventoryQuery;
pub use moderation::ModerationQuery;
pub use roll::RollQuery;
pub use genie_session::GenieSessionQuery;
