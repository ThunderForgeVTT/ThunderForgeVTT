//! Pure, engine-agnostic canvas-authoring logic (specs/001-bevy-canvas-authoring).
//!
//! This crate holds the data model and geometry/algorithm core for the
//! native canvas authoring feature (walls, and — as they're built —
//! lighting and shapes) with **no dependency on Bevy or wasm-bindgen**.
//! The payoff: this crate compiles to a native target and its tests
//! actually *execute* via plain `cargo test`, unlike the `thunderforge_engine`
//! crate, which only targets `wasm32-unknown-unknown` and has no
//! wasm-bindgen-test-runner configured in this environment — its tests
//! only ever compile-check, never run.
//!
//! `thunderforge_engine` wraps these types in thin Bevy `Resource`
//! newtypes (see `src/engine/src/resources/wall.rs`) rather than
//! reimplementing the logic — the engine crate is the ECS/rendering
//! shell, this crate is the tested core underneath it.

pub mod attributes;
pub mod camera;
pub mod frame_trace;
pub mod grid;
pub mod interaction;
pub mod item;
pub mod lighting;
pub mod lore_link;
pub mod measure;
pub mod movement;
pub mod movement_budget;
pub mod navigation;
pub mod party;
pub mod resource_display;
/// Spec 030, US7. A contributor that exists only to be added and removed —
/// see the module's own docs for why it is a feature rather than a test.
#[cfg(feature = "seam-probe")]
pub mod seam_probe;
pub mod shape;
pub mod snapping;
pub mod texture_budget;
pub mod token_art;
pub mod token_kind;
pub mod token_stack;
pub mod vision;
pub mod wall;
