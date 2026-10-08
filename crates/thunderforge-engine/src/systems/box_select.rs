//! The box a user drags over the board in Select (spec 085).
//!
//! A press on empty board, dragged past the click threshold, draws a box. On
//! release the box takes every item wholly inside it that the viewer may take
//! ([`box_candidate`]), folded into the group by `apply_box` — with shift
//! held, the box toggles what it holds instead of replacing the group. A press
//! that never travels is a click on empty board, and clears the group.
//!
//! The geometry is canvas-core's (`box_select`); this module only measures
//! what the ECS holds into `Footprint`s and writes the result.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde_json::json;

use thunderforge_canvas_core::box_select::{Footprint, ScreenBox, apply_box, wholly_inside};
use thunderforge_canvas_core::camera::DRAG_THRESHOLD_PX;
use thunderforge_canvas_core::door_icon::distance_to_segment;
use thunderforge_canvas_core::shape_geometry::shape_bounds;
use thunderforge_canvas_core::token_stack::{StackCandidate, tokens_at};

use crate::plugins::selection_filter::SelectionFilter;
use crate::resources::{
    GroupKind, GroupSelection, IsGameMaster, LightSet, SceneGrid, SelectedLight, SelectedShape,
    SelectedToken, SelectedWall, Shape, ShapeSet, Singles, TokenGridBehaviour, ViewerUserId,
    WallSet,
};
use crate::systems::shape_authority::may_edit_shape;
use crate::systems::token::TokenOwner;
use crate::{TOKEN_SIZE, TokenIdentity, emit_event};

/// How close a press must come to a wall to land on it, as the Walls tool's
/// body select measures it.
const WALL_GRAB_DISTANCE: f32 = 6.0;
/// How close a press must come to a shape's bounds to land on it.
const SHAPE_GRAB_DISTANCE: f32 = 8.0;
/// The outline drawn while the box is dragged.
const BOX_COLOR: Color = Color::srgba(0.35, 0.75, 0.95, 0.9);

/// One item the box might take, as the candidate rule needs to see it.
pub enum BoxItem<'a> {
    Token { owner: Option<&'a str>, seen: bool },
    Wall,
    Light,
    Shape(&'a Shape),
}

/// Whether the viewer's box may take `item`.
///
/// A Game Master takes every kind the Select filter allows. A player takes
/// only their own tokens they can see, and the shapes they may edit; walls
/// and lights are never theirs, and the filter does not apply to them. A
/// player the board was never told the name of takes nothing.
pub fn box_candidate(
    is_gm: bool,
    viewer: Option<&str>,
    filter: &SelectionFilter,
    item: BoxItem<'_>,
) -> bool {
    if is_gm {
        return match item {
            BoxItem::Token { .. } => filter.tokens,
            BoxItem::Wall => filter.walls,
            BoxItem::Light => filter.lights,
            BoxItem::Shape(_) => filter.shapes,
        };
    }
    let Some(viewer) = viewer else {
        return false;
    };
    match item {
        BoxItem::Token { owner, seen } => seen && owner == Some(viewer),
        BoxItem::Wall | BoxItem::Light => false,
        BoxItem::Shape(shape) => may_edit_shape(false, Some(viewer), shape),
    }
}

/// The box in progress: where the press landed, in world and screen units.
#[derive(Resource, Debug, Default)]
pub struct BoxDrag {
    anchor: Option<(Vec2, Vec2)>,
    /// Past the click threshold: this gesture is a box, not a click.
    boxing: bool,
}

impl BoxDrag {
    /// The box being drawn, or `None` while there is none to draw.
    pub fn drawn(&self, cursor: Vec2) -> Option<ScreenBox> {
        let (anchor, _) = self.anchor?;
        self.boxing.then(|| ScreenBox::from_corners(anchor, cursor))
    }
}

/// The four single selections, borrowed together so a group can write
/// through to them.
#[derive(SystemParam)]
pub(crate) struct SelectionParams<'w> {
    pub group: ResMut<'w, GroupSelection>,
    token: ResMut<'w, SelectedToken>,
    wall: ResMut<'w, SelectedWall>,
    light: ResMut<'w, SelectedLight>,
    shape: ResMut<'w, SelectedShape>,
}

impl SelectionParams<'_> {
    pub(crate) fn set(&mut self, next: GroupSelection) {
        let singles = Singles {
            token: &mut self.token,
            wall: &mut self.wall,
            light: &mut self.light,
            shape: &mut self.shape,
        };
        self.group.set_group(next, singles);
    }

    /// Whether the singles still say what the group wrote to them.
    pub(crate) fn agrees(&mut self) -> bool {
        let singles = Singles {
            token: &mut self.token,
            wall: &mut self.wall,
            light: &mut self.light,
            shape: &mut self.shape,
        };
        self.group.agrees_with(&singles)
    }

    /// The group the singles describe, when a tool selected one thing on
    /// its own.
    pub(crate) fn singles_group(&mut self) -> GroupSelection {
        let singles = Singles {
            token: &mut self.token,
            wall: &mut self.wall,
            light: &mut self.light,
            shape: &mut self.shape,
        };
        singles.as_group()
    }
}

/// Everything on the board the box measures, read-only.
#[derive(SystemParam)]
pub(crate) struct BoardItems<'w, 's> {
    pub(crate) tokens: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static TokenIdentity,
            Option<&'static TokenGridBehaviour>,
            Option<&'static TokenOwner>,
            Option<&'static Visibility>,
        ),
    >,
    pub(crate) walls: Option<Res<'w, WallSet>>,
    pub(crate) lights: Option<Res<'w, LightSet>>,
    pub(crate) shapes: Option<Res<'w, ShapeSet>>,
    pub(crate) grid: Option<Res<'w, SceneGrid>>,
}

impl BoardItems<'_, '_> {
    pub(crate) fn token_side(&self, behaviour: Option<&TokenGridBehaviour>) -> f32 {
        let footprint = behaviour.map_or_else(Default::default, |b| b.footprint);
        self.grid
            .as_ref()
            .map(|grid| footprint.world_size(grid.size))
            .unwrap_or(TOKEN_SIZE.y)
    }

    /// The tokens under `at`, topmost first, as the token drag measures them.
    pub(crate) fn tokens_at(&self, at: Vec2) -> Vec<String> {
        let candidates: Vec<StackCandidate> = self
            .tokens
            .iter()
            .map(|(transform, identity, behaviour, _, _)| StackCandidate {
                id: identity.0.clone(),
                center: transform.translation.truncate(),
                footprint_side: self.token_side(behaviour),
                z: transform.translation.z,
            })
            .collect();
        tokens_at(&candidates, at)
    }

    /// The member of `group` under `at`, if any: a token first, then a
    /// wall, a light, a shape.
    pub(crate) fn member_at(
        &self,
        group: &GroupSelection,
        at: Vec2,
    ) -> Option<(GroupKind, String)> {
        if let Some(id) = self
            .tokens_at(at)
            .into_iter()
            .find(|id| group.tokens.contains(id))
        {
            return Some((GroupKind::Token, id));
        }
        if let Some(walls) = &self.walls {
            for wall in walls.walls() {
                if group.walls.contains(&wall.id)
                    && distance_to_segment(at, wall.start(), wall.end()) <= WALL_GRAB_DISTANCE
                {
                    return Some((GroupKind::Wall, wall.id.clone()));
                }
            }
        }
        if let Some(lights) = &self.lights {
            for light in lights.lights() {
                if group.lights.contains(&light.id)
                    && at.distance(light.position()) <= crate::systems::lighting::LIGHT_GRAB_RADIUS
                {
                    return Some((GroupKind::Light, light.id.clone()));
                }
            }
        }
        if let Some(shapes) = &self.shapes {
            for shape in shapes.shapes() {
                if !group.shapes.contains(&shape.id) {
                    continue;
                }
                if let Some((min, max)) = shape_bounds(shape.kind, &shape.geometry) {
                    let grow = Vec2::splat(SHAPE_GRAB_DISTANCE);
                    if at.cmpge(min - grow).all() && at.cmple(max + grow).all() {
                        return Some((GroupKind::Shape, shape.id.clone()));
                    }
                }
            }
        }
        None
    }

    /// What `area` takes for this viewer, per kind, in board order.
    fn hits(
        &self,
        area: &ScreenBox,
        is_gm: bool,
        viewer: Option<&str>,
        filter: &SelectionFilter,
    ) -> GroupSelection {
        let mut hits = GroupSelection::default();
        for (transform, identity, behaviour, owner, visibility) in self.tokens.iter() {
            let half = Vec2::splat(self.token_side(behaviour) / 2.0);
            let center = transform.translation.truncate();
            let item = BoxItem::Token {
                owner: owner.and_then(|o| o.0.as_deref()),
                seen: visibility != Some(&Visibility::Hidden),
            };
            if box_candidate(is_gm, viewer, filter, item)
                && wholly_inside(
                    area,
                    &Footprint::Rect {
                        min: center - half,
                        max: center + half,
                    },
                )
            {
                hits.tokens.push(identity.0.clone());
            }
        }
        if let Some(walls) = &self.walls {
            for wall in walls.walls() {
                if box_candidate(is_gm, viewer, filter, BoxItem::Wall)
                    && wholly_inside(area, &Footprint::Segment(wall.start(), wall.end()))
                {
                    hits.walls.push(wall.id.clone());
                }
            }
        }
        if let Some(lights) = &self.lights {
            for light in lights.lights() {
                // A carried light is its character's, and moves with it.
                if !light.is_carried()
                    && box_candidate(is_gm, viewer, filter, BoxItem::Light)
                    && wholly_inside(area, &Footprint::Point(light.position()))
                {
                    hits.lights.push(light.id.clone());
                }
            }
        }
        if let Some(shapes) = &self.shapes {
            for shape in shapes.shapes() {
                let Some((min, max)) = shape_bounds(shape.kind, &shape.geometry) else {
                    continue;
                };
                if box_candidate(is_gm, viewer, filter, BoxItem::Shape(shape))
                    && wholly_inside(area, &Footprint::Rect { min, max })
                {
                    hits.shapes.push(shape.id.clone());
                }
            }
        }
        hits
    }
}

/// Whether either shift key is held.
pub(crate) fn shift_held(keys: Option<&ButtonInput<KeyCode>>) -> bool {
    keys.is_some_and(|keys| keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
}

/// Tell the web what the group now holds.
pub(crate) fn emit_group(group: &GroupSelection) {
    emit_event(json!({
        "type": "select_group",
        "tokenIds": group.tokens,
        "wallIds": group.walls,
        "lightIds": group.lights,
        "shapeIds": group.shapes,
    }));
}

/// What a left press in Select starts, as far as the group is concerned.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Press {
    /// Empty board: a box, or a click that clears.
    Box,
    /// Shift on an item: it joins or leaves the group.
    Toggle(GroupKind, String),
    /// A member of the group: the group move's, or the token drag's.
    Member,
    /// A token outside the group: the token drag's, from an empty group.
    Other,
}

/// Decide a press from what lies under it: the group member there, if any,
/// and the topmost token.
pub(crate) fn press_outcome(
    shift: bool,
    member: Option<(GroupKind, String)>,
    top_token: Option<String>,
) -> Press {
    match (shift, member, top_token) {
        (true, Some((kind, id)), _) => Press::Toggle(kind, id),
        (true, None, Some(id)) => Press::Toggle(GroupKind::Token, id),
        (false, Some(_), _) => Press::Member,
        (false, None, Some(_)) => Press::Other,
        (_, None, None) => Press::Box,
    }
}

/// The group with one item toggled in or out.
pub(crate) fn toggle_one(group: &GroupSelection, kind: GroupKind, id: String) -> GroupSelection {
    let mut next = group.clone();
    let toggled = apply_box(group.list(kind), &[id], true);
    match kind {
        GroupKind::Token => next.tokens = toggled,
        GroupKind::Wall => next.walls = toggled,
        GroupKind::Light => next.lights = toggled,
        GroupKind::Shape => next.shapes = toggled,
    }
    next
}

/// `apply_box` for every kind at once.
fn fold_box(current: &GroupSelection, hits: &GroupSelection, toggle: bool) -> GroupSelection {
    GroupSelection {
        tokens: apply_box(&current.tokens, &hits.tokens, toggle),
        walls: apply_box(&current.walls, &hits.walls, toggle),
        lights: apply_box(&current.lights, &hits.lights, toggle),
        shapes: apply_box(&current.shapes, &hits.shapes, toggle),
    }
}

/// The box gesture, in Select only.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_box_select(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    board: BoardItems,
    mut selection: SelectionParams,
    mut drag: ResMut<BoxDrag>,
    is_gm: Res<IsGameMaster>,
    viewer: Option<Res<ViewerUserId>>,
    filter: Option<Res<SelectionFilter>>,
) {
    let Some(cursor_px) = pointer.position() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };
    let Ok(cursor) = camera.viewport_to_world_2d(camera_transform, cursor_px) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        let toggle = shift_held(keys.as_deref());
        let member = board.member_at(&selection.group, cursor);
        let under = board.tokens_at(cursor);
        *drag = BoxDrag::default();
        match press_outcome(toggle, member, under.first().cloned()) {
            Press::Box => drag.anchor = Some((cursor, cursor_px)),
            Press::Toggle(kind, id) => {
                let next = toggle_one(&selection.group, kind, id);
                selection.set(next);
                emit_group(&selection.group);
            }
            Press::Member => {}
            Press::Other => {
                // A plain press on a token outside the group starts over:
                // the token drag then selects what it picked up.
                if !selection.group.is_empty() {
                    selection.set(GroupSelection::default());
                    emit_group(&selection.group);
                }
            }
        }
        return;
    }

    let Some((anchor, anchor_px)) = drag.anchor else {
        return;
    };

    if mouse_button.pressed(MouseButton::Left) {
        if cursor_px.distance(anchor_px) > DRAG_THRESHOLD_PX {
            drag.boxing = true;
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        let boxing = std::mem::take(&mut *drag).boxing;
        let toggle = shift_held(keys.as_deref());
        let next = if boxing {
            let area = ScreenBox::from_corners(anchor, cursor);
            let hits = board.hits(
                &area,
                is_gm.0,
                viewer.as_ref().and_then(|v| v.0.as_deref()),
                &filter.as_deref().copied().unwrap_or_default(),
            );
            fold_box(&selection.group, &hits, toggle)
        } else if toggle {
            // A shift-click on empty board keeps what is selected.
            return;
        } else {
            GroupSelection::default()
        };
        selection.set(next);
        emit_group(&selection.group);
    }
}

/// When a tool selects one thing on its own — a token clicked, a wall
/// picked in Walls — the group follows the singles.
pub(crate) fn follow_single_selection(mut selection: SelectionParams) {
    if !selection.agrees() {
        let group = selection.singles_group();
        *selection.group = group;
    }
}

/// The box outline, while it is dragged.
pub(crate) fn draw_box(
    pointer: crate::plugins::touch::Pointer,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    drag: Res<BoxDrag>,
    mut gizmos: Gizmos,
) {
    let Some(cursor_px) = pointer.position() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };
    let Ok(cursor) = camera.viewport_to_world_2d(camera_transform, cursor_px) else {
        return;
    };
    if let Some(area) = drag.drawn(cursor) {
        let corners = [
            area.min,
            Vec2::new(area.max.x, area.min.y),
            area.max,
            Vec2::new(area.min.x, area.max.y),
        ];
        for i in 0..4 {
            gizmos.line_2d(corners[i], corners[(i + 1) % 4], BOX_COLOR);
        }
    }
}

#[cfg(test)]
#[path = "box_select_tests.rs"]
mod tests;
