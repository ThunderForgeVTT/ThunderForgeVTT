//! The ThunderForge server, as a library.
//!
//! # Why this is a library and `src/app` is the binary
//!
//! A system pack must be able to own the tables it writes and contribute its
//! own GraphQL (spec 032 FR-004, ADR-063). That means a pack crate has to
//! depend on this code — for `AppState`, `is_dm_of_world`, `record_world_event`
//! and the shared models and schema — and it could not, because this crate was
//! binary-only and nothing can depend on a binary.
//!
//! Inverting it costs almost nothing. The whole server-to-pack coupling was
//! seven `use <pack> as _;` lines and seven Cargo entries; those moved to
//! `src/app`, the binary, which is the composition root: it links the packs,
//! merges their GraphQL into the roots, and runs `main`. Nothing here knows a
//! pack exists.
//!
//! See `specs/032-pack-architecture/research.md` § F-5.

// Same reason `main.rs` carries it: async-graphql's `MergedObject` dispatch
// nests one level deeper per merged root member, and the default 128-deep
// type-layout recursion limit is not enough for this many.
#![recursion_limit = "512"]

pub mod ability_vocabulary;
pub mod actor_assets_serve;
pub mod adapters;
pub mod admin;
pub mod attributes;
pub mod auth;
pub mod auth_middleware;
pub mod canvas_assets_serve;
pub mod collections;
pub mod config;
pub mod crypto;
pub mod db_types;
pub mod declared_values;
pub mod door_effects;
pub mod errors;
pub mod graphql;
pub mod instance_identity;
pub mod interaction;
pub mod interface_packs;
pub mod light_effects;
pub mod lore_assets_serve;
pub mod lore_sync;
pub mod map_import;
pub mod markdown;
pub mod models;
pub mod moderation;
pub mod network;
pub mod peer_signaling;
pub mod pubsub;
pub mod repo_host;
pub mod scene_assets_serve;
pub mod scene_fingerprint;
pub mod schema;
pub mod serve;
pub mod session;
pub mod sheet;
pub mod state;
pub mod status_display;
pub mod storage;
pub mod systems;
#[cfg(test)]
pub mod test_packs;
/// Shared database fixtures.
///
/// Compiled for this crate's own tests, and for anyone who turns on
/// `test-support` — which the system packs' dev-dependencies do, because a
/// pack that owns its tables tests them against a real database the same way
/// this crate always has.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod turn_structure;
pub mod users;
pub mod utils;
pub mod world;
pub mod world_events;
pub mod world_hooks;

/// Modules reach this as `crate::AppState`, which `main.rs` used to provide at
/// its own crate root.
pub use state::AppState;
