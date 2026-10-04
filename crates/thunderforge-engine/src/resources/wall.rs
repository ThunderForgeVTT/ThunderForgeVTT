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

/// What the wall tool draws when it is dragged.
///
/// Spec 031 FR-026. A Game Master building a map spends most of the time
/// drawing the same two things — the outline of a room, and a door in it — and
/// drawing either from single segments is four gestures and a keypress where
/// it could be one. The primitive says which of them a drag means.
///
/// Deliberately a property of the tool rather than of a wall: nothing about a
/// finished wall records that it was drawn as part of a room. Four walls drawn
/// as a room and four drawn one at a time are the same four walls, and a room
/// that remained a unit would be a grouping feature that nothing has asked
/// for — and one that every later edit would have to maintain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallPrimitive {
    /// One wall per drag, and a click continues a multi-point chain. The
    /// behaviour the tool has always had, and the default for that reason.
    #[default]
    Segment,
    /// Four walls closing a rectangle between the drag's two corners.
    Room,
    /// One wall, already a closed door.
    Door,
}

impl WallPrimitive {
    /// Parse the identifier the web app uses.
    ///
    /// An unrecognised value yields `None` and the caller leaves the primitive
    /// alone — the same rule `AuthoringMode::from_tool_id` follows, and for the
    /// same reason: a name this build does not know must not silently rearm
    /// the tool to something the Game Master did not pick.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "segment" => Some(Self::Segment),
            "room" => Some(Self::Room),
            "door" => Some(Self::Door),
            _ => None,
        }
    }

    /// The identifier the web app uses, for reporting the choice back out.
    pub fn as_id(self) -> &'static str {
        match self {
            Self::Segment => "segment",
            Self::Room => "room",
            Self::Door => "door",
        }
    }
}

/// The primitive the wall tool currently draws.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWallPrimitive(pub WallPrimitive);

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
