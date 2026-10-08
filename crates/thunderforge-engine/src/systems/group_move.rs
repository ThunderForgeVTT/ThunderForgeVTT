//! Moving and deleting a group as one (spec 085, R6 and R10).
//!
//! A press on a member of a group of two or more, dragged past the click
//! threshold, shows every member moved by one offset: the cursor's travel from
//! the press, snapped the way the pressed item snaps. On release each member
//! gets the event its own tool already sends, so the server sees N ordinary
//! changes, each stamped with the same `group` so the web can tell the mover
//! once how many were refused.
//!
//! Delete and Backspace with a group of two or more, or the Select bar's
//! Delete (`delete_selection`), send one delete per member the viewer may
//! delete.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use serde_json::{Value, json};

use thunderforge_canvas_core::camera::DRAG_THRESHOLD_PX;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::shape_geometry::translate;
use thunderforge_canvas_core::snapping::SnapRule;

use crate::payloads::ActiveWorld;
use crate::resources::{
    GridSnapEnabled, GroupKind, GroupSelection, IsGameMaster, LightSet, LightSource, SceneGrid,
    Shape, ShapeSet, ViewerUserId, Wall, WallSet,
};
use crate::systems::box_select::{BoardItems, SelectionParams, emit_group, shift_held};
use crate::systems::shape_authority::may_edit_shape;
use crate::{TokenIdentity, emit_event};

/// The stamp every event of one group move or delete carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupStamp {
    pub id: String,
    pub size: usize,
}

impl GroupStamp {
    fn to_json(&self) -> Value {
        json!({ "id": self.id, "size": self.size })
    }
}

/// Hands out group ids, `g-<n>`, unique for this board session.
#[derive(Resource, Debug, Default)]
pub struct GroupStamps(u64);

impl GroupStamps {
    pub fn next(&mut self, size: usize) -> GroupStamp {
        self.0 += 1;
        GroupStamp {
            id: format!("g-{}", self.0),
            size,
        }
    }
}

/// One member as it was when the drag began.
#[derive(Debug, Clone)]
pub enum Member {
    Token {
        id: String,
        at: Vec3,
        scale: f32,
        rotation: f32,
    },
    Wall(Wall),
    Light(LightSource),
    Shape(Shape),
}

impl Member {
    /// This member moved by `offset`, as the event its own tool sends.
    fn moved_event(&self, offset: Vec2, world_id: &str) -> Value {
        match self {
            Member::Token {
                id,
                at,
                scale,
                rotation,
            } => json!({
                "type": "upsert_token",
                "token": {
                    "id": id,
                    "x": at.x + offset.x,
                    "y": at.y + offset.y,
                    "z": at.z,
                    "scale": scale,
                    "rotation": rotation,
                },
                "worldId": world_id,
            }),
            Member::Wall(wall) => json!({
                "type": "update_wall",
                "wallId": wall.id,
                "changes": {
                    "x1": wall.x1 + offset.x,
                    "y1": wall.y1 + offset.y,
                    "x2": wall.x2 + offset.x,
                    "y2": wall.y2 + offset.y,
                },
                "worldId": world_id,
            }),
            Member::Light(light) => json!({
                "type": "update_light",
                "lightId": light.id,
                "changes": { "x": light.x + offset.x, "y": light.y + offset.y },
                "worldId": world_id,
            }),
            Member::Shape(shape) => json!({
                "type": "update_shape",
                "shapeId": shape.id,
                "changes": { "geometry": translate(shape.kind, &shape.geometry, offset) },
                "worldId": world_id,
            }),
        }
    }
}

/// Every member's event for a move by `offset`, each carrying `stamp`.
pub fn release_events(
    members: &[Member],
    offset: Vec2,
    stamp: &GroupStamp,
    world_id: &str,
) -> Vec<Value> {
    members
        .iter()
        .map(|member| {
            let mut event = member.moved_event(offset, world_id);
            event["group"] = stamp.to_json();
            event
        })
        .collect()
}

/// How the pressed item snaps, which decides how the whole group snaps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    /// A token: its centre, its footprint, and whether it snaps at all.
    Token {
        center: Vec2,
        footprint: Footprint,
        snaps: bool,
    },
    /// A wall: its first end, which its tool puts on a lattice corner.
    Wall { start: Vec2 },
    /// A light: its point, which its tool puts on a cell centre.
    Light { at: Vec2 },
    /// A shape, which its tool does not snap.
    Free,
}

/// The cursor's `raw` travel, snapped the way the pressed item snaps: the
/// offset that lands the pressed item where its own tool would put it.
pub fn snapped_offset(anchor: Anchor, raw: Vec2, rule: &SnapRule) -> Vec2 {
    match anchor {
        Anchor::Token {
            center,
            footprint,
            snaps,
        } => {
            if snaps {
                rule.token(center + raw, footprint) - center
            } else {
                raw
            }
        }
        Anchor::Wall { start } => rule.vertex(start + raw) - start,
        Anchor::Light { at } => rule.cell(at + raw) - at,
        Anchor::Free => raw,
    }
}

/// A group drag in progress.
#[derive(Resource, Debug, Default)]
pub struct GroupDrag {
    /// The press, in world and screen space.
    press: Option<(Vec2, Vec2)>,
    anchor: Option<Anchor>,
    members: Vec<Member>,
    /// Past the click threshold: the members are being shown moved.
    moving: bool,
    /// The snapped offset last shown.
    offset: Vec2,
}

/// Start a group drag on a plain press on a member of a group of two or more.
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_group_drag(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    board: BoardItems,
    group: Res<GroupSelection>,
    mut drag: ResMut<GroupDrag>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }
    *drag = GroupDrag::default();
    if group.len() < 2 || shift_held(keys.as_deref()) {
        return;
    }
    let Some(cursor_px) = pointer.position() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };
    let Ok(cursor) = camera.viewport_to_world_2d(camera_transform, cursor_px) else {
        return;
    };
    let Some((kind, pressed)) = board.member_at(&group, cursor) else {
        return;
    };
    let (members, anchor) = snapshot(&board, &group, kind, &pressed);
    *drag = GroupDrag {
        press: Some((cursor, cursor_px)),
        anchor,
        members,
        moving: false,
        offset: Vec2::ZERO,
    };
}

/// Every member as it stands, and the pressed item's snapping.
fn snapshot(
    board: &BoardItems,
    group: &GroupSelection,
    kind: GroupKind,
    pressed: &str,
) -> (Vec<Member>, Option<Anchor>) {
    let mut members = Vec::with_capacity(group.len());
    let mut anchor = None;
    for (transform, identity, behaviour, _, _) in board.tokens.iter() {
        if !group.tokens.contains(&identity.0) {
            continue;
        }
        if kind == GroupKind::Token && identity.0 == pressed {
            let behaviour = behaviour.copied().unwrap_or_default();
            anchor = Some(Anchor::Token {
                center: transform.translation.truncate(),
                footprint: behaviour.footprint,
                snaps: behaviour.snap,
            });
        }
        members.push(Member::Token {
            id: identity.0.clone(),
            at: transform.translation,
            scale: transform.scale.x,
            rotation: transform.rotation.to_euler(EulerRot::ZYX).0,
        });
    }
    if let Some(walls) = &board.walls {
        for id in &group.walls {
            if let Some(wall) = walls.get(id) {
                if kind == GroupKind::Wall && id == pressed {
                    anchor = Some(Anchor::Wall {
                        start: wall.start(),
                    });
                }
                members.push(Member::Wall(wall.clone()));
            }
        }
    }
    if let Some(lights) = &board.lights {
        for id in &group.lights {
            if let Some(light) = lights.get(id).filter(|l| !l.is_carried()) {
                if kind == GroupKind::Light && id == pressed {
                    anchor = Some(Anchor::Light {
                        at: light.position(),
                    });
                }
                members.push(Member::Light(light.clone()));
            }
        }
    }
    if let Some(shapes) = &board.shapes {
        for id in &group.shapes {
            if let Some(shape) = shapes.get(id) {
                if kind == GroupKind::Shape && id == pressed {
                    anchor = Some(Anchor::Free);
                }
                members.push(Member::Shape(shape.clone()));
            }
        }
    }
    (members, anchor)
}

/// Show the members moved while the drag is held, and send them on release.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_group_drag(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<GroupDrag>,
    mut tokens: Query<(&mut Transform, &TokenIdentity)>,
    mut walls: Option<ResMut<WallSet>>,
    mut lights: Option<ResMut<LightSet>>,
    mut shapes: Option<ResMut<ShapeSet>>,
    grid: Option<Res<SceneGrid>>,
    snap_enabled: Option<Res<GridSnapEnabled>>,
    mut stamps: ResMut<GroupStamps>,
    active_world: Res<ActiveWorld>,
) {
    let Some((press, press_px)) = drag.press else {
        return;
    };
    let released = mouse_button.just_released(MouseButton::Left);
    if !released && !mouse_button.pressed(MouseButton::Left) {
        *drag = GroupDrag::default();
        return;
    }
    // Where the cursor is, if it can be said: a release whose frame has no
    // pointer still lands at the offset last shown.
    let cursor = pointer.position().and_then(|px| {
        let (camera, camera_transform) = camera_query.iter().next()?;
        let world = camera.viewport_to_world_2d(camera_transform, px).ok()?;
        Some((world, px))
    });
    if let Some((_, cursor_px)) = cursor
        && cursor_px.distance(press_px) > DRAG_THRESHOLD_PX
    {
        drag.moving = true;
    }
    if !drag.moving {
        if released {
            *drag = GroupDrag::default();
        }
        return;
    }

    if let Some((cursor, _)) = cursor {
        let rule = SnapRule::new(
            grid.map(|g| g.0).unwrap_or_default(),
            snap_enabled.is_none_or(|s| s.0),
        );
        drag.offset = drag.anchor.map_or(cursor - press, |anchor| {
            snapped_offset(anchor, cursor - press, &rule)
        });
    }
    let offset = drag.offset;

    // The preview, every frame of the drag, and the last frame's on release.
    for member in &drag.members {
        match member {
            Member::Token { id, at, .. } => {
                if let Some((mut transform, _)) =
                    tokens.iter_mut().find(|(_, identity)| &identity.0 == id)
                {
                    transform.translation.x = at.x + offset.x;
                    transform.translation.y = at.y + offset.y;
                }
            }
            Member::Wall(wall) => {
                if let Some(walls) = walls.as_mut() {
                    let mut moved = wall.clone();
                    moved.x1 += offset.x;
                    moved.y1 += offset.y;
                    moved.x2 += offset.x;
                    moved.y2 += offset.y;
                    walls.upsert(moved);
                }
            }
            Member::Light(light) => {
                if let Some(lights) = lights.as_mut() {
                    let mut moved = light.clone();
                    moved.x += offset.x;
                    moved.y += offset.y;
                    lights.upsert(moved);
                }
            }
            Member::Shape(shape) => {
                if let Some(shapes) = shapes.as_mut() {
                    let mut moved = shape.clone();
                    moved.geometry = translate(shape.kind, &shape.geometry, offset);
                    shapes.upsert(moved);
                }
            }
        }
    }

    if released {
        let members = std::mem::take(&mut drag.members);
        *drag = GroupDrag::default();
        if offset == Vec2::ZERO || members.is_empty() {
            return;
        }
        let stamp = stamps.next(members.len());
        for event in release_events(&members, offset, &stamp, &active_world.0) {
            emit_event(event);
        }
    }
}

static DELETE_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Ask for the group to be deleted on the next frame: the Select bar's
/// Delete, through `delete_selection()` or the `delete_group` command.
pub(crate) fn request_delete() {
    DELETE_REQUESTED.store(true, Ordering::Release);
}

/// What the viewer may delete of `group`. A Game Master deletes every
/// member; a player only the shapes they may edit, since players do not
/// delete tokens, walls or lights.
pub fn deletable(
    group: &GroupSelection,
    is_gm: bool,
    viewer: Option<&str>,
    shape_of: impl Fn(&str) -> Option<Shape>,
    carried: impl Fn(&str) -> bool,
) -> GroupSelection {
    if is_gm {
        return GroupSelection {
            tokens: group.tokens.clone(),
            walls: group.walls.clone(),
            lights: group
                .lights
                .iter()
                .filter(|id| !carried(id))
                .cloned()
                .collect(),
            shapes: group.shapes.clone(),
        };
    }
    GroupSelection {
        shapes: group
            .shapes
            .iter()
            .filter(|id| shape_of(id).is_some_and(|shape| may_edit_shape(false, viewer, &shape)))
            .cloned()
            .collect(),
        ..Default::default()
    }
}

/// One delete per item of `doomed`, each carrying `stamp`.
pub fn delete_events(doomed: &GroupSelection, stamp: &GroupStamp, world_id: &str) -> Vec<Value> {
    let tokens = doomed
        .tokens
        .iter()
        .map(|id| json!({ "type": "remove_token", "tokenId": id }));
    let walls = doomed
        .walls
        .iter()
        .map(|id| json!({ "type": "delete_wall", "wallId": id, "worldId": world_id }));
    let lights = doomed
        .lights
        .iter()
        .map(|id| json!({ "type": "delete_light", "lightId": id, "worldId": world_id }));
    let shapes = doomed
        .shapes
        .iter()
        .map(|id| json!({ "type": "delete_shape", "shapeId": id, "worldId": world_id }));
    tokens
        .chain(walls)
        .chain(lights)
        .chain(shapes)
        .map(|mut event| {
            event["group"] = stamp.to_json();
            event
        })
        .collect()
}

/// Delete or Backspace with a group of two or more, or a requested delete.
#[allow(clippy::too_many_arguments)]
pub(crate) fn delete_group_selection(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut selection: SelectionParams,
    mut walls: Option<ResMut<WallSet>>,
    mut lights: Option<ResMut<LightSet>>,
    mut shapes: Option<ResMut<ShapeSet>>,
    is_gm: Res<IsGameMaster>,
    viewer: Option<Res<ViewerUserId>>,
    mut stamps: ResMut<GroupStamps>,
    active_world: Res<ActiveWorld>,
) {
    let requested = DELETE_REQUESTED.swap(false, Ordering::AcqRel);
    let pressed = keys.as_deref().is_some_and(|keys| {
        keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace)
    });
    let group = selection.group.clone();
    if !(requested && !group.is_empty() || pressed && group.len() >= 2) {
        return;
    }
    let doomed = deletable(
        &group,
        is_gm.0,
        viewer.as_ref().and_then(|v| v.0.as_deref()),
        |id| shapes.as_ref().and_then(|s| s.get(id).cloned()),
        |id| {
            lights
                .as_ref()
                .and_then(|l| l.get(id))
                .is_some_and(|l| l.is_carried())
        },
    );
    if doomed.is_empty() {
        return;
    }

    // Gone from this board at once, as each tool's own Delete does; a token
    // goes when the server's answer comes back.
    if let Some(walls) = walls.as_mut() {
        for id in &doomed.walls {
            walls.remove(id);
        }
    }
    if let Some(lights) = lights.as_mut() {
        for id in &doomed.lights {
            lights.remove(id);
        }
    }
    if let Some(shapes) = shapes.as_mut() {
        for id in &doomed.shapes {
            shapes.remove(id);
        }
    }
    let mut left = group.clone();
    for id in doomed
        .tokens
        .iter()
        .chain(&doomed.walls)
        .chain(&doomed.lights)
        .chain(&doomed.shapes)
    {
        left.remove(id);
    }
    selection.set(left);
    emit_group(&selection.group);

    let stamp = stamps.next(doomed.len());
    for event in delete_events(&doomed, &stamp, &active_world.0) {
        emit_event(event);
    }
}

#[cfg(test)]
#[path = "group_move_tests.rs"]
mod tests;
