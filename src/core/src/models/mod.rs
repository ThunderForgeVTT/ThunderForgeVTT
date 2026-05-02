//! Shared domain models for ThunderForgeVTT
//! 
//! These types are serializable and contain no Diesel dependencies.
//! They serve as the canonical data shapes for:
//! - Server persistence (adapted to/from Diesel models)
//! - Engine state management (Bevy components)
//! - Frontend state (RxDB collections)
//! 
//! All types must be serializable with serde to support GraphQL transport.

pub mod auth;
pub mod world;
pub mod errors;
pub mod version;

pub use auth::*;
pub use world::*;
pub use errors::*;
pub use version::*;
