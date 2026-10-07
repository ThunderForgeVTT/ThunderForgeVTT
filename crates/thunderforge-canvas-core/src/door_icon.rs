//! What a door shows on the board, and when (spec 071 User Story 3).
//!
//! A door can be opened, shut, locked and hidden from a right-click, but none
//! of that was visible: a door looked like a wall with a different colour, and
//! nothing on it said "press here". The owner, of the demo: "doors don't get
//! little icon to open or close them."
//!
//! Two questions, both answered here so they are tested natively rather than
//! only compile-checked in the wasm-only engine:
//!
//! - **Which icon** ([`door_icon`]): open when shut, close when open, a
//!   padlock when locked, none when secret. It is the same for a Game Master
//!   and a player — the owner decided players see the padlock — so it takes
//!   no role. What *pressing* it does differs by role, and that is the web
//!   app's door rule (`doorIconAction`), the same module the right-click menu
//!   reads.
//! - **When** ([`icon_shown`]): while the pointer is within ten screen pixels
//!   of the door, or over the icon itself, or while a token the viewer
//!   controls is within one grid square of it.

use glam::Vec2;

use crate::wall::{DoorState, Wall};

/// How near the door the pointer has to be for its icon to show, in screen
/// pixels: the same reach a right-click on a door has (spec 071 FR-001).
pub const ICON_POINTER_REACH_PX: f32 = 10.0;

/// The icon's side as a share of one grid square.
pub const ICON_GRID_SHARE: f32 = 0.42;

/// The least the icon's side may be, in screen pixels: it is something to
/// press, and a scene's grid can be small enough to make a share of it a
/// speck (an unmapped scene keeps the 5-unit grid it was created with).
pub const ICON_MIN_SCREEN_PX: f32 = 24.0;

/// The icon a door shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoorIcon {
    /// The door is shut and may be opened.
    Open,
    /// The door is open and may be shut.
    Close,
    /// The door is locked.
    Padlock,
}

impl DoorIcon {
    /// The name the web app and the engine probe use.
    pub fn as_id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Close => "close",
            Self::Padlock => "padlock",
        }
    }
}

/// The icon `wall` shows, or `None` when it shows none.
///
/// A plain wall has nothing to press. A secret door shows nothing to anybody:
/// to the table it is wall, and an icon on it would say where it is; the Game
/// Master works a hidden door from the right-click menu. A lock outranks the
/// door's state, so a locked open door shows the padlock: pressing it is about
/// the lock, never a refused attempt to shut it.
pub fn door_icon(wall: &Wall) -> Option<DoorIcon> {
    if wall.door_state == DoorState::None || wall.secret {
        return None;
    }
    if wall.locked {
        return Some(DoorIcon::Padlock);
    }
    Some(match wall.door_state {
        DoorState::Open => DoorIcon::Close,
        _ => DoorIcon::Open,
    })
}

/// How far `point` is from the segment `a`–`b`.
pub fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

/// A token near a door: its centre and the side of its footprint, in world
/// units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearbyToken {
    pub center: Vec2,
    pub side: f32,
}

/// Where a door's icon is drawn, and how big, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconPlace {
    pub center: Vec2,
    pub side: f32,
}

impl IconPlace {
    /// The icon for `wall` on a grid whose squares are `square` wide: at the
    /// door's midpoint, a fixed share of a square, and never less than
    /// `least` — [`ICON_MIN_SCREEN_PX`] converted to world units at this zoom.
    pub fn for_wall(wall: &Wall, square: f32, least: f32) -> Self {
        Self {
            center: wall.midpoint(),
            side: (square.max(1.0) * ICON_GRID_SHARE).max(least),
        }
    }

    /// Whether `point` is on the icon.
    pub fn contains(&self, point: Vec2) -> bool {
        let half = self.side / 2.0;
        (point - self.center).abs().max_element() <= half
    }
}

/// What decides whether a door's icon shows this frame.
#[derive(Debug, Clone, Copy)]
pub struct Proximity<'a> {
    /// The pointer on the board, if it is over the board.
    pub pointer: Option<Vec2>,
    /// [`ICON_POINTER_REACH_PX`] converted to world units at this zoom.
    pub pointer_reach: f32,
    /// The tokens the viewer controls.
    pub tokens: &'a [NearbyToken],
    /// One grid square, in world units.
    pub square: f32,
}

/// Whether `wall`'s icon shows (spec 071 FR-008).
///
/// The pointer counts within reach of the door, and anywhere on the icon
/// itself: the icon can be wider than the reach, and an icon that vanished as
/// the pointer moved onto its corner could never be pressed there.
///
/// A token counts when the gap between its footprint and the door is at most
/// one square — a token standing in the next square over, or nearer.
pub fn icon_shown(wall: &Wall, place: &IconPlace, near: &Proximity) -> bool {
    let (start, end) = (wall.start(), wall.end());
    let by_pointer = near.pointer.is_some_and(|pointer| {
        place.contains(pointer) || distance_to_segment(pointer, start, end) <= near.pointer_reach
    });
    by_pointer
        || near.tokens.iter().any(|token| {
            distance_to_segment(token.center, start, end) - token.side / 2.0 <= near.square
        })
}

#[cfg(test)]
#[path = "door_icon_tests.rs"]
mod tests;
