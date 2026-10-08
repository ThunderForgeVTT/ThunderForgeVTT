//! Reach, range and line of sight, measured from footprint to footprint (spec
//! 046 US4, FR-032–FR-035, decision 3, research R11).
//!
//! **Flags, never refusals** (clarification 1, contract C3). Nothing here can
//! stop an attack; it describes one.
//!
//! - **Distance**: [`GridSpec::footprint_distance`] between the squares each
//!   creature fills, in cells, times the system's `vision.unitsPerCell` — so
//!   5e reads feet. Adjacent is one cell: five feet.
//! - **Line of sight**: [`footprint_line_of_sight`] against the scene's walls,
//!   skipped for an ability that does not need it.
//!
//! The server's `combat::reach::SceneMeasure` loads a scene and calls
//! [`measure`]; the demo calls it on the scene it holds.

use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::{Footprint, GridKind, GridSpec};
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::wall::{WallSet, footprint_line_of_sight};

use crate::records::{
    FLAG_BEYOND_RANGE, FLAG_LONG_RANGE, FLAG_NO_LINE_OF_SIGHT, FLAG_NO_REACH_DECLARED,
    FLAG_OUT_OF_REACH,
};

/// What an ability or item says about how far it reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Reach {
    /// A melee reach, in system units.
    pub reach: Option<f64>,
    /// A ranged attack's normal range, in system units.
    pub range_normal: Option<f64>,
    /// And its long range; beyond normal and within this is a long shot.
    pub range_long: Option<f64>,
    pub needs_line_of_sight: bool,
}

/// What an attack is recorded with.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Measured {
    /// In system units; `None` with no target.
    pub distance: Option<f64>,
    pub flags: Vec<String>,
}

/// The flags a distance and a line of sight earn an attack (pure).
///
/// - Within its **reach**, nothing.
/// - Beyond its reach with a **range** (a thrown dagger), or with only a
///   range: beyond normal is `long_range`, and beyond long — or beyond normal
///   when no long range is declared — is `beyond_range`.
/// - Beyond its reach with no range: `out_of_reach`.
/// - Neither declared: `no_reach_declared`, because a table cannot be told
///   the swing was too far when nobody said how far it goes.
/// - No line of sight when it needs one: `no_line_of_sight`, whatever else.
pub fn flags_for(distance: f64, reach: &Reach, line_of_sight: bool) -> Vec<String> {
    let mut flags = Vec::new();
    let within = |limit: f64| distance <= limit + SLACK;

    match (reach.reach, reach.range_normal, reach.range_long) {
        (None, None, None) => flags.push(FLAG_NO_REACH_DECLARED),
        (Some(r), _, _) if within(r) => {}
        (_, None, None) => flags.push(FLAG_OUT_OF_REACH),
        (_, normal, long) => {
            let normal = normal.or(long).unwrap_or(0.0);
            let long = long.unwrap_or(normal).max(normal);
            if !within(long) {
                flags.push(FLAG_BEYOND_RANGE);
            } else if !within(normal) {
                flags.push(FLAG_LONG_RANGE);
            }
        }
    }
    if reach.needs_line_of_sight && !line_of_sight {
        flags.push(FLAG_NO_LINE_OF_SIGHT);
    }
    flags.into_iter().map(str::to_string).collect()
}

/// A scene's grid, as the engine builds it from the same row: anchored to the
/// map's corner when the scene has a size, else at the world origin
/// (`WorldPage`'s `set_scene_grid`, the engine's `SceneGrid`).
pub fn scene_grid(grid_type: &str, grid_size: i32, width: i32, height: i32) -> GridSpec {
    let kind = GridKind::from_server_str(grid_type);
    let size = grid_size.max(1) as f32;
    if width > 0 && height > 0 {
        GridSpec::anchored_to_map(kind, size, Vec2::new(width as f32, height as f32))
    } else {
        GridSpec {
            kind,
            size,
            origin: Vec2::ZERO,
        }
    }
}

/// A hair of tolerance: 5.000001 ft from float arithmetic is five feet.
const SLACK: f64 = 1e-6;

/// Spec 084 research R6: the attack is a melee one when the part has a
/// reach and the target was measured within it. A target with no distance
/// (none chosen, or not on the board) is not in melee.
pub fn is_melee(distance: Option<f64>, reach: &Reach) -> bool {
    matches!((distance, reach.reach), (Some(d), Some(r)) if d <= r + SLACK)
}

/// Distance and flags for one attack between two placed creatures.
pub fn measure(
    grid: &GridSpec,
    units: &GridUnits,
    walls: &WallSet,
    from: (Vec2, Footprint),
    to: (Vec2, Footprint),
    reach: &Reach,
) -> Measured {
    let ((from, from_size), (to, to_size)) = (from, to);
    let cells = grid.footprint_distance(from, from_size, to, to_size);
    let distance = (units.distance(cells) as f64 * 100.0).round() / 100.0;
    let line_of_sight = !reach.needs_line_of_sight
        || footprint_line_of_sight(grid, from, from_size, to, to_size, walls);
    Measured {
        distance: Some(distance),
        flags: flags_for(distance, reach, line_of_sight),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reach(reach: Option<f64>, range_normal: Option<f64>) -> Reach {
        Reach {
            reach,
            range_normal,
            range_long: None,
            needs_line_of_sight: true,
        }
    }

    #[test]
    fn melee_is_a_reach_and_a_target_within_it() {
        assert!(is_melee(Some(5.0), &reach(Some(5.0), None)));
        assert!(is_melee(Some(5.000_000_1), &reach(Some(5.0), None)));
        assert!(
            !is_melee(Some(10.0), &reach(Some(5.0), None)),
            "out of reach"
        );
        assert!(
            !is_melee(Some(5.0), &reach(None, Some(80.0))),
            "a ranged part"
        );
        assert!(!is_melee(None, &reach(Some(5.0), None)), "no distance");
    }
}
