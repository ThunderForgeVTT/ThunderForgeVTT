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
                grid_walk(&rule.grid, start, end)
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
    let start = rule.vertex_among(start, anchors.iter().copied(), radius);
    let end = rule.vertex_among(cursor, anchors, radius);
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
}
