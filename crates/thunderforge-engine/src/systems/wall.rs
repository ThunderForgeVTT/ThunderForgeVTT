//! Wall authoring input, rendering sync, and vision-occlusion systems
//! (T012-T014, T016 of specs/001-bevy-canvas-authoring/tasks.md).
//!
//! Wiring: see `plugins/wall.rs`'s `WallPlugin`. Undo is `wall_undo.rs`.

use std::collections::HashMap;

use bevy::prelude::*;
use serde_json::json;
use thunderforge_canvas_core::snapping::SnapRule;

use crate::resources::{
    ActiveWallPrimitive, CanvasLayer, DoorState, IsGameMaster, SelectedWall, Wall, WallEdit,
    WallPrimitive, WallSet,
};
use crate::systems::wall_draw::{
    emit_planned, join_endpoint, planned_walls, snap_radius, wall_anchors, world_per_pixel,
};
use crate::{ActiveWorld, emit_event};

/// Rendered height (px) of a wall's thin sprite (T012: "a small fixed
/// height like 4.0 px").
const WALL_VISUAL_HEIGHT: f32 = 4.0;

/// Minimum drag distance (px) for a click-drag to count as a wall instead
/// of being rejected as a zero-length click (T016).
const MIN_WALL_LENGTH: f32 = 1.0;

/// What a freshly drawn wall stops.
///
/// A wall somebody drew on a map is a wall: it stops sight and it stops
/// people. Every creation path here (drag, chain, room) uses these, so the
/// answer is in one place rather than repeated three times and drifting.
/// `create_wall` on the server defaults to the same pair, so a wall created
/// without an explicit profile behaves identically whichever side made it.
///
/// A vision-only wall — a window, a railing, a balcony edge — is still
/// drawable: select the wall and clear "Blocks movement" in the Walls panel,
/// or press `B` with it selected. It is a deliberate second step because it
/// is the rarer of the two.
const DRAWN_WALL_BLOCKS_VISION: bool = true;
/// See `DRAWN_WALL_BLOCKS_VISION`.
const DRAWN_WALL_BLOCKS_MOVEMENT: bool = true;

/// How close (px) the cursor must be to an existing wall's endpoint to
/// grab it for a move-drag, rather than starting a new wall or selecting
/// the wall's body.
const ENDPOINT_GRAB_RADIUS: f32 = 10.0;

/// How close (px) the cursor must be to a wall's body (the segment
/// itself, not an endpoint) to select it with a plain click.
const WALL_SELECT_DISTANCE: f32 = 6.0;

const UNSELECTED_COLOR: Color = Color::srgb(0.75, 0.75, 0.78);
const SELECTED_COLOR: Color = Color::srgb(0.95, 0.85, 0.25);
const DOOR_COLOR: Color = Color::srgb(0.55, 0.35, 0.2);
/// A locked door, for whoever can see that it is locked.
///
/// Cooler and darker than an unlocked one rather than a different hue: at a
/// glance a Game Master needs to read "door, and it will not open", and two
/// unrelated colours would read as two unrelated things.
const LOCKED_DOOR_COLOR: Color = Color::srgb(0.38, 0.26, 0.30);
/// A secret door, drawn only for the Game Master.
///
/// Deliberately faint. It is a note to the person running the scene, and it
/// should not compete with anything the table is actually looking at.
const SECRET_DOOR_COLOR: Color = Color::srgb(0.35, 0.30, 0.45);
const HANDLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const HANDLE_SIZE: Vec2 = Vec2::new(8.0, 8.0);

/// Marker on the sprite entity rendered for a given `WallSet` wall id.
#[derive(Component)]
pub(crate) struct WallVisual;

/// Marker on a GM-only endpoint-handle sprite (T012's "wall edit handles",
/// gated GM-only per `CanvasLayer::Walls.editing_is_gm_only()`).
#[derive(Component)]
pub(crate) struct WallHandle;

/// Maps `WallSet` wall ids to their spawned sprite entity, mirroring the
/// `TokenEntities` pattern in lib.rs.
#[derive(Resource, Default)]
pub(crate) struct WallEntities(HashMap<String, Entity>);

#[derive(Default)]
enum WallDragMode {
    #[default]
    Idle,
    /// Click-dragging out a brand new wall from `start` to the live
    /// cursor position.
    Creating { start: Vec2 },
    /// Dragging an existing wall's endpoint (`is_start` selects which of
    /// the two endpoints). `prior_*` is the wall's full endpoint state at
    /// drag-start, captured for the undo stack.
    MovingEndpoint {
        wall_id: String,
        is_start: bool,
        prior_x1: f32,
        prior_y1: f32,
        prior_x2: f32,
        prior_y2: f32,
    },
}

/// Session-local wall-tool drag state (not persisted, not part of
/// `WallSet`).
#[derive(Resource, Default)]
pub(crate) struct WallDragState {
    mode: WallDragMode,
}

impl WallDragState {
    /// Abandon whatever gesture is in progress, leaving nothing behind.
    ///
    /// Called from the mode's `OnExit`. A drag begun under one tool must not
    /// complete under another's rules (spec 031 FR-040a): the user changed
    /// what a click means partway through, and the honest answer is that the
    /// unfinished gesture is discarded rather than reinterpreted.
    /// Where a wall gesture in progress began, for the preview.
    pub(crate) fn creating_from(&self) -> Option<Vec2> {
        match self.mode {
            WallDragMode::Creating { start } => Some(start),
            _ => None,
        }
    }

    pub(crate) fn abandon(&mut self) {
        *self = Self::default();
    }
}

/// FR-001/FR-002: session-local, not-yet-persisted points of an
/// in-progress multi-point wall chain ("click three points, end the
/// chain -> one wall per consecutive pair"). Empty = no chain active.
/// Nothing here is emitted as a `create_wall` event until the chain ends
/// (`Enter`, see `handle_wall_keyboard_toggles`); `Escape` clears this
/// with no persistence at all (Acceptance Scenario 4).
#[derive(Resource, Default)]
pub(crate) struct WallChainState {
    points: Vec<Vec2>,
}

impl WallChainState {
    /// Discard an unfinished multi-point chain.
    ///
    /// Nothing here has been persisted — a chain only becomes walls when it is
    /// ended with Enter — so abandoning it is exactly what Escape already does
    /// (Acceptance Scenario 4). Leaving a tool is the same situation arrived at
    /// a different way.
    pub(crate) fn abandon(&mut self) {
        self.points.clear();
    }
}

/// Convert the cursor's window-pixel position into Bevy world space,
/// mirroring `systems/selection.rs`'s private `cursor_world_position`
/// helper (not exported from that module, so duplicated here rather than
/// changing that module's visibility for an unrelated feature).
fn cursor_world_position(
    pointer: &crate::plugins::touch::Pointer,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = pointer.position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_px)
        .ok()
}

/// Shortest distance from `point` to the segment `a`-`b`. Pure/testable —
/// used for wall body hit-testing (select-by-click).
pub(crate) fn distance_point_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let projection = a + ab * t;
    point.distance(projection)
}

/// Notifies the frontend of a selection change. `SelectedWall` (Bevy-side)
/// previously only ever changed locally — nothing told React's
/// `worldState.selectedWallId`, so `WallTool.tsx`'s "Selected wall" panel
/// (door toggle, blocks-vision/movement checkboxes, delete button) could
/// never appear for a wall selected by clicking the canvas, only via
/// WallTool's own `select_wall: null` dispatch after a UI-driven delete.
/// Fixed here (T014/T015, specs/002-canvas-authoring-asset-storage) by
/// emitting the same `select_wall` command type `WallTool.tsx` already
/// dispatches — a bevy-sourced event never gets re-forwarded back into the
/// engine (`bindWorldStore` skips `event.source === "bevy"`), so this
/// can't loop.
fn emit_wall_selection(wall_id: Option<&str>) {
    emit_event(json!({
        "type": "select_wall",
        "wallId": wall_id,
    }));
}

fn wall_color(wall: &Wall, selected: bool) -> Color {
    if selected {
        SELECTED_COLOR
    } else if wall.secret {
        // Only ever reached for a Game Master: `sync_wall_visuals` does not
        // draw a secret wall at all for anybody else.
        SECRET_DOOR_COLOR
    } else if wall.door_state != DoorState::None {
        if wall.locked {
            LOCKED_DOOR_COLOR
        } else {
            DOOR_COLOR
        }
    } else {
        UNSELECTED_COLOR
    }
}

/// T012: click-drag to create a wall, click to select, drag an endpoint to
/// move it. GM-only per `CanvasLayer::Walls.editing_is_gm_only()` — this
/// crate has no broader role system, so `IsGameMaster` gates it directly.
/// T016: a press-and-release without meaningfully dragging is rejected
/// (no wall created) rather than producing a zero-length segment.
/// One parameter per Query/Res the interaction reads — the shape clippy.toml
/// raised the threshold to 10 for, arrived at from the other side.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_wall_input(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut wall_set: ResMut<WallSet>,
    mut selected_wall: ResMut<SelectedWall>,
    mut drag: ResMut<WallDragState>,
    mut chain: ResMut<WallChainState>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
    scene_grid: Res<crate::resources::grid::SceneGrid>,
    snap_enabled: Res<crate::resources::token_grid::GridSnapEnabled>,
    primitive: Res<ActiveWallPrimitive>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&pointer, &camera_query) else {
        return;
    };

    // FR-024/FR-025: every point this system commits goes through the same
    // rule, and the rule is the scene's — square, hex or gridless, and off
    // entirely when the Game Master has turned snapping off.
    let rule = SnapRule::new(scene_grid.0, snap_enabled.0);

    if mouse_button.just_pressed(MouseButton::Left) {
        // FR-026: with a room or a door armed, a press always starts drawing.
        //
        // Selecting and endpoint-dragging belong to the segment tool, the same
        // way `handle_shape_input` draws rather than selects when a shape tool
        // is active. A room tool that sometimes grabbed a nearby endpoint
        // instead of starting a room would be a tool that behaves differently
        // depending on what happens to be under the cursor.
        if primitive.0 != WallPrimitive::Segment {
            // Spec 077 FR-016: with the Door primitive armed, a press on an
            // existing wall makes that wall a door rather than starting a
            // new one. The decision is taken on the press, when the wall is
            // under the cursor; the release then has nothing to draw.
            if primitive.0 == WallPrimitive::Door
                && let Some(wall) = wall_set
                    .walls()
                    .iter()
                    .find(|wall| {
                        distance_point_to_segment(cursor, wall.start(), wall.end())
                            <= WALL_SELECT_DISTANCE
                    })
                    .cloned()
            {
                let (updated, changes) = crate::systems::wall_door::door_click_update(&wall);
                wall_set.push_undo(WallEdit::DoorToggle {
                    wall_id: wall.id.clone(),
                    prior_door_state: wall.door_state,
                });
                wall_set.upsert(updated);
                selected_wall.select(wall.id.clone());
                emit_wall_selection(Some(&wall.id));
                emit_event(json!({
                    "type": "update_wall",
                    "wallId": wall.id,
                    "changes": changes,
                    "worldId": active_world.0,
                }));
                drag.mode = WallDragMode::Idle;
                return;
            }
            selected_wall.deselect();
            emit_wall_selection(None);
            drag.mode = WallDragMode::Creating { start: cursor };
            return;
        }

        // FR-001: once a wall-point chain is in progress, every click
        // feeds it directly — bypassing endpoint-grab/body-select so an
        // accidental near-miss over an existing wall doesn't hijack the
        // chain into a move/select instead of adding the next point.
        if !chain.points.is_empty() {
            drag.mode = WallDragMode::Creating { start: cursor };
            return;
        }

        // Endpoint grab takes priority over body-select/create.
        for wall in wall_set.walls() {
            if cursor.distance(wall.start()) <= ENDPOINT_GRAB_RADIUS {
                selected_wall.select(wall.id.clone());
                emit_wall_selection(Some(&wall.id));
                drag.mode = WallDragMode::MovingEndpoint {
                    wall_id: wall.id.clone(),
                    is_start: true,
                    prior_x1: wall.x1,
                    prior_y1: wall.y1,
                    prior_x2: wall.x2,
                    prior_y2: wall.y2,
                };
                return;
            }
            if cursor.distance(wall.end()) <= ENDPOINT_GRAB_RADIUS {
                selected_wall.select(wall.id.clone());
                emit_wall_selection(Some(&wall.id));
                drag.mode = WallDragMode::MovingEndpoint {
                    wall_id: wall.id.clone(),
                    is_start: false,
                    prior_x1: wall.x1,
                    prior_y1: wall.y1,
                    prior_x2: wall.x2,
                    prior_y2: wall.y2,
                };
                return;
            }
        }

        // Body select (no drag intent).
        for wall in wall_set.walls() {
            if distance_point_to_segment(cursor, wall.start(), wall.end()) <= WALL_SELECT_DISTANCE {
                selected_wall.select(wall.id.clone());
                emit_wall_selection(Some(&wall.id));
                drag.mode = WallDragMode::Idle;
                return;
            }
        }

        // Neither an endpoint nor a body: start creating a new wall.
        selected_wall.deselect();
        emit_wall_selection(None);
        drag.mode = WallDragMode::Creating { start: cursor };
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if let WallDragMode::MovingEndpoint {
            wall_id, is_start, ..
        } = &drag.mode
            && let Some(wall) = wall_set.get(wall_id).cloned()
        {
            // Snapped while dragging, not only when created. An endpoint
            // dragged to a raw cursor position lands between lattice corners,
            // which is how a room that was drawn closed stops being closed
            // the first time someone nudges a corner (FR-025).
            // And to another wall's endpoint ahead of the lattice, within
            // the screen-space radius, so a nudged corner closes onto its
            // neighbour rather than beside it (spec 077 FR-009).
            let anchors = wall_anchors(&wall_set, Some(wall_id.as_str()));
            let moved = rule.vertex_among(cursor, anchors, snap_radius(&camera_query));
            let mut updated = wall;
            if *is_start {
                updated.x1 = moved.x;
                updated.y1 = moved.y;
            } else {
                updated.x2 = moved.x;
                updated.y2 = moved.y;
            }
            // Optimistic local move so the sprite tracks the cursor;
            // reconciled by the next `upsert_wall` confirmation from
            // the server (see lib.rs's `apply_external_commands`).
            wall_set.upsert(updated);
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        match std::mem::take(&mut drag.mode) {
            WallDragMode::Creating { start } => {
                // Snapped to grid *vertices*, not cell centres.
                //
                // A wall runs between cells rather than through one, so a
                // room drawn against the lattice needs its corners on the
                // lattice — snapping to centres would put every wall half a
                // cell off and make four segments fail to meet (spec 031
                // FR-024/FR-025, and `SnapRule::vertex` exists for exactly
                // this).
                //
                // Both ends go through the rule: `start` was recorded from a
                // raw cursor when the drag began.
                let radius = snap_radius(&camera_query);
                let anchors = wall_anchors(&wall_set, None);
                let raw_start = start;
                let start = rule.vertex_among(start, anchors.iter().copied(), radius);
                let end = rule.vertex_among(cursor, anchors, radius);

                match primitive.0 {
                    WallPrimitive::Room | WallPrimitive::Circle => {
                        // Spec 077 FR-012/FR-013: a quick tool lays its walls
                        // along the grid when snapping is on and as free
                        // geometry when it is off; a degenerate drag lays
                        // none (FR-014).
                        let walls = planned_walls(
                            primitive.0,
                            &rule,
                            start,
                            end,
                            world_per_pixel(&camera_query),
                        );
                        emit_planned(&mut wall_set, &active_world.0, &walls, false);
                        return;
                    }
                    WallPrimitive::Door => {
                        if start.distance(end) >= MIN_WALL_LENGTH {
                            let walls = planned_walls(primitive.0, &rule, start, end, 1.0);
                            emit_planned(&mut wall_set, &active_world.0, &walls, true);
                        }
                        // A click with no drag draws no door (a click *on a
                        // wall* was handled on the press). Unlike the segment
                        // tool it does not seed a chain either: a chain of
                        // doors is not a thing, and silently starting one
                        // would make the next click somewhere else produce a
                        // door across the room.
                        return;
                    }
                    WallPrimitive::Segment => {}
                }

                if start.distance(end) < MIN_WALL_LENGTH {
                    // FR-001: a plain click (no drag) adds/continues a
                    // wall-point chain instead of being a no-op. The
                    // first click seeds the chain; nothing is emitted
                    // until it explicitly ends (Enter) or is cancelled
                    // (Escape) — see `handle_wall_keyboard_toggles`.
                    // Spec 078: a click near a wall's middle lands on it;
                    // the split waits for the commit, so a cancelled chain
                    // leaves the wall whole.
                    let end = if snap_enabled.0 {
                        wall_set
                            .join_target(cursor, radius)
                            .map_or(end, |join| join.at)
                    } else {
                        end
                    };
                    chain.points.push(end);
                    return;
                }

                if !chain.points.is_empty() {
                    // A real drag while a chain is active still just
                    // extends the chain by one point (from wherever the
                    // chain currently ends) rather than creating a
                    // standalone segment.
                    chain.points.push(end);
                    return;
                }

                // Spec 078 FR-001: an end in the middle of another wall
                // joins it, splitting that wall there. Only with snapping
                // on: off means the point goes exactly where it was put.
                // The raw pointer decides, not the snapped corner: a wall
                // ranks above a corner (spec 077), and a corner 30 units
                // from the wall says nothing about where the hand was.
                let mut land = |raw: Vec2, snapped: Vec2| {
                    if snap_enabled.0 && wall_set.join_target(raw, radius).is_some() {
                        join_endpoint(&mut wall_set, &active_world.0, raw, radius)
                    } else {
                        snapped
                    }
                };
                let start = land(raw_start, start);
                let end = land(cursor, end);

                // Spec 077 FR-005/FR-008: along the grid, one wall per cell
                // edge, when snapping is on; one free wall when it is off.
                // Deliberately no local WallSet entry yet: the server
                // assigns each wall's real id, so they stay untracked until
                // the matching `upsert_wall` commands arrive (see module doc
                // / WallPlugin for the rationale). Undo finds them by their
                // endpoints (`WallEdit::Created`).
                let walls = planned_walls(primitive.0, &rule, start, end, 1.0);
                emit_planned(&mut wall_set, &active_world.0, &walls, false);
            }
            WallDragMode::MovingEndpoint {
                wall_id,
                is_start,
                prior_x1,
                prior_y1,
                prior_x2,
                prior_y2,
            } => {
                if let Some(wall) = wall_set.get(&wall_id) {
                    let changes = if is_start {
                        json!({ "x1": wall.x1, "y1": wall.y1 })
                    } else {
                        json!({ "x2": wall.x2, "y2": wall.y2 })
                    };
                    emit_event(json!({
                        "type": "update_wall",
                        "wallId": wall_id,
                        "changes": changes,
                        "worldId": active_world.0,
                    }));
                }
                wall_set.push_undo(WallEdit::Move {
                    wall_id,
                    prior_x1,
                    prior_y1,
                    prior_x2,
                    prior_y2,
                });
            }
            WallDragMode::Idle => {}
        }
    }
}

/// How far a committed chain end may sit from the wall it was landed on
/// and still split it: the landing already snapped it there, so this only
/// absorbs float noise.
const JOIN_COMMIT_TOLERANCE: f32 = 0.05;

/// T012: keybound toggles for the selected wall's `blocks_vision` /
/// `blocks_movement` / door-state, plus Delete to remove it. GM-only,
/// same gating as `handle_wall_input`.
///
/// Keybinds (chosen to avoid the existing WASD/arrow-key/+-/Home bindings
/// in `lib.rs`/`plugins/camera.rs`):
/// - `V`: toggle blocks_vision
/// - `B`: toggle blocks_movement
/// - `O`: cycle door state (none -> closed -> open -> closed -> ...)
/// - `Delete`/`Backspace`: delete the selected wall
pub(crate) fn handle_wall_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wall_set: ResMut<WallSet>,
    mut selected_wall: ResMut<SelectedWall>,
    mut chain: ResMut<WallChainState>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    // FR-001/FR-002: end (Enter) or cancel (Escape) an in-progress
    // wall-point chain. While a chain is active it takes over these keys
    // entirely — a chain with only one point placed still gets no wall
    // out of Enter (nothing to connect), matching "nothing partial is
    // persisted" for anything short of two points.
    if !chain.points.is_empty() {
        if keyboard.just_pressed(KeyCode::Escape) {
            chain.points.clear();
            return;
        }
        if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
            let points = std::mem::take(&mut chain.points);
            // Spec 078 FR-001: the chain's ends were put on the walls they
            // were clicked near; now that it commits, split those walls.
            // The tolerance is "exactly there": the landing already happened.
            if let (Some(first), Some(last)) = (points.first(), points.last())
                && points.len() >= 2
            {
                join_endpoint(
                    &mut wall_set,
                    &active_world.0,
                    *first,
                    JOIN_COMMIT_TOLERANCE,
                );
                join_endpoint(&mut wall_set, &active_world.0, *last, JOIN_COMMIT_TOLERANCE);
            }
            for pair in points.windows(2) {
                emit_event(json!({
                    "type": "create_wall",
                    "wall": {
                        "x1": pair[0].x,
                        "y1": pair[0].y,
                        "x2": pair[1].x,
                        "y2": pair[1].y,
                        "blocksVision": DRAWN_WALL_BLOCKS_VISION,
                        "blocksMovement": DRAWN_WALL_BLOCKS_MOVEMENT,
                        "doorState": "none",
                    },
                    "worldId": active_world.0,
                }));
            }
            return;
        }
        // Any other key while chaining falls through to the
        // selected-wall toggles below, same as before.
    }

    let Some(wall_id) = selected_wall.get_selected().cloned() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyV) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_blocks_vision = wall.blocks_vision;
            let prior_blocks_movement = wall.blocks_movement;
            let mut updated = wall;
            updated.blocks_vision = !updated.blocks_vision;
            wall_set.upsert(updated.clone());
            wall_set.push_undo(WallEdit::FlagsToggle {
                wall_id: wall_id.clone(),
                prior_blocks_vision,
                prior_blocks_movement,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "blocksVision": updated.blocks_vision },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_blocks_vision = wall.blocks_vision;
            let prior_blocks_movement = wall.blocks_movement;
            let mut updated = wall;
            updated.blocks_movement = !updated.blocks_movement;
            wall_set.upsert(updated.clone());
            wall_set.push_undo(WallEdit::FlagsToggle {
                wall_id: wall_id.clone(),
                prior_blocks_vision,
                prior_blocks_movement,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "blocksMovement": updated.blocks_movement },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_door_state = wall.door_state;
            let next = match wall.door_state {
                DoorState::None => DoorState::Closed,
                DoorState::Closed => DoorState::Open,
                DoorState::Open => DoorState::Closed,
            };
            let mut updated = wall;
            updated.door_state = next;
            wall_set.upsert(updated);
            wall_set.push_undo(WallEdit::DoorToggle {
                wall_id: wall_id.clone(),
                prior_door_state,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "doorState": next.as_str() },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if (keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace))
        && let Some(deleted) = wall_set.remove(&wall_id)
    {
        wall_set.push_undo(WallEdit::Delete { deleted });
        selected_wall.deselect();
        emit_wall_selection(None);
        emit_event(json!({
            "type": "delete_wall",
            "wallId": wall_id,
            "worldId": active_world.0,
        }));
    }
}

/// T012/T015: keeps one thin rotated sprite per `WallSet` wall in sync
/// (spawn on new id, update transform/color on change, despawn on
/// removal) — the same "spawn/update/despawn by stable id" shape as
/// `TokenEntities` in lib.rs, just against `WallSet` instead of the
/// external-command queue. Also renders GM-only endpoint handles for the
/// selected wall (data-model.md's Canvas Layer: wall editing handles are
/// GM-only, unlike the effect they produce).
pub(crate) fn sync_wall_visuals(
    mut commands: Commands,
    wall_set: Res<WallSet>,
    selected_wall: Res<SelectedWall>,
    is_gm: Res<IsGameMaster>,
    mut wall_entities: ResMut<WallEntities>,
    mut sprite_query: Query<(&mut Transform, &mut Sprite), (With<WallVisual>, Without<WallHandle>)>,
    handle_query: Query<Entity, With<WallHandle>>,
) {
    // A Game Master's walls are authoring aids, drawn above the darkness so
    // they can be edited in a dark scene. Anyone else's are drawn under it,
    // so darkness is not traced by its own walls (playtest 2026-09-10 P9).
    let z = if is_gm.0 {
        CanvasLayer::Walls.z()
    } else {
        CanvasLayer::player_wall_z()
    };

    // Despawn sprites for walls that no longer exist in `WallSet`.
    let stale_ids: Vec<String> = wall_entities
        .0
        .keys()
        .filter(|id| wall_set.get(id).is_none())
        .cloned()
        .collect();
    for id in stale_ids {
        if let Some(entity) = wall_entities.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }

    for wall in wall_set.walls() {
        // A secret door is not drawn for the table.
        //
        // Per the spec's decision the geometry still reaches every client and
        // this is presentation only — somebody who inspects their own client
        // and announces a secret door has created a table problem, not found a
        // security hole. Resolving it here rather than by withholding geometry
        // keeps vision and movement correct for everyone: a secret door that
        // did not arrive would also stop blocking, and the wall would vanish.
        if wall.secret && !is_gm.0 {
            if let Some(entity) = wall_entities.0.remove(&wall.id) {
                commands.entity(entity).despawn();
            }
            continue;
        }

        let selected = selected_wall.is_selected(&wall.id);
        let color = wall_color(wall, selected);
        let length = wall.length().max(0.5);
        let size = Vec2::new(length, WALL_VISUAL_HEIGHT);
        let translation_z = if selected { z + 1.0 } else { z };
        let transform = Transform {
            translation: wall.midpoint().extend(translation_z),
            rotation: Quat::from_rotation_z(wall.angle()),
            ..default()
        };

        if let Some(&entity) = wall_entities.0.get(&wall.id) {
            if let Ok((mut t, mut sprite)) = sprite_query.get_mut(entity) {
                *t = transform;
                sprite.color = color;
                sprite.custom_size = Some(size);
            }
        } else {
            let entity = commands
                .spawn((Sprite::from_color(color, size), transform, WallVisual))
                .id();
            wall_entities.0.insert(wall.id.clone(), entity);
        }
    }

    // Publish what is actually on screen, for observation only.
    //
    // The claim "a player is not shown a secret door" is about *drawing*, and
    // every other way to check it is a proxy: the geometry is deliberately
    // sent to every client, so a payload assertion would prove the opposite of
    // what is wanted, and a screenshot proves only that something was painted.
    // This is the one place that knows.
    //
    // Read-only, like `get_token_status`. An observation surface that could
    // also mutate becomes a way to write tests that pass against situations
    // the application cannot reach.
    if let Ok(mut slot) = crate::drawn_walls_slot().lock() {
        *slot = wall_entities.0.keys().cloned().collect();
        slot.sort_unstable();
    }

    // GM-only endpoint handles for the selected wall (rebuilt each pass;
    // wall counts here are small enough that this isn't a hot path).
    for entity in handle_query.iter() {
        commands.entity(entity).despawn();
    }

    if is_gm.0
        && let Some(selected_id) = selected_wall.get_selected()
        && let Some(wall) = wall_set.get(selected_id)
    {
        for point in [wall.start(), wall.end()] {
            commands.spawn((
                Sprite::from_color(HANDLE_COLOR, HANDLE_SIZE),
                Transform::from_translation(point.extend(z + 2.0)),
                WallHandle,
            ));
        }
    }
}
/// Superseded by `systems::lighting::apply_light_illumination`.
///
/// This applied player line-of-sight by writing `Visibility` on every other
/// token. It has been removed rather than kept alongside, because it and the
/// lighting system wrote the same component from different criteria and
/// whichever ran later in the schedule won for that frame. Occlusion, facing
/// and illumination are now resolved together, once, through
/// `thunderforge_canvas_core::vision::visibility_of`.
///
/// The occlusion geometry itself did not go anywhere — it is
/// `thunderforge_canvas_core::wall::is_visible`, which that function calls.
pub(crate) fn init_wall_systems_resources(app: &mut App) {
    app.init_resource::<WallDragState>()
        .init_resource::<WallChainState>()
        .init_resource::<WallEntities>();
}

/// Perform the door effects this subsystem contributed to the interaction
/// seam (spec 030, US2 and US4).
///
/// # Why this lives with walls rather than with interactions
///
/// Doors are the effect most tempting to build into the interaction core,
/// because they are the most obviously spatial thing on a map. Building them
/// there would couple that plugin to walls and make it the place every future
/// subsystem also gets added — which is exactly what Constitution Principle II
/// forbids and what `scripts/verify.mjs` greps for.
///
/// So this reads the activation message like any other contributor, filters
/// for the three identifiers `canvas_core::wall` declared, and ignores the
/// rest. Nothing in the interaction plugin knows this exists.
///
/// # Why setting `WallSet` is enough
///
/// Vision and movement are *derived* from door state rather than stored
/// alongside it (`Wall::blocking`), so changing the state here re-resolves
/// both on the next frame with nothing else to keep in step. That is the
/// payoff of deriving rather than duplicating, and it is why an open window
/// and an open stone door behave correctly without either being a special
/// case.
///
/// This is the optimistic half. The server has already performed the same
/// change authoritatively; applying it here makes it visible now rather than a
/// round trip later, and the two agreeing is the client's responsibility
/// (ADR-054).
pub(crate) fn handle_door_effects(
    mut activations: MessageReader<crate::plugins::interaction::InteractionActivated>,
    mut wall_set: ResMut<WallSet>,
) {
    use thunderforge_canvas_core::wall::{
        REVEAL, SET_LOCK, SET_STATE, requested_lock, requested_state, target_of,
    };

    for activation in activations.read() {
        let effect = activation.effect_id.as_str();
        if !matches!(effect, SET_STATE | SET_LOCK | REVEAL) {
            continue;
        }
        let Some(target) = target_of(&activation.config) else {
            continue;
        };
        let Some(existing) = wall_set.get(target) else {
            // A wall this client has not been sent. Not an error: the next
            // sync will bring it, already in the state the server holds.
            continue;
        };

        let mut updated = existing.clone();
        match effect {
            SET_STATE => {
                // A wall that is not a door has no state to set. Turning one
                // into a door here would be an edit nobody asked for.
                if updated.door_state == DoorState::None {
                    continue;
                }
                let Some(next) = requested_state(&activation.config, updated.door_state) else {
                    continue;
                };
                updated.door_state = next;
            }
            SET_LOCK => {
                let Some(locked) = requested_lock(&activation.config) else {
                    continue;
                };
                updated.locked = locked;
            }
            REVEAL => {
                // One-way. Re-hiding something the table has seen is a fiction
                // problem rather than a state problem.
                updated.secret = false;
            }
            _ => continue,
        }
        wall_set.upsert(updated);
    }
}

#[cfg(test)]
#[path = "wall_tests.rs"]
mod tests;
