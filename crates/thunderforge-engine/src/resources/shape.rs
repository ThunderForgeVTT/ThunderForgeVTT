//! Bevy `Resource` shell around `thunderforge_canvas_core::shape`'s pure
//! data. The engine crate only targets `wasm32-unknown-unknown` and has no
//! wasm-bindgen-test-runner configured here, so its own tests only ever
//! compile-check, never run — the actual shape logic tests live in
//! `thunderforge_canvas_core` instead, where they execute for real via
//! native `cargo test`. Keep this file to Bevy plumbing only; put logic in
//! the core crate.

use bevy::prelude::*;
use std::ops::{Deref, DerefMut};

pub use thunderforge_canvas_core::shape::{Shape, ShapeEdit, ShapeKind};

/// Currently selected shape id (mirrors `resources/wall.rs`'s
/// `SelectedWall` pattern). Only one shape can be selected at a time.
#[derive(Resource, Default)]
pub struct SelectedShape(pub Option<String>);

impl SelectedShape {
    pub fn select(&mut self, shape_id: String) {
        self.0 = Some(shape_id);
    }

    pub fn deselect(&mut self) {
        self.0 = None;
    }

    pub fn is_selected(&self, shape_id: &str) -> bool {
        self.0.as_deref() == Some(shape_id)
    }

    pub fn get_selected(&self) -> Option<&String> {
        self.0.as_ref()
    }
}

/// The shape tool currently selected by the GM in the toolbar (mirrors
/// `resources/selection.rs`'s selection-state resources). `None` = no
/// draw tool active (plain select/move mode).
#[derive(Resource, Default)]
pub struct ActiveShapeTool(pub Option<ShapeKind>);

/// Bevy `Resource` newtype over the engine-agnostic `ShapeSet` core type.
/// `Deref`/`DerefMut` make this transparent to existing call sites
/// (`shape_set.shapes()`, `shape_set.upsert(...)`, etc. all keep working
/// unchanged) while Bevy's `ResMut<ShapeSet>` change detection still
/// applies normally — it only cares that *this* type is a `Resource`, not
/// what's inside it.
#[derive(Resource, Default)]
pub struct ShapeSet(pub thunderforge_canvas_core::shape::ShapeSet);

impl Deref for ShapeSet {
    type Target = thunderforge_canvas_core::shape::ShapeSet;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ShapeSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
