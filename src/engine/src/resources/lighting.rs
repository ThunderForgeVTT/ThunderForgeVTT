//! Bevy `Resource` shell around `thunderforge_canvas_core::lighting`'s pure
//! data. The engine crate only targets `wasm32-unknown-unknown` and has no
//! wasm-bindgen-test-runner configured here, so its own tests only ever
//! compile-check, never run — the actual light-set logic tests live in
//! `thunderforge_canvas_core` instead, where they execute for real via
//! native `cargo test`. Keep this file to Bevy plumbing only; put logic in
//! the core crate. Mirrors `src/engine/src/resources/wall.rs` exactly.

use bevy::prelude::*;
use std::ops::{Deref, DerefMut};

pub use thunderforge_canvas_core::lighting::{LightEdit, LightSource};

/// Currently selected light id (mirrors `resources::wall::SelectedWall`).
/// Only one light can be selected at a time.
#[derive(Resource, Default)]
pub struct SelectedLight(pub Option<String>);

impl SelectedLight {
    pub fn select(&mut self, light_id: String) {
        self.0 = Some(light_id);
    }

    pub fn deselect(&mut self) {
        self.0 = None;
    }

    pub fn is_selected(&self, light_id: &str) -> bool {
        self.0.as_deref() == Some(light_id)
    }

    pub fn get_selected(&self) -> Option<&String> {
        self.0.as_ref()
    }
}

/// Bevy `Resource` newtype over the engine-agnostic `LightSet` core type.
/// `Deref`/`DerefMut` make this transparent to existing call sites
/// (`light_set.lights()`, `light_set.upsert(...)`, etc. all keep working
/// unchanged) while Bevy's `ResMut<LightSet>` change detection still
/// applies normally — it only cares that *this* type is a `Resource`, not
/// what's inside it. Mirrors `resources::wall::WallSet` exactly.
#[derive(Resource, Default)]
pub struct LightSet(pub thunderforge_canvas_core::lighting::LightSet);

impl Deref for LightSet {
    type Target = thunderforge_canvas_core::lighting::LightSet;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LightSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
