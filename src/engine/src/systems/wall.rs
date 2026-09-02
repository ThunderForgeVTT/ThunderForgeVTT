//! Wall authoring input, rendering sync, undo, and vision-occlusion systems
//! (T012-T014, T016 of specs/001-bevy-canvas-authoring/tasks.md).
//!
//! Wiring: see `plugins/wall.rs`'s `WallPlugin`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;
use thunderforge_canvas_core::snapping::SnapRule;

use crate::resources::{
    ActiveWallPrimitive, CanvasLayer, DoorState, IsGameMaster, SelectedWall, Wall, WallEdit,
    WallPrimitive, WallSet,
};
use crate::{ActiveWorld, emit_event};

/// Rendered height (px) of a wall's thin sprite (T012: "a small fixed
/// height like 4.0 px").
const WALL_VISUAL_HEIGHT: f32 = 4.0;

/// Minimum drag distance (px) for a click-drag to count as a wall instead
/// of being rejected as a zero-length click (T016).
const MIN_WALL_LENGTH: f32 = 1.0;

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
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_px)
        .ok()
}

/// Shortest distance from `point` to the segment `a`-`b`. Pure/testable —
/// used for wall body hit-testing (select-by-click).
fn distance_point_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
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

/// Emit the four walls of a room drawn between two snapped corners.
///
/// FR-026. The geometry is `thunderforge_canvas_core::wall::room_segments`,
/// which is where it can be tested for the property that matters — that the
/// four segments share their corner points exactly, so the room encloses its
/// interior rather than leaking light through a seam a fraction of a unit
/// wide.
///
/// A degenerate drag emits nothing. The gesture did not describe a room, and
/// four walls stacked along a line is a worse answer than none.
fn emit_room(start: Vec2, end: Vec2, world_id: &str) {
    let Some(segments) = thunderforge_canvas_core::wall::room_segments(start, end) else {
        return;
    };

    for (from, to) in segments {
        emit_event(json!({
            "type": "create_wall",
            "wall": {
                "x1": from.x,
                "y1": from.y,
                "x2": to.x,
                "y2": to.y,
                "blocksVision": true,
                "blocksMovement": false,
                "doorState": "none",
            },
            "worldId": world_id,
        }));
    }
}

/// Emit one wall that is already a door.
///
/// FR-027: a door drawn this way is a functional door, and it is functional
/// because it is *the same kind of door* the tool has always produced — a wall
/// whose `door_state` is not `None`. Everything that acts on doors keys off
/// exactly that: `handle_door_effects` performs the contributed
/// `door.set_state` / `door.set_lock` / `door.reveal` effects, the `O` keybind
/// cycles the selected wall's state, `wall_color` draws it as a door, and
/// `Wall::blocking` derives what it stops from the state it is in. None of
/// them is taught about this primitive, and none of them needs to be —
/// inventing a second kind of door here is exactly what would break them.
///
/// It blocks movement, where a plain wall from this tool does not. That is the
/// difference between a wall and a doorway: a closed door is a way through
/// that happens to be shut, so it has to stop somebody while it is shut and
/// stop nobody once it opens, which `Wall::blocking` already derives.
fn emit_door(start: Vec2, end: Vec2, world_id: &str) {
    emit_event(json!({
        "type": "create_wall",
        "wall": {
            "x1": start.x,
            "y1": start.y,
            "x2": end.x,
            "y2": end.y,
            "blocksVision": true,
            "blocksMovement": true,
            // Closed, not open. A door drawn onto a map is a door in a wall,
            // and a Game Master who wanted an opening would have drawn none.
            "doorState": "closed",
        },
        "worldId": world_id,
    }));
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
    windows: Query<&Window, With<PrimaryWindow>>,
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

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
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
            let moved = rule.vertex(cursor);
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
                let start = rule.vertex(start);
                let end = rule.vertex(cursor);

                match primitive.0 {
                    WallPrimitive::Room => {
                        emit_room(start, end, &active_world.0);
                        return;
                    }
                    WallPrimitive::Door => {
                        if start.distance(end) >= MIN_WALL_LENGTH {
                            emit_door(start, end, &active_world.0);
                        }
                        // A click with no drag draws no door. Unlike the
                        // segment tool it does not seed a chain either: a
                        // chain of doors is not a thing, and silently starting
                        // one would make the next click somewhere else produce
                        // a door across the room.
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

                emit_event(json!({
                    "type": "create_wall",
                    "wall": {
                        "x1": start.x,
                        "y1": start.y,
                        "x2": end.x,
                        "y2": end.y,
                        "blocksVision": true,
                        "blocksMovement": false,
                        "doorState": "none",
                    },
                    "worldId": active_world.0,
                }));
                // Deliberately no local WallSet entry yet: the server
                // assigns the wall's real id, so this stays untracked
                // until the matching `upsert_wall` command arrives (see
                // module doc / WallPlugin for the rationale).
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
            for pair in points.windows(2) {
                emit_event(json!({
                    "type": "create_wall",
                    "wall": {
                        "x1": pair[0].x,
                        "y1": pair[0].y,
                        "x2": pair[1].x,
                        "y2": pair[1].y,
                        "blocksVision": true,
                        "blocksMovement": false,
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

/// T014: wall undo. Ctrl+Z pops `WallSet`'s undo stack and re-issues the
/// inverse mutation through the same outbound-event path a normal edit
/// uses (research.md §4) — applied locally first (optimistic), then
/// emitted so other clients converge once the server confirms it.
pub(crate) fn handle_wall_undo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wall_set: ResMut<WallSet>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !ctrl || !keyboard.just_pressed(KeyCode::KeyZ) {
        return;
    }

    let Some(edit) = wall_set.pop_undo() else {
        return;
    };

    match edit {
        WallEdit::Move {
            wall_id,
            prior_x1,
            prior_y1,
            prior_x2,
            prior_y2,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.x1 = prior_x1;
                wall.y1 = prior_y1;
                wall.x2 = prior_x2;
                wall.y2 = prior_y2;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "x1": prior_x1, "y1": prior_y1, "x2": prior_x2, "y2": prior_y2 },
                "worldId": active_world.0,
            }));
        }
        WallEdit::DoorToggle {
            wall_id,
            prior_door_state,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.door_state = prior_door_state;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "doorState": prior_door_state.as_str() },
                "worldId": active_world.0,
            }));
        }
        WallEdit::FlagsToggle {
            wall_id,
            prior_blocks_vision,
            prior_blocks_movement,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.blocks_vision = prior_blocks_vision;
                wall.blocks_movement = prior_blocks_movement;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": {
                    "blocksVision": prior_blocks_vision,
                    "blocksMovement": prior_blocks_movement,
                },
                "worldId": active_world.0,
            }));
        }
        WallEdit::Delete { deleted } => {
            // Re-creates the wall; the server assigns a new id (the
            // original id cannot be resurrected — see the module's
            // scope note on optimistic reconciliation).
            emit_event(json!({
                "type": "create_wall",
                "wall": {
                    "x1": deleted.x1,
                    "y1": deleted.y1,
                    "x2": deleted.x2,
                    "y2": deleted.y2,
                    "blocksVision": deleted.blocks_vision,
                    "blocksMovement": deleted.blocks_movement,
                    "doorState": deleted.door_state.as_str(),
                },
                "worldId": active_world.0,
            }));
        }
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
    let z = CanvasLayer::Walls.z();

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
mod tests {
    use super::*;

    #[test]
    fn distance_point_to_segment_on_the_line() {
        let d = distance_point_to_segment(
            Vec2::new(5.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
        );
        assert!(d.abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_perpendicular() {
        let d = distance_point_to_segment(
            Vec2::new(5.0, 3.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
        );
        assert!((d - 3.0).abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_beyond_endpoint() {
        let d = distance_point_to_segment(
            Vec2::new(15.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
        );
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_degenerate_segment() {
        // Zero-length segment: distance is just distance to the point.
        let d = distance_point_to_segment(
            Vec2::new(3.0, 4.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 0.0),
        );
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn wall_color_prioritizes_selection_over_door() {
        let wall = Wall {
            id: "w1".to_string(),
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
            blocks_vision: true,
            blocks_movement: false,
            door_state: DoorState::Closed,
        };
        assert_eq!(wall_color(&wall, true), SELECTED_COLOR);
        assert_eq!(wall_color(&wall, false), DOOR_COLOR);
    }
}
