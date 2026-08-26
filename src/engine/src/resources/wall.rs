//! Bevy `Resource` shell around `thunderforge_canvas_core::wall`'s pure
//! data/geometry. The engine crate only targets `wasm32-unknown-unknown`
//! and has no wasm-bindgen-test-runner configured here, so its own tests
//! only ever compile-check, never run — the actual wall geometry/logic
//! tests live in `thunderforge_canvas_core` instead, where they execute
//! for real via native `cargo test`. Keep this file to Bevy plumbing only;
//! put logic in the core crate.

use bevy::prelude::*;
use std::ops::{Deref, DerefMut};

pub use thunderforge_canvas_core::wall::{DoorState, Wall, WallEdit, is_visible};

/// Whether the local session is acting as GM (wall edit handles and tool
/// input are GM-only, per data-model.md's Canvas Layer section —
/// `CanvasLayer::Walls.editing_is_gm_only()` is already true). This crate
/// doesn't otherwise have a GM/player role concept, so this is a minimal
/// flag the wall systems gate on; real GM-only enforcement for player
/// sessions happens server-side (`mutations_walls.rs`) and in the React
/// shell (hiding the tool entirely) — this is just so the engine doesn't
/// assume every session is a GM.
#[derive(Resource, Default)]
pub struct IsGameMaster(pub bool);

/// Currently selected wall id (mirrors `resources/selection.rs`'s
/// `SelectedToken` pattern). Only one wall can be selected at a time.
#[derive(Resource, Default)]
pub struct SelectedWall(pub Option<String>);

impl SelectedWall {
    pub fn select(&mut self, wall_id: String) {
        self.0 = Some(wall_id);
    }

    pub fn deselect(&mut self) {
        self.0 = None;
    }

    pub fn is_selected(&self, wall_id: &str) -> bool {
        self.0.as_deref() == Some(wall_id)
    }

    pub fn get_selected(&self) -> Option<&String> {
        self.0.as_ref()
    }
}

/// Bevy `Resource` newtype over the engine-agnostic `WallSet` core type.
/// `Deref`/`DerefMut` make this transparent to existing call sites
/// (`wall_set.walls()`, `wall_set.upsert(...)`, etc. all keep working
/// unchanged) while Bevy's `ResMut<WallSet>` change detection still
/// applies normally — it only cares that *this* type is a `Resource`,
/// not what's inside it.
#[derive(Resource, Default)]
pub struct WallSet(pub thunderforge_canvas_core::wall::WallSet);

impl Deref for WallSet {
    type Target = thunderforge_canvas_core::wall::WallSet;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WallSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
