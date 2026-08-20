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

pub mod wall;
