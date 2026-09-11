//! One light's shadow map — playtest 2026-09-10 P9. Split from `vision.rs`,
//! which re-exports it, so `vision::shadow_map_row` is unchanged for callers.

use glam::Vec2;

use crate::wall::WallSet;

/// One light's shadow map: in each of `bins` directions around `origin`, how
/// far the light reaches before a vision-blocking wall stops it, capped at
/// `reach` — playtest 2026-09-10 P9.
///
/// Shadows used to be quads painted over the whole darkness layer, so one
/// light's shadow also darkened every other light's pool it fell across: a
/// room lit from both sides was dark behind each wall. A row per light lets
/// the darkness shader stop a light only where *its own* view is blocked, at
/// a cost per pixel of one lookup per light rather than one test per wall.
///
/// Direction `k` points at `-π + 2π(k + ½)/bins`, so bin `k` covers the angles
/// `[-π + 2πk/bins, -π + 2π(k+1)/bins)` — the convention `darkness.wgsl`
/// indexes by, from `atan2`.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowRow {
    pub distances: Vec<f32>,
    /// How many walls stop this light in at least one direction: the walls
    /// actually casting its shadows.
    pub casting_walls: usize,
}

pub fn shadow_map_row(origin: Vec2, reach: f32, walls: &WallSet, bins: usize) -> ShadowRow {
    use std::f32::consts::{PI, TAU};

    // Only walls that could reach into the light's radius: a wall's nearest
    // point is at least its midpoint's distance minus half its length.
    let candidates: Vec<(Vec2, Vec2)> = walls
        .vision_blocking_walls()
        .filter(|wall| wall.midpoint().distance(origin) <= reach + wall.length() / 2.0)
        .map(|wall| (wall.start(), wall.end()))
        .collect();

    let mut distances = vec![reach; bins];
    let mut casting = vec![false; candidates.len()];
    for (k, distance) in distances.iter_mut().enumerate() {
        let angle = -PI + TAU * (k as f32 + 0.5) / bins as f32;
        let direction = Vec2::new(angle.cos(), angle.sin());
        let mut nearest = None;
        for (i, (a, b)) in candidates.iter().enumerate() {
            if let Some(t) = ray_hits_segment(origin, direction, *a, *b)
                && t < *distance
            {
                *distance = t;
                nearest = Some(i);
            }
        }
        if let Some(i) = nearest {
            casting[i] = true;
        }
    }

    ShadowRow {
        distances,
        casting_walls: casting.iter().filter(|c| **c).count(),
    }
}

/// How close to its light a wall must be crossed to count as blocking it.
///
/// A crossing at the light itself is not a wall in the way. A light sitting on
/// a wall — or snapped to a corner where walls meet, which grid snapping does
/// to sconces as a matter of course — would otherwise meet that wall at zero
/// distance in every direction and go completely dark.
const ON_THE_LIGHT: f32 = 1e-3;

/// How far along the ray `origin + t·direction` (`t > 0`, `direction` of unit
/// length) it crosses the segment `a`–`b`, if it does at all.
fn ray_hits_segment(origin: Vec2, direction: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let edge = b - a;
    let denominator = direction.perp_dot(edge);
    if denominator.abs() <= f32::EPSILON {
        // Parallel: a ray grazing along a wall is not stopped by it.
        return None;
    }
    let to_a = a - origin;
    let t = to_a.perp_dot(edge) / denominator;
    let u = to_a.perp_dot(direction) / denominator;
    (t > ON_THE_LIGHT && (0.0..=1.0).contains(&u)).then_some(t)
}

#[cfg(test)]
mod shadow_map_tests {
    use super::*;
    use crate::wall::{DoorState, Wall};

    fn walls(list: &[(f32, f32, f32, f32, DoorState)]) -> WallSet {
        let mut set = WallSet::default();
        for (i, (x1, y1, x2, y2, door_state)) in list.iter().enumerate() {
            set.upsert(Wall {
                id: format!("w{i}"),
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                blocks_vision: true,
                blocks_movement: false,
                door_state: *door_state,
                locked: false,
                secret: false,
            });
        }
        set
    }

    /// The bin whose direction is closest to `angle`.
    fn bin_at(angle: f32, bins: usize) -> usize {
        use std::f32::consts::{PI, TAU};
        (((angle + PI) / TAU * bins as f32).floor() as usize).min(bins - 1)
    }

    #[test]
    fn with_no_walls_a_light_reaches_its_full_radius_everywhere() {
        let row = shadow_map_row(Vec2::ZERO, 300.0, &WallSet::default(), 64);
        assert!(row.distances.iter().all(|d| (*d - 300.0).abs() < 1e-4));
        assert_eq!(row.casting_walls, 0);
    }

    #[test]
    fn a_wall_stops_the_light_where_it_stands_and_only_in_its_direction() {
        let set = walls(&[(100.0, -50.0, 100.0, 50.0, DoorState::None)]);
        let row = shadow_map_row(Vec2::ZERO, 300.0, &set, 360);
        let east = row.distances[bin_at(0.001, 360)];
        assert!(
            (east - 100.0).abs() < 0.1,
            "stopped at the wall, got {east}"
        );
        let west = row.distances[bin_at(std::f32::consts::PI - 0.001, 360)];
        assert!(
            (west - 300.0).abs() < 1e-3,
            "nothing behind the light, got {west}"
        );
        assert_eq!(row.casting_walls, 1);
    }

    #[test]
    fn a_wall_beyond_the_light_or_an_open_door_casts_nothing() {
        let far = walls(&[(900.0, -50.0, 900.0, 50.0, DoorState::None)]);
        assert_eq!(shadow_map_row(Vec2::ZERO, 300.0, &far, 90).casting_walls, 0);
        let open = walls(&[(100.0, -50.0, 100.0, 50.0, DoorState::Open)]);
        assert_eq!(
            shadow_map_row(Vec2::ZERO, 300.0, &open, 90).casting_walls,
            0
        );
    }

    #[test]
    fn a_light_on_a_wall_or_in_a_corner_is_not_put_out_by_it() {
        // A sconce snapped to where two walls meet, and one on a wall's
        // middle: neither wall is *in the way* of the light.
        let corner = walls(&[
            (0.0, 0.0, 100.0, 0.0, DoorState::None),
            (0.0, 0.0, 0.0, 100.0, DoorState::None),
        ]);
        let row = shadow_map_row(Vec2::ZERO, 300.0, &corner, 90);
        assert!(row.distances.iter().all(|d| (*d - 300.0).abs() < 1e-3));
        let on_a_wall = walls(&[(-100.0, 0.0, 100.0, 0.0, DoorState::None)]);
        let row = shadow_map_row(Vec2::ZERO, 300.0, &on_a_wall, 90);
        assert!(row.distances.iter().all(|d| (*d - 300.0).abs() < 1e-3));
    }

    #[test]
    fn the_nearer_of_two_walls_is_the_one_that_stops_the_light() {
        let set = walls(&[
            (200.0, -50.0, 200.0, 50.0, DoorState::None),
            (100.0, -50.0, 100.0, 50.0, DoorState::None),
        ]);
        let row = shadow_map_row(Vec2::ZERO, 300.0, &set, 360);
        assert!((row.distances[bin_at(0.001, 360)] - 100.0).abs() < 0.1);
        assert_eq!(
            row.casting_walls, 1,
            "the far wall is in the near one's shadow"
        );
    }
}
