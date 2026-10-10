//! Spec 088 US6: the walls along a map's edges.
//!
//! A map's art ends at its edges, but nothing stopped a token there: a token
//! dragged off the side kept going into the dark. So an import walls the
//! rectangle the art covers, except where the file's own walls already do.
//! Pure: the caller says where the map sits and what walls it already has.

use super::geometry::{ScenePlacement, WallInsert};

/// How far, in scene pixels, a wall may sit from an edge and still count as
/// lying on it.
pub const TOLERANCE: f64 = 0.5;

/// One edge of the map: the line it runs along, and its extent on the other
/// axis. `horizontal` edges sit at `y = at` from `x = from` to `x = to`;
/// vertical ones at `x = at` from `y = from` to `y = to`.
#[derive(Clone, Copy)]
struct Edge {
    horizontal: bool,
    at: f64,
    from: f64,
    to: f64,
}

impl Edge {
    /// A point as (distance off the edge's line, position along it).
    fn local(&self, x: f64, y: f64) -> (f64, f64) {
        if self.horizontal {
            ((y - self.at).abs(), x)
        } else {
            ((x - self.at).abs(), y)
        }
    }

    /// The stretch of this edge a wall covers, if both its ends lie on it.
    fn covered_by(&self, wall: &WallInsert) -> Option<(f64, f64)> {
        let (off_a, along_a) = self.local(wall.x1, wall.y1);
        let (off_b, along_b) = self.local(wall.x2, wall.y2);
        if off_a > TOLERANCE || off_b > TOLERANCE {
            return None;
        }
        let start = along_a.min(along_b).max(self.from);
        let end = along_a.max(along_b).min(self.to);
        (end > start).then_some((start, end))
    }

    /// Whether a wall lies along this edge and inside its length.
    fn holds(&self, wall: &WallInsert) -> bool {
        let (off_a, along_a) = self.local(wall.x1, wall.y1);
        let (off_b, along_b) = self.local(wall.x2, wall.y2);
        let inside = |along: f64| along >= self.from - TOLERANCE && along <= self.to + TOLERANCE;
        off_a <= TOLERANCE && off_b <= TOLERANCE && inside(along_a) && inside(along_b)
    }

    fn wall(&self, start: f64, end: f64) -> WallInsert {
        let (x1, y1, x2, y2) = if self.horizontal {
            (start, self.at, end, self.at)
        } else {
            (self.at, start, self.at, end)
        };
        WallInsert {
            x1,
            y1,
            x2,
            y2,
            blocks_vision: true,
            blocks_movement: true,
            door_state: "none",
            perimeter: true,
        }
    }
}

/// Top, right, bottom and left of a `width` x `height` map centred on the
/// origin, y up.
fn edges(width: f64, height: f64) -> [Edge; 4] {
    let (w, h) = (width / 2.0, height / 2.0);
    [
        Edge {
            horizontal: true,
            at: h,
            from: -w,
            to: w,
        },
        Edge {
            horizontal: false,
            at: w,
            from: -h,
            to: h,
        },
        Edge {
            horizontal: true,
            at: -h,
            from: -w,
            to: w,
        },
        Edge {
            horizontal: false,
            at: -w,
            from: -h,
            to: h,
        },
    ]
}

/// FR-090: walls along the four edges of the map's rectangle, less every
/// stretch an existing wall already covers. A fully covered edge gets none,
/// and a partly covered one a wall per gap wider than `TOLERANCE`.
pub fn perimeter_walls(placement: &ScenePlacement, existing: &[WallInsert]) -> Vec<WallInsert> {
    let mut walls = Vec::new();
    for edge in edges(placement.width, placement.height) {
        let mut covered: Vec<(f64, f64)> = existing
            .iter()
            .filter_map(|wall| edge.covered_by(wall))
            .collect();
        covered.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut cursor = edge.from;
        for (start, end) in covered {
            if start - cursor > TOLERANCE {
                walls.push(edge.wall(cursor, start));
            }
            cursor = cursor.max(end);
        }
        if edge.to - cursor > TOLERANCE {
            walls.push(edge.wall(cursor, edge.to));
        }
    }
    walls
}

/// Whether a wall still lies on the edges of a `width` x `height` map: both
/// ends within `TOLERANCE` of the same edge, and inside its length. A marked
/// wall the GM has moved fails this, and is left alone.
pub fn lies_on_bounds(wall: &WallInsert, width: f64, height: f64) -> bool {
    edges(width, height).iter().any(|edge| edge.holds(wall))
}

/// The `metadata` an edge wall is written with.
pub fn perimeter_metadata() -> serde_json::Value {
    serde_json::json!({ "perimeter": true })
}

/// Whether a wall's `metadata` marks it as an edge wall.
pub fn is_marked(metadata: Option<&serde_json::Value>) -> bool {
    metadata
        .and_then(|metadata| metadata.get("perimeter"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "perimeter_tests.rs"]
mod tests;
