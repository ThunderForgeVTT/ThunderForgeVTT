//! What a wall gesture lays down, and the preview of it (spec 077).
//!
//! `handle_wall_input` (`systems/wall.rs`) owns the pointer: it decides when
//! a gesture begins and ends and which primitive is armed. This module owns
//! what that gesture *means* once both of its points are known —
//! [`planned_walls`] — so the preview drawn while the button is held and the
//! walls emitted when it is released come from one function and cannot
//! disagree (FR-006, FR-019's sibling for walls).
//!
//! With snapping on, every plan is cell edges: a walk along grid lines for a
//! segment (FR-005), the perimeter for a box (FR-012), the rasterised ring
//! for a circle (FR-013). With it off, a plan is free geometry: one wall, a
//! rectangle of four, a polygon of chords (FR-008). The geometry is
//! `thunderforge_canvas_core::wall_layout`; this module only chooses.

use bevy::prelude::*;
use serde_json::json;
use thunderforge_canvas_core::snapping::{SNAP_RADIUS_PIXELS, SnapRule};
use thunderforge_canvas_core::wall::{WallEdit, WallSet, room_segments};
use thunderforge_canvas_core::wall_layout::{
    Segment, box_edges, chords_for, circle_chords, circle_edges, grid_walk,
};

use crate::emit_event;
use crate::resources::WallPrimitive;

/// A free circle's chords are about this long on screen, so it looks round
/// at the zoom it was drawn at and no rounder (FR-013).
const FREE_CHORD_PIXELS: f32 = 12.0;

/// Colour the preview is drawn in: the wall colour, see-through, so what is
/// promised reads as not yet there.
const PREVIEW_COLOR: Color = Color::srgba(0.95, 0.85, 0.25, 0.7);

/// The walls a gesture from `start` to `end` lays down under `primitive`.
///
/// `start` and `end` are already snapped by the caller (a wall endpoint, a
/// lattice corner, or the raw cursor when snapping is off). `pixel` is the
/// size of one screen pixel in world units at the current zoom, for the
/// free circle's chord count.
pub(crate) fn planned_walls(
    primitive: WallPrimitive,
    rule: &SnapRule,
    start: Vec2,
    end: Vec2,
    pixel: f32,
) -> Vec<Segment> {
    let snapped = rule.is_active();
    match primitive {
        WallPrimitive::Segment => {
            if snapped {
                walk_from(rule, start, end)
            } else {
                vec![(start, end)]
            }
        }
        WallPrimitive::Door => vec![(start, end)],
        WallPrimitive::Room => {
            if snapped {
                box_edges(&rule.grid, start, end)
            } else {
                room_segments(start, end)
                    .map(|sides| sides.to_vec())
                    .unwrap_or_default()
            }
        }
        WallPrimitive::Circle => {
            let radius = start.distance(end);
            if snapped {
                circle_edges(&rule.grid, start, radius)
            } else {
                circle_chords(start, radius, chords_for(radius, FREE_CHORD_PIXELS * pixel))
            }
        }
    }
}

/// Nearer than this to the lattice, an end *is* the vertex: no stub.
const ON_LATTICE: f32 = 1e-2;

/// [`grid_walk`] between two points that may not be lattice vertices. The
/// walk runs vertex to vertex; an end that was snapped elsewhere — onto
/// another wall's end (spec 077, walls before corners) or into its middle
/// (spec 078's join) — gets a short free wall from the nearest vertex to it,
/// so the drawn wall touches what it was aimed at instead of rounding away.
fn walk_from(rule: &SnapRule, start: Vec2, end: Vec2) -> Vec<Segment> {
    let a = rule.vertex(start);
    let b = rule.vertex(end);
    let mut walls = Vec::new();
    if start.distance(a) > ON_LATTICE {
        walls.push((start, a));
    }
    walls.extend(grid_walk(&rule.grid, a, b));
    if end.distance(b) > ON_LATTICE {
        walls.push((b, end));
    }
    walls
}

/// Emit `create_wall` for each planned wall and record the gesture as one
/// undo entry (FR-015). A door gesture emits closed doors; everything else
/// emits plain walls with the drawn defaults.
pub(crate) fn emit_planned(
    wall_set: &mut WallSet,
    world_id: &str,
    walls: &[Segment],
    as_door: bool,
) {
    if walls.is_empty() {
        return;
    }
    for (from, to) in walls {
        emit_event(json!({
            "type": "create_wall",
            "wall": {
                "x1": from.x,
                "y1": from.y,
                "x2": to.x,
                "y2": to.y,
                "blocksVision": true,
                "blocksMovement": true,
                // A door drawn onto a map is a door in a wall, closed: a Game
                // Master who wanted an opening would have drawn none.
                "doorState": if as_door { "closed" } else { "none" },
            },
            "worldId": world_id,
        }));
    }
    wall_set.push_undo(WallEdit::Created {
        endpoints: walls.to_vec(),
    });
}

/// Spec 078 FR-001: a wall ending in the middle of another wall joins it.
///
/// Returns where the end lands — on that wall — and splits the wall there:
/// the original keeps its id and is shortened (`update_wall`), the rest is a
/// new wall with the same flags (`create_wall`). Two walls meeting at a
/// corner already join, so a point on a wall's end is returned unchanged;
/// so is one near no wall, or near only a door (`WallSet::join_target`).
///
/// Pushed as its own undo step, before the gesture's `Created`: one Ctrl+Z
/// takes the drawn wall back, a second un-splits.
pub(crate) fn join_endpoint(
    wall_set: &mut WallSet,
    world_id: &str,
    point: Vec2,
    radius: f32,
) -> Vec2 {
    let Some(join) = wall_set.join_target(point, radius) else {
        return point;
    };
    let Some(split) = wall_set.split_at(&join.wall_id, join.at) else {
        return point;
    };
    let Some(prior) = wall_set.get(&join.wall_id).map(|w| (w.start(), w.end())) else {
        return point;
    };
    let shortened = split.shortened;
    let (from, to) = split.remainder;
    emit_event(json!({
        "type": "update_wall",
        "wallId": shortened.id,
        "changes": { "x2": shortened.x2, "y2": shortened.y2 },
        "worldId": world_id,
    }));
    emit_event(json!({
        "type": "create_wall",
        "wall": {
            "x1": from.x,
            "y1": from.y,
            "x2": to.x,
            "y2": to.y,
            "blocksVision": shortened.blocks_vision,
            "blocksMovement": shortened.blocks_movement,
            "doorState": "none",
        },
        "worldId": world_id,
    }));
    wall_set.push_undo(WallEdit::Split {
        wall_id: shortened.id.clone(),
        prior,
        remainder: (from, to),
    });
    wall_set.upsert(shortened);
    join.at
}

/// Undo of [`WallEdit::Split`]: the original back to its full length, the
/// remainder deleted by its endpoints (the server named it).
pub(crate) fn undo_split(
    wall_set: &mut WallSet,
    world_id: &str,
    wall_id: &str,
    prior: (Vec2, Vec2),
    remainder: (Vec2, Vec2),
) {
    undo_created(wall_set, world_id, &[remainder]);
    if let Some(mut wall) = wall_set.get(wall_id).cloned() {
        wall.x1 = prior.0.x;
        wall.y1 = prior.0.y;
        wall.x2 = prior.1.x;
        wall.y2 = prior.1.y;
        wall_set.upsert(wall);
    }
    emit_event(json!({
        "type": "update_wall",
        "wallId": wall_id,
        "changes": { "x1": prior.0.x, "y1": prior.0.y, "x2": prior.1.x, "y2": prior.1.y },
        "worldId": world_id,
    }));
}

/// Undo of [`WallEdit::Created`]: the walls now in the set whose endpoints
/// are the gesture's, deleted. The server named them after the gesture, so
/// endpoints are the only handle undo has; a wall moved since is left alone,
/// which is the safer mistake.
pub(crate) fn undo_created(wall_set: &mut WallSet, world_id: &str, endpoints: &[(Vec2, Vec2)]) {
    const SAME: f32 = 1e-2;
    let doomed: Vec<String> = wall_set
        .walls()
        .iter()
        .filter(|wall| {
            endpoints.iter().any(|(a, b)| {
                (wall.start().distance(*a) <= SAME && wall.end().distance(*b) <= SAME)
                    || (wall.start().distance(*b) <= SAME && wall.end().distance(*a) <= SAME)
            })
        })
        .map(|wall| wall.id.clone())
        .collect();
    for id in doomed {
        wall_set.remove(&id);
        emit_event(json!({
            "type": "delete_wall",
            "wallId": id,
            "worldId": world_id,
        }));
    }
}

/// One screen pixel in world units at the camera's zoom, or `1.0` with no
/// camera to ask. The snap radius is a screen distance (FR-010); this is
/// what turns it into a world one.
pub(crate) fn world_per_pixel(camera_query: &Query<(&Camera, &GlobalTransform)>) -> f32 {
    let Some((camera, transform)) = camera_query.iter().next() else {
        return 1.0;
    };
    let origin = camera.viewport_to_world_2d(transform, Vec2::ZERO);
    let one = camera.viewport_to_world_2d(transform, Vec2::X);
    match (origin, one) {
        (Ok(a), Ok(b)) => a.distance(b).max(f32::EPSILON),
        _ => 1.0,
    }
}

/// The snap radius in world units at this zoom.
pub(crate) fn snap_radius(camera_query: &Query<(&Camera, &GlobalTransform)>) -> f32 {
    SNAP_RADIUS_PIXELS * world_per_pixel(camera_query)
}

/// The endpoints a wall endpoint may snap to: every wall's, except the one
/// being dragged (its own far end is not a thing to land on).
pub(crate) fn wall_anchors(wall_set: &WallSet, except: Option<&str>) -> Vec<Vec2> {
    wall_set
        .walls()
        .iter()
        .filter(|wall| except != Some(wall.id.as_str()))
        .flat_map(|wall| [wall.start(), wall.end()])
        .collect()
}

/// Draw the walls a gesture in progress will lay down (FR-006): the same
/// plan the release will emit, in the preview colour.
pub(crate) fn draw_wall_preview(gizmos: &mut Gizmos, walls: &[Segment]) {
    for (from, to) in walls {
        gizmos.line_2d(*from, *to, PREVIEW_COLOR);
    }
}

/// While the button is held on a wall gesture, show what releasing it here
/// would lay down. Snaps both ends exactly as `handle_wall_input` will.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_wall_drag(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    drag: Res<crate::systems::wall::WallDragState>,
    wall_set: Res<crate::resources::WallSet>,
    is_gm: Res<crate::resources::IsGameMaster>,
    scene_grid: Res<crate::resources::SceneGrid>,
    snap_enabled: Res<crate::resources::GridSnapEnabled>,
    primitive: Res<crate::resources::ActiveWallPrimitive>,
    mut gizmos: Gizmos,
) {
    if !is_gm.0 {
        return;
    }
    let Some(start) = drag.creating_from() else {
        return;
    };
    let Some((camera, transform)) = camera_query.iter().next() else {
        return;
    };
    let Some(cursor) = pointer
        .position()
        .and_then(|px| camera.viewport_to_world_2d(transform, px).ok())
    else {
        return;
    };
    let rule = SnapRule::new(scene_grid.0, snap_enabled.0);
    let radius = snap_radius(&camera_query);
    let anchors = wall_anchors(&wall_set, None);
    // Spec 078: a segment end over another wall's middle shows landing on
    // it, as the release will (`handle_wall_input` makes the same choice).
    let land = |raw: Vec2, snapped: Vec2| {
        if primitive.0 == WallPrimitive::Segment && snap_enabled.0 {
            wall_set
                .join_target(raw, radius)
                .map_or(snapped, |join| join.at)
        } else {
            snapped
        }
    };
    let start = land(
        start,
        rule.vertex_among(start, anchors.iter().copied(), radius),
    );
    let end = land(cursor, rule.vertex_among(cursor, anchors, radius));
    let walls = planned_walls(
        primitive.0,
        &rule,
        start,
        end,
        world_per_pixel(&camera_query),
    );
    draw_wall_preview(&mut gizmos, &walls);
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_canvas_core::grid::{GridKind, GridSpec};

    fn square(size: f32, on: bool) -> SnapRule {
        SnapRule::new(
            GridSpec {
                kind: GridKind::Square,
                size,
                origin: Vec2::ZERO,
            },
            on,
        )
    }

    #[test]
    fn a_snapped_segment_walks_and_a_free_one_is_one_wall() {
        let a = Vec2::ZERO;
        let b = Vec2::new(150.0, 100.0);
        assert_eq!(
            planned_walls(WallPrimitive::Segment, &square(50.0, true), a, b, 1.0).len(),
            5
        );
        assert_eq!(
            planned_walls(WallPrimitive::Segment, &square(50.0, false), a, b, 1.0),
            vec![(a, b)]
        );
    }

    #[test]
    fn a_snapped_segment_ending_off_the_lattice_keeps_touching_its_target() {
        // The end was put on another wall's middle at (150, 20): the walk
        // runs to the nearest vertex (150, 0) and a stub reaches the target.
        let a = Vec2::ZERO;
        let end = Vec2::new(150.0, 20.0);
        let walls = planned_walls(WallPrimitive::Segment, &square(50.0, true), a, end, 1.0);
        assert_eq!(walls.len(), 4);
        assert_eq!(walls[0].0, a);
        assert_eq!(walls[3], (Vec2::new(150.0, 0.0), end));
    }

    #[test]
    fn a_box_is_edges_when_snapped_and_four_walls_when_free() {
        let a = Vec2::ZERO;
        let b = Vec2::new(150.0, 100.0);
        assert_eq!(
            planned_walls(WallPrimitive::Room, &square(50.0, true), a, b, 1.0).len(),
            10
        );
        assert_eq!(
            planned_walls(WallPrimitive::Room, &square(50.0, false), a, b, 1.0).len(),
            4
        );
    }

    #[test]
    fn a_circle_is_a_ring_either_way_and_a_door_is_one_wall() {
        let c = Vec2::new(25.0, 25.0);
        let r = Vec2::new(125.0, 25.0);
        let snapped = planned_walls(WallPrimitive::Circle, &square(50.0, true), c, r, 1.0);
        assert!(snapped.len() >= 8, "{}", snapped.len());
        let free = planned_walls(WallPrimitive::Circle, &square(50.0, false), c, r, 1.0);
        assert_eq!(free.len(), chords_for(100.0, FREE_CHORD_PIXELS));
        assert_eq!(free.last().unwrap().1, free[0].0);
        assert_eq!(
            planned_walls(WallPrimitive::Door, &square(50.0, true), c, r, 1.0),
            vec![(c, r)]
        );
    }

    #[test]
    fn undo_deletes_the_walls_the_gesture_made_by_their_endpoints() {
        use thunderforge_canvas_core::wall::{DoorState, Wall};
        let mut set = WallSet::default();
        let wall = |id: &str, x1: f32, x2: f32| Wall {
            id: id.into(),
            x1,
            y1: 0.0,
            x2,
            y2: 0.0,
            blocks_vision: true,
            blocks_movement: true,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        };
        set.upsert(wall("mine", 0.0, 50.0));
        set.upsert(wall("reversed", 100.0, 50.0));
        set.upsert(wall("other", 200.0, 250.0));
        undo_created(
            &mut set,
            "w",
            &[
                (Vec2::ZERO, Vec2::new(50.0, 0.0)),
                (Vec2::new(50.0, 0.0), Vec2::new(100.0, 0.0)),
            ],
        );
        let left: Vec<&str> = set.walls().iter().map(|w| w.id.as_str()).collect();
        assert_eq!(left, vec!["other"]);
    }

    #[test]
    fn an_end_on_a_wall_joins_it_and_undo_makes_it_whole() {
        use thunderforge_canvas_core::wall::{DoorState, Wall};
        let mut set = WallSet::default();
        set.upsert(Wall {
            id: "long".into(),
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 0.0,
            blocks_vision: true,
            blocks_movement: true,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        });

        // Near the middle: lands on the wall and shortens it.
        let at = join_endpoint(&mut set, "w", Vec2::new(40.0, 4.0), 12.0);
        assert_eq!(at, Vec2::new(40.0, 0.0));
        assert_eq!(set.get("long").unwrap().end(), Vec2::new(40.0, 0.0));
        assert_eq!(set.undo_stack_len(), 1);

        // On its corner, or far from it: nothing moves, nothing splits.
        assert_eq!(join_endpoint(&mut set, "w", Vec2::ZERO, 12.0), Vec2::ZERO);
        let far = Vec2::new(40.0, 30.0);
        assert_eq!(join_endpoint(&mut set, "w", far, 12.0), far);
        assert_eq!(set.undo_stack_len(), 1);

        // Undo: the remainder (once the server names it) goes, the original
        // is whole again.
        set.upsert(Wall {
            id: "rest".into(),
            x1: 40.0,
            y1: 0.0,
            x2: 100.0,
            y2: 0.0,
            blocks_vision: true,
            blocks_movement: true,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        });
        let Some(WallEdit::Split {
            wall_id,
            prior,
            remainder,
        }) = set.pop_undo()
        else {
            panic!("a split on the undo stack");
        };
        undo_split(&mut set, "w", &wall_id, prior, remainder);
        assert!(set.get("rest").is_none());
        assert_eq!(set.get("long").unwrap().end(), Vec2::new(100.0, 0.0));
    }
}
