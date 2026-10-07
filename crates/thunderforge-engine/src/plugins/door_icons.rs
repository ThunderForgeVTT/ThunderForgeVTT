//! A door shows what it can do (spec 071 User Story 3).
//!
//! An icon at the door's midpoint: open when the door is shut, close when it
//! is open, a padlock when it is locked, and nothing on a secret door. It shows
//! while the pointer is within ten screen pixels of the door, or while a token
//! the viewer controls is within one grid square of it. Which icon, and when,
//! is `thunderforge_canvas_core::door_icon`, tested natively there; this file
//! draws it and reports a press on it.
//!
//! A press is reported, not acted on (constitution I): the engine emits
//! `door_icon_pressed` and the web app does what the right-click menu's
//! matching item does, through the same rule and the same mutations. What a
//! press *means* differs by role, and the role's rules are the web app's.
//!
//! Cheap by construction: the icons wanted are worked out each frame from a
//! handful of doors, but an entity is spawned or despawned only when that
//! answer changes — the pointer resting near a door costs no allocation.
//!
//! Removing this plugin's line from `startup.rs` removes the icons and
//! changes nothing else (constitution II).

use std::collections::HashMap;

use bevy::input::InputSystems;
use bevy::prelude::*;
use serde_json::json;
use thunderforge_canvas_core::door_icon::{
    DoorIcon, ICON_MIN_SCREEN_PX, ICON_POINTER_REACH_PX, IconPlace, NearbyToken, Proximity,
    door_icon, icon_shown,
};
use thunderforge_canvas_core::grid::Footprint;

use crate::emit_event;
use crate::plugins::authoring_mode::AuthoringMode;
use crate::plugins::touch::Pointer;
use crate::resources::{
    CameraManager, IsGameMaster, SceneGrid, SelectedToken, TokenGridBehaviour, WallSet,
};
use crate::systems::token_move::ControlledToken;
use crate::{TOKEN_SIZE, TokenEntities};

/// Above tokens, darkness and the interaction markers (400): an icon under
/// the dark would be an icon nobody can press.
const ICON_Z: f32 = 450.0;
/// Each part a little above the last, so they never fight for a pixel.
const PART_Z_STEP: f32 = 0.1;

/// The square of grid assumed when a scene has none.
const DEFAULT_SQUARE: f32 = 100.0;

const PLATE_EDGE: Color = Color::srgb(0.93, 0.89, 0.80);
const PLATE: Color = Color::srgb(0.16, 0.14, 0.12);
const WOOD: Color = Color::srgb(0.78, 0.56, 0.34);
const OPENING: Color = Color::srgb(0.05, 0.05, 0.06);
const BRASS: Color = Color::srgb(0.95, 0.78, 0.30);

/// One drawn icon.
#[derive(Component)]
struct DoorIconMarker;

/// The icons on the board now, by wall id.
#[derive(Resource, Default)]
struct ShownDoorIcons(HashMap<String, Shown>);

struct Shown {
    icon: DoorIcon,
    place: IconPlace,
    entity: Entity,
}

/// One rectangle of an icon, as a share of the icon's side.
struct Part {
    offset: Vec2,
    size: Vec2,
    color: Color,
    turn: f32,
}

const fn part(x: f32, y: f32, w: f32, h: f32, color: Color) -> Part {
    Part {
        offset: Vec2::new(x, y),
        size: Vec2::new(w, h),
        color,
        turn: 0.0,
    }
}

/// What an icon is drawn from, back to front, in shares of its side.
fn icon_parts(icon: DoorIcon) -> Vec<Part> {
    let mut parts = vec![
        part(0.0, 0.0, 1.0, 1.0, PLATE_EDGE),
        part(0.0, 0.0, 0.86, 0.86, PLATE),
    ];
    match icon {
        // Shut, so the icon is a doorway with its door swung open: press to
        // open.
        DoorIcon::Open => parts.extend([
            part(0.0, 0.0, 0.46, 0.64, WOOD),
            part(0.0, -0.02, 0.34, 0.56, OPENING),
            Part {
                turn: -0.45,
                ..part(-0.17, -0.02, 0.10, 0.56, WOOD)
            },
        ]),
        // Open, so the icon is a shut door with its handle: press to shut.
        DoorIcon::Close => parts.extend([
            part(0.0, 0.0, 0.42, 0.62, WOOD),
            part(0.0, 0.0, 0.30, 0.04, PLATE),
            part(0.12, -0.04, 0.07, 0.07, BRASS),
        ]),
        DoorIcon::Padlock => parts.extend([
            // The shackle: two posts and a bar.
            part(-0.13, 0.14, 0.07, 0.22, BRASS),
            part(0.13, 0.14, 0.07, 0.22, BRASS),
            part(0.0, 0.25, 0.33, 0.07, BRASS),
            // The body and its keyhole.
            part(0.0, -0.10, 0.44, 0.34, BRASS),
            part(0.0, -0.08, 0.06, 0.12, PLATE),
        ]),
    }
    parts
}

fn spawn_icon(commands: &mut Commands, icon: DoorIcon, place: IconPlace) -> Entity {
    commands
        .spawn((
            DoorIconMarker,
            Transform::from_translation(place.center.extend(ICON_Z)),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, part) in icon_parts(icon).into_iter().enumerate() {
                parent.spawn((
                    Sprite::from_color(part.color, part.size * place.side),
                    Transform::from_translation(
                        (part.offset * place.side).extend(index as f32 * PART_Z_STEP),
                    )
                    .with_rotation(Quat::from_rotation_z(part.turn)),
                ));
            }
        })
        .id()
}

/// The pointer on the board, in world units.
fn pointer_on_board(
    pointer: &Pointer,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<Vec2> {
    let (camera, transform) = cameras.iter().next()?;
    camera
        .viewport_to_world_2d(transform, pointer.position()?)
        .ok()
}

/// Icons are a play affordance: while a Game Master draws walls or places
/// lights, a press on a door is about the wall tool, not the door.
fn playing(mode: Option<&State<AuthoringMode>>) -> bool {
    mode.is_none_or(|mode| *mode.get() == AuthoringMode::Select)
}

/// The tokens this viewer controls: the one the application named as theirs,
/// and, for a Game Master, whatever they have selected.
fn controlled_tokens(
    controlled: Option<&ControlledToken>,
    selected: Option<&SelectedToken>,
    game_master: bool,
) -> Vec<String> {
    let mut ids: Vec<String> = controlled.and_then(|c| c.0.clone()).into_iter().collect();
    if game_master && let Some(selected) = selected {
        for id in selected.selected_ids() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

#[allow(clippy::too_many_arguments)]
fn sync_door_icons(
    mut commands: Commands,
    mut shown: ResMut<ShownDoorIcons>,
    walls: Option<Res<WallSet>>,
    mode: Option<Res<State<AuthoringMode>>>,
    pointer: Pointer,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    camera_mgr: Option<Res<CameraManager>>,
    grid: Option<Res<SceneGrid>>,
    is_gm: Option<Res<IsGameMaster>>,
    controlled: Option<Res<ControlledToken>>,
    selected: Option<Res<SelectedToken>>,
    token_entities: Option<Res<TokenEntities>>,
    tokens: Query<(&Transform, Option<&TokenGridBehaviour>), Without<DoorIconMarker>>,
) {
    let square = grid.as_ref().map_or(DEFAULT_SQUARE, |grid| grid.size);
    let mut wanted: HashMap<String, (DoorIcon, IconPlace)> = HashMap::new();

    if let Some(walls) = walls.as_ref()
        && playing(mode.as_deref())
    {
        let game_master = is_gm.is_some_and(|gm| gm.0);
        let nearby: Vec<NearbyToken> =
            controlled_tokens(controlled.as_deref(), selected.as_deref(), game_master)
                .iter()
                .filter_map(|id| token_entities.as_ref()?.0.get(id).copied())
                .filter_map(|entity| tokens.get(entity).ok())
                .map(|(transform, behaviour)| NearbyToken {
                    center: transform.translation.truncate(),
                    side: grid.as_ref().map_or(TOKEN_SIZE.y, |grid| {
                        behaviour
                            .map_or_else(Footprint::default, |b| b.footprint)
                            .world_size(grid.size)
                    }),
                })
                .collect();
        // World units per screen pixel at this zoom.
        let per_px = camera_mgr.as_ref().map_or(1.0, |camera| camera.scale);
        let near = Proximity {
            pointer: pointer_on_board(&pointer, &cameras),
            pointer_reach: ICON_POINTER_REACH_PX * per_px,
            tokens: &nearby,
            square,
        };
        for wall in walls.walls() {
            let Some(icon) = door_icon(wall) else {
                continue;
            };
            let place = IconPlace::for_wall(wall, square, ICON_MIN_SCREEN_PX * per_px);
            if icon_shown(wall, &place, &near) {
                wanted.insert(wall.id.clone(), (icon, place));
            }
        }
    }

    let before = shown.0.len();
    let mut changed = false;
    shown.0.retain(|wall_id, held| {
        let keep = wanted
            .get(wall_id)
            .is_some_and(|(icon, place)| *icon == held.icon && *place == held.place);
        if !keep {
            commands.entity(held.entity).despawn();
        }
        keep
    });
    changed |= shown.0.len() != before;
    for (wall_id, (icon, place)) in wanted {
        if shown.0.contains_key(&wall_id) {
            continue;
        }
        let entity = spawn_icon(&mut commands, icon, place);
        shown.0.insert(
            wall_id,
            Shown {
                icon,
                place,
                entity,
            },
        );
        changed = true;
    }
    if changed {
        mirror_shown(&shown);
    }
}

/// A left press on a shown icon is the icon's, and nothing else's.
///
/// In `PreUpdate`, after the frame's input and the fingers have been read,
/// so the press can be claimed before `Update`: a token under the door would
/// otherwise be picked up, or the selection dropped as a click on empty board.
/// Reads the icons shown last frame, which are the ones on the screen.
fn press_door_icon(
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    shown: Res<ShownDoorIcons>,
    mode: Option<Res<State<AuthoringMode>>>,
    pointer: Pointer,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    if shown.0.is_empty() || !mouse.just_pressed(MouseButton::Left) || !playing(mode.as_deref()) {
        return;
    }
    let Some(at) = pointer_on_board(&pointer, &cameras) else {
        return;
    };
    let Some((wall_id, held)) = shown.0.iter().find(|(_, held)| held.place.contains(at)) else {
        return;
    };
    mouse.clear_just_pressed(MouseButton::Left);
    emit_event(json!({
        "type": "door_icon_pressed",
        "wallId": wall_id,
        "icon": held.icon.as_id(),
    }));
}

static SHOWN_DOOR_ICONS: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();

fn mirror_shown(shown: &ShownDoorIcons) {
    let mut rows: Vec<_> = shown
        .0
        .iter()
        .map(|(wall_id, held)| {
            json!({
                "wallId": wall_id,
                "icon": held.icon.as_id(),
                "x": held.place.center.x,
                "y": held.place.center.y,
                "side": held.place.side,
            })
        })
        .collect();
    rows.sort_by(|a, b| a["wallId"].as_str().cmp(&b["wallId"].as_str()));
    let text = serde_json::Value::Array(rows).to_string();
    let slot = SHOWN_DOOR_ICONS.get_or_init(|| std::sync::Mutex::new(String::from("[]")));
    if let Ok(mut held) = slot.lock() {
        *held = text;
    }
}

/// The door icons on this board, as JSON: `[{ wallId, icon, x, y, side }]`,
/// `icon` one of `open`, `close`, `padlock`, and the rest in world units.
///
/// For the engine probe: which icon a door shows, and when, is the claim
/// spec 071's checks make, and a sprite cannot be read back from a canvas.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn door_icons() -> String {
    SHOWN_DOOR_ICONS
        .get_or_init(|| std::sync::Mutex::new(String::from("[]")))
        .lock()
        .map(|held| held.clone())
        .unwrap_or_else(|_| String::from("[]"))
}

pub struct DoorIconsPlugin;

impl Plugin for DoorIconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShownDoorIcons>()
            .add_systems(
                PreUpdate,
                press_door_icon
                    .after(InputSystems)
                    .after(crate::plugins::touch::translate_touches),
            )
            .add_systems(Update, sync_door_icons);
    }
}
