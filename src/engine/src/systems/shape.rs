//! Shape/annotation authoring input, rendering sync, undo, and visibility
//! filtering (T053-T055 of specs/001-bevy-canvas-authoring/tasks.md).
//!
//! Wiring: see `plugins/shape.rs`'s `ShapePlugin`.
//!
//! **Scope simplifications (called out per the task brief)**:
//! - Ellipse rendering (T018 of specs/002-canvas-authoring-asset-storage)
//!   draws a real ellipse outline as an N-segment polygon approximation,
//!   reusing the same `segment_sprite` sprite-chain technique `Stroke`
//!   already uses — deliberately not a `Mesh2d`/`ColorMaterial` asset,
//!   since `sync_shape_visuals` fully despawns and respawns every shape
//!   every pass (no change-detection guard) and per-frame `Assets<Mesh>`/
//!   `Assets<ColorMaterial>` inserts would leak without a much larger
//!   asset-caching change than this fix warrants.
//! - Text *input* (the on-canvas typing UI) is a frontend/UI concern, not
//!   built here. `ShapeTool.tsx`'s `TextPlacement` popover already handles
//!   this end-to-end (click the canvas container while the text sub-tool
//!   is active -> popover -> `createShape` mutation -> `upsert_shape`
//!   dispatch), confirmed during T019 of
//!   specs/002-canvas-authoring-asset-storage — no engine-side gap here.
//!   The engine only renders a `Text2d` for a `Text` shape whose `text`
//!   field arrives already filled in; it never needs to collect it.
//! - Restyling in v1 is a small hardcoded color-palette cycle (a `KeyC`
//!   keybind on the selected shape), matching the task brief's "read from
//!   a simple resource or hardcode a small palette cycle for v1, style
//!   refinement isn't the priority here."

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::{Value, json};

use crate::resources::{
    ActiveShapeTool, CanvasLayer, IsGameMaster, SelectedShape, Shape, ShapeEdit, ShapeKind,
    ShapeSet,
};
use crate::{ActiveWorld, emit_event};

/// Rendered height (px) of a stroke/line segment's thin sprite, same
/// technique as `systems/wall.rs`'s `WALL_VISUAL_HEIGHT`.
const SEGMENT_VISUAL_HEIGHT: f32 = 3.0;

/// Minimum drag distance (px) for a click-drag to count as a shape instead
/// of being rejected as a zero-length click (mirrors wall's `MIN_WALL_LENGTH`).
const MIN_SHAPE_SIZE: f32 = 1.0;

/// How close (px) the cursor must be to a shape to select it with a plain
/// click.
const SHAPE_SELECT_DISTANCE: f32 = 8.0;

const UNSELECTED_COLOR: Color = Color::srgb(0.4, 0.75, 0.95);
const SELECTED_COLOR: Color = Color::srgb(0.95, 0.85, 0.25);

/// Small hardcoded restyle palette (v1 simplification — see module doc).
const COLOR_PALETTE: [Color; 5] = [
    Color::srgb(0.4, 0.75, 0.95),
    Color::srgb(0.9, 0.3, 0.3),
    Color::srgb(0.3, 0.9, 0.4),
    Color::srgb(0.95, 0.7, 0.2),
    Color::srgb(0.8, 0.4, 0.9),
];

/// Marker on a sprite/text entity rendered for a given `ShapeSet` shape id.
#[derive(Component)]
pub(crate) struct ShapeVisual;

/// Maps `ShapeSet` shape ids to their spawned entity, mirroring the
/// `WallEntities`/`TokenEntities` pattern.
#[derive(Resource, Default)]
pub(crate) struct ShapeEntities(HashMap<String, Entity>);

#[derive(Default)]
enum ShapeDragMode {
    #[default]
    Idle,
    /// Click-dragging out a brand-new stroke/rect/ellipse/line from
    /// `start` to the live cursor position. For `Stroke`, `points`
    /// accumulates every sampled position while dragging.
    Creating { kind: ShapeKind, start: Vec2, points: Vec<Vec2> },
    /// Dragging an existing shape's whole body to a new position. `origin`
    /// is the cursor position at drag-start; `prior_geometry` is the
    /// shape's full geometry at drag-start, captured for the undo stack.
    Moving {
        shape_id: String,
        origin: Vec2,
        prior_geometry: Value,
    },
}

/// Session-local shape-tool drag state (not persisted, not part of
/// `ShapeSet`).
#[derive(Resource, Default)]
pub(crate) struct ShapeDragState {
    mode: ShapeDragMode,
}

/// Convert the cursor's window-pixel position into Bevy world space,
/// duplicated from `systems/wall.rs` (itself duplicated from
/// `systems/selection.rs`'s private helper — not exported, so each canvas-
/// authoring system module keeps its own copy rather than changing that
/// module's visibility for an unrelated feature).
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor_px).ok()
}

/// The anchor/center position used for hit-testing and moving a shape,
/// read out of its opaque `geometry` blob per the kind-specific contract
/// (contracts/graphql.md). Falls back to the origin if the geometry is
/// missing expected fields (defensive — geometry from the server is
/// trusted, but a shape mid-creation locally may not match yet).
fn shape_anchor(shape: &Shape) -> Vec2 {
    let g = &shape.geometry;
    match shape.kind {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let x = g["x"].as_f64().unwrap_or(0.0) as f32;
            let y = g["y"].as_f64().unwrap_or(0.0) as f32;
            let w = g["w"].as_f64().unwrap_or(0.0) as f32;
            let h = g["h"].as_f64().unwrap_or(0.0) as f32;
            Vec2::new(x + w / 2.0, y + h / 2.0)
        }
        ShapeKind::Line => {
            let x1 = g["x1"].as_f64().unwrap_or(0.0) as f32;
            let y1 = g["y1"].as_f64().unwrap_or(0.0) as f32;
            let x2 = g["x2"].as_f64().unwrap_or(0.0) as f32;
            let y2 = g["y2"].as_f64().unwrap_or(0.0) as f32;
            Vec2::new((x1 + x2) / 2.0, (y1 + y2) / 2.0)
        }
        ShapeKind::Text => {
            let x = g["x"].as_f64().unwrap_or(0.0) as f32;
            let y = g["y"].as_f64().unwrap_or(0.0) as f32;
            Vec2::new(x, y)
        }
        ShapeKind::Stroke => {
            let points = g["points"].as_array().cloned().unwrap_or_default();
            if points.is_empty() {
                return Vec2::ZERO;
            }
            let mut sum = Vec2::ZERO;
            let mut count = 0.0;
            for p in &points {
                if let Some(pair) = p.as_array() {
                    let x = pair.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = pair.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    sum += Vec2::new(x, y);
                    count += 1.0;
                }
            }
            if count > 0.0 { sum / count } else { Vec2::ZERO }
        }
    }
}

/// Translates every point-like field in a shape's geometry blob by
/// `delta`, used by move-drag. Kind-aware since each kind stores position
/// under different field names (contracts/graphql.md).
fn translate_geometry(kind: ShapeKind, geometry: &Value, delta: Vec2) -> Value {
    match kind {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let x = geometry["x"].as_f64().unwrap_or(0.0) as f32;
            let y = geometry["y"].as_f64().unwrap_or(0.0) as f32;
            let w = geometry["w"].as_f64().unwrap_or(0.0);
            let h = geometry["h"].as_f64().unwrap_or(0.0);
            json!({ "x": x + delta.x, "y": y + delta.y, "w": w, "h": h })
        }
        ShapeKind::Line => {
            let x1 = geometry["x1"].as_f64().unwrap_or(0.0) as f32;
            let y1 = geometry["y1"].as_f64().unwrap_or(0.0) as f32;
            let x2 = geometry["x2"].as_f64().unwrap_or(0.0) as f32;
            let y2 = geometry["y2"].as_f64().unwrap_or(0.0) as f32;
            json!({
                "x1": x1 + delta.x, "y1": y1 + delta.y,
                "x2": x2 + delta.x, "y2": y2 + delta.y,
            })
        }
        ShapeKind::Text => {
            let x = geometry["x"].as_f64().unwrap_or(0.0) as f32;
            let y = geometry["y"].as_f64().unwrap_or(0.0) as f32;
            json!({ "x": x + delta.x, "y": y + delta.y })
        }
        ShapeKind::Stroke => {
            let points: Vec<Value> = geometry["points"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|p| {
                    let pair = p.as_array().cloned().unwrap_or_default();
                    let x = pair.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = pair.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    json!([x + delta.x, y + delta.y])
                })
                .collect();
            json!({ "points": points })
        }
    }
}

/// Notifies the frontend of a selection change — same gap and same fix as
/// `systems/wall.rs`'s `emit_wall_selection` (T014/T015/T020,
/// specs/002-canvas-authoring-asset-storage): `SelectedShape` previously
/// only ever changed locally, so `ShapeTool.tsx`'s "Selected shape" panel
/// could never appear for a shape selected by clicking the canvas.
fn emit_shape_selection(shape_id: Option<&str>) {
    emit_event(json!({
        "type": "select_shape",
        "shapeId": shape_id,
    }));
}

fn shape_color(shape: &Shape, selected: bool) -> Color {
    if selected {
        return SELECTED_COLOR;
    }
    if let Some(style) = &shape.style
        && let Some(index) = style["colorIndex"].as_u64()
    {
        return COLOR_PALETTE[index as usize % COLOR_PALETTE.len()];
    }
    UNSELECTED_COLOR
}

/// T053: toolbar-driven tool selection via number keys 1-5 (Stroke, Rect,
/// Ellipse, Line, Text respectively), Escape clears the active tool back
/// to plain select/move mode. GM-only, same gating as wall input.
pub(crate) fn handle_shape_tool_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut active_tool: ResMut<ActiveShapeTool>,
    is_gm: Res<IsGameMaster>,
) {
    if !is_gm.0 {
        return;
    }

    if keyboard.just_pressed(KeyCode::Digit1) {
        active_tool.0 = Some(ShapeKind::Stroke);
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        active_tool.0 = Some(ShapeKind::Rect);
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        active_tool.0 = Some(ShapeKind::Ellipse);
    } else if keyboard.just_pressed(KeyCode::Digit4) {
        active_tool.0 = Some(ShapeKind::Line);
    } else if keyboard.just_pressed(KeyCode::Digit5) {
        active_tool.0 = Some(ShapeKind::Text);
    } else if keyboard.just_pressed(KeyCode::Escape) {
        active_tool.0 = None;
    }
}

/// T053: click-drag to draw with the active tool, or (with no active tool)
/// click to select and drag to move an existing shape. GM-only. A
/// press-and-release without meaningfully dragging/moving is rejected for
/// draw-tools (mirrors wall's T016 zero-length rejection); a plain click
/// with no tool active still selects the shape under the cursor.
pub(crate) fn handle_shape_input(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut shape_set: ResMut<ShapeSet>,
    mut selected_shape: ResMut<SelectedShape>,
    active_tool: Res<ActiveShapeTool>,
    mut drag: ResMut<ShapeDragState>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some(kind) = active_tool.0 {
            if kind == ShapeKind::Text {
                // Text placement is click-only; the frontend is expected to
                // prompt for the label and send `create_shape` with `text`
                // already filled in (module doc's text-input scope note) —
                // the engine side just needs a placement point to hand off.
                emit_event(json!({
                    "type": "create_shape",
                    "shape": {
                        "kind": "text",
                        "geometry": { "x": cursor.x, "y": cursor.y },
                        "text": Value::Null,
                        "style": Value::Null,
                        "visibleToPlayers": false,
                    },
                    "worldId": active_world.0,
                }));
                return;
            }
            selected_shape.deselect();
            emit_shape_selection(None);
            drag.mode = ShapeDragMode::Creating {
                kind,
                start: cursor,
                points: vec![cursor],
            };
            return;
        }

        // No active tool: select-by-click, or start a move-drag if the
        // click lands on the already-selected shape.
        for shape in shape_set.shapes() {
            if cursor.distance(shape_anchor(shape)) <= SHAPE_SELECT_DISTANCE {
                selected_shape.select(shape.id.clone());
                emit_shape_selection(Some(&shape.id));
                drag.mode = ShapeDragMode::Moving {
                    shape_id: shape.id.clone(),
                    origin: cursor,
                    prior_geometry: shape.geometry.clone(),
                };
                return;
            }
        }
        selected_shape.deselect();
        emit_shape_selection(None);
        drag.mode = ShapeDragMode::Idle;
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        match &mut drag.mode {
            ShapeDragMode::Creating { kind, points, .. } if *kind == ShapeKind::Stroke => {
                points.push(cursor);
            }
            ShapeDragMode::Moving {
                shape_id,
                origin,
                prior_geometry,
            } => {
                if let Some(shape) = shape_set.get(shape_id).cloned() {
                    let delta = cursor - *origin;
                    let mut moved = shape;
                    moved.geometry = translate_geometry(moved.kind, prior_geometry, delta);
                    // Optimistic local move so the sprite tracks the
                    // cursor; reconciled by the next `upsert_shape`
                    // confirmation from the server (mirrors wall's
                    // endpoint-drag optimistic-update comment).
                    shape_set.upsert(moved);
                }
            }
            _ => {}
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        match std::mem::take(&mut drag.mode) {
            ShapeDragMode::Creating { kind, start, points } => {
                let end = cursor;
                let geometry = match kind {
                    ShapeKind::Stroke => {
                        if points.len() < 2 {
                            return;
                        }
                        let pts: Vec<Value> = points.iter().map(|p| json!([p.x, p.y])).collect();
                        json!({ "points": pts })
                    }
                    ShapeKind::Rect | ShapeKind::Ellipse => {
                        if start.distance(end) < MIN_SHAPE_SIZE {
                            return;
                        }
                        let x = start.x.min(end.x);
                        let y = start.y.min(end.y);
                        let w = (end.x - start.x).abs();
                        let h = (end.y - start.y).abs();
                        json!({ "x": x, "y": y, "w": w, "h": h })
                    }
                    ShapeKind::Line => {
                        if start.distance(end) < MIN_SHAPE_SIZE {
                            return;
                        }
                        json!({ "x1": start.x, "y1": start.y, "x2": end.x, "y2": end.y })
                    }
                    ShapeKind::Text => unreachable!("Text is handled on press, not drag"),
                };

                emit_event(json!({
                    "type": "create_shape",
                    "shape": {
                        "kind": kind.as_str(),
                        "geometry": geometry,
                        "text": Value::Null,
                        "style": Value::Null,
                        "visibleToPlayers": false,
                    },
                    "worldId": active_world.0,
                }));
                // Deliberately no local ShapeSet entry yet: the server
                // assigns the shape's real id (mirrors wall creation).
            }
            ShapeDragMode::Moving {
                shape_id,
                prior_geometry,
                ..
            } => {
                if let Some(shape) = shape_set.get(&shape_id) {
                    emit_event(json!({
                        "type": "update_shape",
                        "shapeId": shape_id,
                        "changes": { "geometry": shape.geometry },
                        "worldId": active_world.0,
                    }));
                    shape_set.push_undo(ShapeEdit::Move {
                        shape_id,
                        prior_geometry,
                    });
                }
            }
            ShapeDragMode::Idle => {}
        }
    }
}

/// T053: keybound restyle (palette cycle) and Delete/Backspace to remove
/// the selected shape. GM-only, same gating as `handle_shape_input`.
///
/// Keybinds (chosen to avoid existing bindings — `lib.rs`/`plugins/camera.rs`
/// and `systems/wall.rs`'s `V`/`B`/`O`):
/// - `C`: cycle to the next color in `COLOR_PALETTE`
/// - `Delete`/`Backspace`: delete the selected shape
/// - `P`: toggle GM-only / visible-to-players
pub(crate) fn handle_shape_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shape_set: ResMut<ShapeSet>,
    mut selected_shape: ResMut<SelectedShape>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(shape_id) = selected_shape.get_selected().cloned() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(shape) = shape_set.get(&shape_id).cloned() {
            let prior_style = shape.style.clone();
            let current_index = shape
                .style
                .as_ref()
                .and_then(|s| s["colorIndex"].as_u64())
                .unwrap_or(0);
            let next_index = (current_index + 1) % COLOR_PALETTE.len() as u64;
            let mut updated = shape;
            let new_style = json!({ "colorIndex": next_index });
            updated.style = Some(new_style.clone());
            shape_set.upsert(updated);
            shape_set.push_undo(ShapeEdit::Restyle {
                shape_id: shape_id.clone(),
                prior_style,
            });
            emit_event(json!({
                "type": "update_shape",
                "shapeId": shape_id,
                "changes": { "style": new_style },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        if let Some(shape) = shape_set.get(&shape_id).cloned() {
            let prior_visible_to_players = shape.visible_to_players;
            let mut updated = shape;
            updated.visible_to_players = !updated.visible_to_players;
            shape_set.upsert(updated.clone());
            shape_set.push_undo(ShapeEdit::VisibilityToggle {
                shape_id: shape_id.clone(),
                prior_visible_to_players,
            });
            emit_event(json!({
                "type": "update_shape",
                "shapeId": shape_id,
                "changes": { "visibleToPlayers": updated.visible_to_players },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Some(deleted) = shape_set.remove(&shape_id) {
            shape_set.push_undo(ShapeEdit::Delete { deleted });
            selected_shape.deselect();
            emit_shape_selection(None);
            emit_event(json!({
                "type": "delete_shape",
                "shapeId": shape_id,
                "worldId": active_world.0,
            }));
        }
    }
}

/// T054: shape undo. Ctrl+Z pops `ShapeSet`'s undo stack and re-issues the
/// inverse mutation through the same outbound-event path a normal edit
/// uses (research.md §4) — applied locally first (optimistic), then
/// emitted so other clients converge once the server confirms it. Mirrors
/// `systems/wall.rs`'s `handle_wall_undo`.
pub(crate) fn handle_shape_undo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shape_set: ResMut<ShapeSet>,
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

    let Some(edit) = shape_set.pop_undo() else {
        return;
    };

    match edit {
        ShapeEdit::Move {
            shape_id,
            prior_geometry,
        } => {
            if let Some(mut shape) = shape_set.get(&shape_id).cloned() {
                shape.geometry = prior_geometry.clone();
                shape_set.upsert(shape);
            }
            emit_event(json!({
                "type": "update_shape",
                "shapeId": shape_id,
                "changes": { "geometry": prior_geometry },
                "worldId": active_world.0,
            }));
        }
        ShapeEdit::Restyle {
            shape_id,
            prior_style,
        } => {
            if let Some(mut shape) = shape_set.get(&shape_id).cloned() {
                shape.style = prior_style.clone();
                shape_set.upsert(shape);
            }
            emit_event(json!({
                "type": "update_shape",
                "shapeId": shape_id,
                "changes": { "style": prior_style },
                "worldId": active_world.0,
            }));
        }
        ShapeEdit::VisibilityToggle {
            shape_id,
            prior_visible_to_players,
        } => {
            if let Some(mut shape) = shape_set.get(&shape_id).cloned() {
                shape.visible_to_players = prior_visible_to_players;
                shape_set.upsert(shape);
            }
            emit_event(json!({
                "type": "update_shape",
                "shapeId": shape_id,
                "changes": { "visibleToPlayers": prior_visible_to_players },
                "worldId": active_world.0,
            }));
        }
        ShapeEdit::Delete { deleted } => {
            // Re-creates the shape; the server assigns a new id (mirrors
            // wall delete-undo — the original id cannot be resurrected).
            emit_event(json!({
                "type": "create_shape",
                "shape": {
                    "kind": deleted.kind.as_str(),
                    "geometry": deleted.geometry,
                    "text": deleted.text,
                    "style": deleted.style,
                    "visibleToPlayers": deleted.visible_to_players,
                },
                "worldId": active_world.0,
            }));
        }
    }
}

/// T053/T055: keeps one rendered entity per `ShapeSet` shape id in sync
/// (spawn on new id, update on change, despawn on removal) — mirrors
/// `systems/wall.rs`'s `sync_wall_visuals`.
///
/// Rendering per kind (first-pass, per the module doc's scope notes):
/// - `Stroke`: a chain of thin rotated sprites between consecutive points
///   (same technique `systems/wall.rs` uses for wall segments).
/// - `Rect`/`Ellipse`: a single scaled sprite. Ellipse uses a rectangle
///   placeholder rather than true curve geometry (acceptable v1
///   simplification, see module doc).
/// - `Line`: a single thin rotated sprite (reuses the wall-segment
///   technique).
/// - `Text`: a `Text2d` entity, mirroring `setup_scene`'s
///   `Text`/`TextFont`/`TextColor` UI-text pattern but positioned in world
///   space via `Transform` instead of a UI `Node`.
///
/// T055: GM-only shapes (`visible_to_players == false`) are skipped
/// entirely for non-GM sessions — defense in depth on top of the server's
/// own `visible_to_players` query filter (contracts/graphql.md), which
/// already means a non-GM session never receives them via `upsert_shape`
/// in practice.
pub(crate) fn sync_shape_visuals(
    mut commands: Commands,
    shape_set: Res<ShapeSet>,
    selected_shape: Res<SelectedShape>,
    is_gm: Res<IsGameMaster>,
    mut shape_entities: ResMut<ShapeEntities>,
) {
    let z = CanvasLayer::Shapes.z();

    // T055: shapes with `visible_to_players == false` are excluded for
    // non-GM sessions entirely (defense in depth — see module doc).
    let visible_ids: std::collections::HashSet<String> = shape_set
        .shapes()
        .iter()
        .filter(|s| is_gm.0 || s.visible_to_players)
        .map(|s| s.id.clone())
        .collect();

    // Full despawn-and-respawn each pass rather than diffing: strokes are
    // multi-entity chains keyed by one shape id, and shape counts here are
    // small enough (mirrors wall's endpoint-handle rebuild) that this isn't
    // a hot path.
    for (_, entity) in shape_entities.0.drain() {
        commands.entity(entity).despawn();
    }

    for shape in shape_set.shapes() {
        if !visible_ids.contains(&shape.id) {
            continue;
        }
        let selected = selected_shape.is_selected(&shape.id);
        let color = shape_color(shape, selected);
        let translation_z = if selected { z + 1.0 } else { z };

        let entity = match shape.kind {
            ShapeKind::Rect => {
                let g = &shape.geometry;
                let x = g["x"].as_f64().unwrap_or(0.0) as f32;
                let y = g["y"].as_f64().unwrap_or(0.0) as f32;
                let w = g["w"].as_f64().unwrap_or(1.0).max(1.0) as f32;
                let h = g["h"].as_f64().unwrap_or(1.0).max(1.0) as f32;
                let center = Vec2::new(x + w / 2.0, y + h / 2.0);
                commands
                    .spawn((
                        Sprite::from_color(color, Vec2::new(w, h)),
                        Transform::from_translation(center.extend(translation_z)),
                        ShapeVisual,
                    ))
                    .id()
            }
            ShapeKind::Ellipse => {
                let g = &shape.geometry;
                let x = g["x"].as_f64().unwrap_or(0.0) as f32;
                let y = g["y"].as_f64().unwrap_or(0.0) as f32;
                let w = g["w"].as_f64().unwrap_or(1.0).max(1.0) as f32;
                let h = g["h"].as_f64().unwrap_or(1.0).max(1.0) as f32;
                let center = Vec2::new(x + w / 2.0, y + h / 2.0);
                let points = ellipse_outline_points(center, w / 2.0, h / 2.0, ELLIPSE_SEGMENTS);

                let parent = commands
                    .spawn((Transform::default(), Visibility::Inherited, ShapeVisual))
                    .id();
                for pair in points.windows(2) {
                    let child = commands
                        .spawn(segment_sprite(pair[0], pair[1], color, translation_z))
                        .id();
                    commands.entity(parent).add_child(child);
                }
                parent
            }
            ShapeKind::Line => {
                let g = &shape.geometry;
                let start = Vec2::new(
                    g["x1"].as_f64().unwrap_or(0.0) as f32,
                    g["y1"].as_f64().unwrap_or(0.0) as f32,
                );
                let end = Vec2::new(
                    g["x2"].as_f64().unwrap_or(0.0) as f32,
                    g["y2"].as_f64().unwrap_or(0.0) as f32,
                );
                commands.spawn(segment_sprite(start, end, color, translation_z)).id()
            }
            ShapeKind::Stroke => {
                let points: Vec<Vec2> = shape.geometry["points"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|p| {
                        let pair = p.as_array()?;
                        let x = pair.first()?.as_f64()? as f32;
                        let y = pair.get(1)?.as_f64()? as f32;
                        Some(Vec2::new(x, y))
                    })
                    .collect();

                // Spawn a segment per consecutive pair, tracked under a
                // single parent entity so it despawns as one unit.
                let parent = commands
                    .spawn((Transform::default(), Visibility::Inherited, ShapeVisual))
                    .id();
                for pair in points.windows(2) {
                    let child = commands
                        .spawn(segment_sprite(pair[0], pair[1], color, translation_z))
                        .id();
                    commands.entity(parent).add_child(child);
                }
                parent
            }
            ShapeKind::Text => {
                let g = &shape.geometry;
                let x = g["x"].as_f64().unwrap_or(0.0) as f32;
                let y = g["y"].as_f64().unwrap_or(0.0) as f32;
                let label = shape.text.clone().unwrap_or_default();
                commands
                    .spawn((
                        Text2d::new(label),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(color),
                        Transform::from_translation(Vec2::new(x, y).extend(translation_z)),
                        ShapeVisual,
                    ))
                    .id()
            }
        };

        shape_entities.0.insert(shape.id.clone(), entity);
    }
}

/// Segment count for the ellipse polygon approximation (T018). High enough
/// to read as a smooth curve at typical scene zoom levels, low enough to
/// stay well within the per-shape entity counts `Stroke` already produces.
const ELLIPSE_SEGMENTS: usize = 32;

/// Closed-loop points tracing an ellipse centered at `center` with
/// semi-axes `rx`/`ry`, `segments` points around the loop plus the
/// closing point back to the start (T018: real ellipse geometry instead
/// of the old rectangle placeholder, rendered via the same
/// sprite-segment-chain technique `Stroke` uses).
fn ellipse_outline_points(center: Vec2, rx: f32, ry: f32, segments: usize) -> Vec<Vec2> {
    (0..=segments)
        .map(|i| {
            let t = (i as f32 / segments as f32) * std::f32::consts::TAU;
            center + Vec2::new(rx * t.cos(), ry * t.sin())
        })
        .collect()
}

/// A thin rotated sprite spanning `start`-`end`, same technique
/// `systems/wall.rs`'s `sync_wall_visuals` uses for wall segments —
/// shared here by `Line` shapes, each segment of a `Stroke`, and each
/// segment of an `Ellipse`'s polygon outline.
fn segment_sprite(
    start: Vec2,
    end: Vec2,
    color: Color,
    z: f32,
) -> (Sprite, Transform, ShapeVisual) {
    let midpoint = (start + end) / 2.0;
    let length = start.distance(end).max(0.5);
    let delta = end - start;
    let angle = delta.y.atan2(delta.x);
    (
        Sprite::from_color(color, Vec2::new(length, SEGMENT_VISUAL_HEIGHT)),
        Transform {
            translation: midpoint.extend(z),
            rotation: Quat::from_rotation_z(angle),
            ..default()
        },
        ShapeVisual,
    )
}

pub(crate) fn init_shape_systems_resources(app: &mut App) {
    app.init_resource::<ShapeDragState>()
        .init_resource::<ShapeEntities>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rect_shape(id: &str, x: f32, y: f32, w: f32, h: f32) -> Shape {
        Shape {
            id: id.to_string(),
            kind: ShapeKind::Rect,
            geometry: json!({ "x": x, "y": y, "w": w, "h": h }),
            text: None,
            style: None,
            visible_to_players: false,
        }
    }

    #[test]
    fn ellipse_outline_points_are_closed_and_on_the_ellipse() {
        let points = ellipse_outline_points(Vec2::new(10.0, 20.0), 4.0, 2.0, 8);
        assert_eq!(points.len(), 9); // 8 segments + closing point back to start
        assert_eq!(points[0], points[8]); // closed loop
        for p in &points {
            let dx = (p.x - 10.0) / 4.0;
            let dy = (p.y - 20.0) / 2.0;
            assert!((dx * dx + dy * dy - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn shape_anchor_rect_is_center() {
        let s = rect_shape("s1", 0.0, 0.0, 10.0, 20.0);
        assert_eq!(shape_anchor(&s), Vec2::new(5.0, 10.0));
    }

    #[test]
    fn shape_anchor_text_is_the_point_itself() {
        let s = Shape {
            id: "s1".to_string(),
            kind: ShapeKind::Text,
            geometry: json!({ "x": 3.0, "y": 4.0 }),
            text: Some("hi".to_string()),
            style: None,
            visible_to_players: false,
        };
        assert_eq!(shape_anchor(&s), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn translate_geometry_rect_shifts_position_keeps_size() {
        let g = json!({ "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 });
        let moved = translate_geometry(ShapeKind::Rect, &g, Vec2::new(5.0, -5.0));
        assert_eq!(moved["x"], 5.0);
        assert_eq!(moved["y"], -5.0);
        assert_eq!(moved["w"], 10.0);
        assert_eq!(moved["h"], 10.0);
    }

    #[test]
    fn translate_geometry_stroke_shifts_every_point() {
        let g = json!({ "points": [[0.0, 0.0], [1.0, 1.0]] });
        let moved = translate_geometry(ShapeKind::Stroke, &g, Vec2::new(2.0, 3.0));
        let points = moved["points"].as_array().unwrap();
        assert_eq!(points[0], json!([2.0, 3.0]));
        assert_eq!(points[1], json!([3.0, 4.0]));
    }

    #[test]
    fn shape_color_prioritizes_selection_over_style() {
        let mut s = rect_shape("s1", 0.0, 0.0, 1.0, 1.0);
        s.style = Some(json!({ "colorIndex": 1 }));
        assert_eq!(shape_color(&s, true), SELECTED_COLOR);
        assert_eq!(shape_color(&s, false), COLOR_PALETTE[1]);
    }
}
