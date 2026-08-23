//! GraphQL query modules organized by domain.
//!
//! Each module contains a Query struct that implements a set of related queries.
//! The main graphql.rs merges all queries into the QueryRoot for the GraphQL schema.

pub mod actor;
pub mod admin;
pub mod healthcheck;
pub mod scene;
pub mod user;
pub mod invite;
pub mod item;
pub mod inventory;

pub use actor::ActorQuery;
pub use admin::AdminQuery;
pub use healthcheck::HealthcheckQuery;
pub use scene::SceneQuery;
pub use user::UserQuery;
pub use invite::InviteQuery;
pub use item::ItemQuery;
pub use inventory::InventoryQuery;
