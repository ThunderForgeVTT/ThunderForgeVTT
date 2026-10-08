//! The box a user drags over the board, and what it holds (spec 085).
//!
//! The engine measures each item's footprint in world units and asks
//! [`wholly_inside`]; an item the box only touches is not taken. The
//! resulting hits are folded into the current selection by [`apply_box`],
//! which is also how a shift-click toggles one item.

use glam::Vec2;

/// A dragged box in world units, with its corners ordered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenBox {
    pub min: Vec2,
    pub max: Vec2,
}

impl ScreenBox {
    /// The box between two corners, whichever way it was dragged.
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    fn holds(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

/// What an item covers on the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Footprint {
    /// A light.
    Point(Vec2),
    /// A wall.
    Segment(Vec2, Vec2),
    /// A token's footprint, or a shape's bounds.
    Rect { min: Vec2, max: Vec2 },
}

/// Every point of `f` lies inside `b`, edges inclusive.
pub fn wholly_inside(b: &ScreenBox, f: &Footprint) -> bool {
    match *f {
        Footprint::Point(p) => b.holds(p),
        // The box is convex, so a segment is inside when both ends are.
        Footprint::Segment(p, q) => b.holds(p) && b.holds(q),
        Footprint::Rect { min, max } => b.holds(min) && b.holds(max),
    }
}

/// Without toggle: `hits`. With toggle: `current` minus the hits it holds,
/// then the hits it did not hold, in order. No duplicates either way.
pub fn apply_box(current: &[String], hits: &[String], toggle: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if toggle {
        for id in current {
            if !hits.contains(id) && !out.contains(id) {
                out.push(id.clone());
            }
        }
        for id in hits {
            if !current.contains(id) && !out.contains(id) {
                out.push(id.clone());
            }
        }
    } else {
        for id in hits {
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "box_select_tests.rs"]
mod tests;
