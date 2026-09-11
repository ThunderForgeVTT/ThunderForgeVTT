//! Light authoring input, rendering sync, undo, and occlusion-aware
//! illumination systems (T037-T039, T041 of
//! specs/001-bevy-canvas-authoring/tasks.md).
//!
//! Wiring: see `plugins/lighting.rs`'s `LightingPlugin`.

use std::collections::HashMap;

use bevy::input::mouse::MouseWheel;

use crate::plugins::camera::read_wheel_notches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::{Value, json};
use thunderforge_canvas_core::snapping::SnapRule;

use crate::resources::{
    CanvasLayer, IsGameMaster, LightEdit, LightSet, LightSource, SceneAmbient, SelectedLight,
    TokenVision, WallSet,
};
use crate::{ActiveWorld, TokenIdentity, emit_event};
use thunderforge_canvas_core::vision::{
    AmbientLight, Illumination, ResolvedLight, Rgb, Visibility as Perceived, VisionProfile,
    illumination_at, visibility_of,
};

/// Default radius (px) for a newly click-placed light (T037).
const DEFAULT_LIGHT_RADIUS: f32 = 100.0;

/// Default intensity for a newly click-placed light (T037).
const DEFAULT_LIGHT_INTENSITY: f32 = 1.0;

/// How close (px) the cursor must be to an existing light's center to grab
/// it for select/drag, rather than placing a new light. Deliberately a
/// fixed value (not scaled by the light's own radius) so tiny and huge
/// lights are equally easy to grab, mirroring `wall.rs`'s
/// `ENDPOINT_GRAB_RADIUS` fixed-pixel approach.
const LIGHT_GRAB_RADIUS: f32 = 15.0;

/// Radius (px) change per scroll-wheel notch (T037's resize control).
const RESIZE_STEP: f32 = 10.0;

/// Never let a resize drive radius to/below zero (T041's zero-radius
/// rejection applies to resize too, not just creation).
const MIN_LIGHT_RADIUS: f32 = 1.0;

const DEFAULT_LIGHT_COLOR: Color = Color::srgb(1.0, 0.85, 0.55);
const SELECTED_LIGHT_COLOR: Color = Color::srgb(0.95, 0.85, 0.25);
const NON_SHADOW_CASTING_TINT: Color = Color::srgb(0.6, 0.85, 1.0);
/// Side length of a light's on-canvas marker, in world units.
///
/// Independent of the light's radius on purpose — see the note where it is
/// used. Sized to stay grabbable at play zoom without covering map detail.
const LIGHT_MARKER_SIZE: f32 = 18.0;

const HANDLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const HANDLE_SIZE: Vec2 = Vec2::new(6.0, 6.0);

/// The `Visibility` value applied to an unlit token by
/// `apply_light_illumination`'s first-pass toggle (see that system's doc
/// comment for the fuller rationale/limitation note).
const UNLIT_VISIBILITY: Visibility = Visibility::Hidden;

/// Alpha applied to a token perceived only dimly.
///
/// Dim light is the whole reason this is a three-state model rather than a
/// toggle: a figure at the edge of torchlight should be a suggestion, not a
/// pop-in. Low enough to read as indistinct, high enough to still find.
const DIM_ALPHA: f32 = 0.45;

/// Marker on the sprite entity rendered for a given `LightSet` light id.
#[derive(Component)]
pub(crate) struct LightVisual;

/// Marker on a GM-only selection-handle sprite for the selected light.
#[derive(Component)]
pub(crate) struct LightHandle;

/// Maps `LightSet` light ids to their spawned sprite entity, mirroring
/// `WallEntities`/`TokenEntities`.
#[derive(Resource, Default)]
pub(crate) struct LightEntities(HashMap<String, Entity>);

#[derive(Default)]
enum LightDragMode {
    #[default]
    Idle,
    /// Dragging an existing light to reposition it. `prior_x`/`prior_y`
    /// are the light's position at drag-start, captured for the undo
    /// stack.
    Moving {
        light_id: String,
        prior_x: f32,
        prior_y: f32,
    },
}

/// Session-local light-tool drag state (not persisted, not part of
/// `LightSet`), mirroring `WallDragState`.
#[derive(Resource, Default)]
pub(crate) struct LightDragState {
    mode: LightDragMode,
}

impl LightDragState {
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

/// Convert the cursor's window-pixel position into Bevy world space,
/// duplicated from `systems/wall.rs`/`systems/selection.rs`'s private
/// helper of the same name/shape (not exported from those modules).
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

/// Resolves a light's effective render/occlusion position: for a
/// token-attached light, the attached token's live position (data-model.md:
/// "x/y are ignored by the engine in favor of the token's live position"),
/// resolved each frame from a position map built off `TokenIdentity`
/// (mirrors how `handle_token_drag`/`TokenIdentity` are queried in
/// `systems/selection.rs`); otherwise the light's own stored position.
/// Falls back to the stored position if the attached token isn't found
/// (e.g. it was removed).
pub(crate) fn effective_light_position(
    light: &LightSource,
    token_positions: &HashMap<String, Vec2>,
) -> Vec2 {
    if let Some(token_id) = &light.attached_token_id
        && let Some(position) = token_positions.get(token_id)
    {
        return *position;
    }
    light.position()
}

fn light_color(light: &LightSource, selected: bool) -> Color {
    if selected {
        return SELECTED_LIGHT_COLOR;
    }
    if !light.casts_shadows {
        return NON_SHADOW_CASTING_TINT;
    }
    light
        .color
        .as_deref()
        .and_then(|hex| Srgba::hex(hex.trim_start_matches('#')).ok())
        .map(Color::from)
        .unwrap_or(DEFAULT_LIGHT_COLOR)
}

/// T037: click to place a light at the cursor (default radius/intensity),
/// click an existing light to select it, drag it to reposition. GM-only
/// per `CanvasLayer::Lighting.editing_is_gm_only()` — reuses
/// `resources::wall::IsGameMaster` rather than duplicating a GM-role flag
/// (it isn't wall-specific despite its current module location). T041:
/// creation always uses a positive default radius, so a plain click can
/// never itself produce a zero-radius light; the guard here is defensive
/// in case that default is ever changed to something caller-supplied.
pub(crate) fn handle_light_input(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut light_set: ResMut<LightSet>,
    mut selected_light: ResMut<SelectedLight>,
    mut drag: ResMut<LightDragState>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
    scene_grid: Res<crate::resources::grid::SceneGrid>,
    snap_enabled: Res<crate::resources::token_grid::GridSnapEnabled>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    // FR-024/FR-025: one rule, built from the scene's own grid, used by both
    // the placement path and the move path below. Built once here so the two
    // cannot end up asking different questions.
    let snap_rule = SnapRule::new(scene_grid.0, snap_enabled.0);

    if mouse_button.just_pressed(MouseButton::Left) {
        for light in light_set.lights() {
            if cursor.distance(light.position()) <= LIGHT_GRAB_RADIUS {
                selected_light.select(light.id.clone());
                drag.mode = LightDragMode::Moving {
                    light_id: light.id.clone(),
                    prior_x: light.x,
                    prior_y: light.y,
                };
                return;
            }
        }

        // No existing light hit: place a new one, GM-only, per T037.
        selected_light.deselect();
        drag.mode = LightDragMode::Idle;

        if DEFAULT_LIGHT_RADIUS <= 0.0 {
            // T041: reject/ignore zero-(or negative-)radius creation.
            return;
        }

        // Snapped to a cell centre, the same rule everything else on the
        // canvas obeys (spec 031 FR-024/FR-025). A light is a point, so the
        // centre is the right lattice for it — walls use vertices instead,
        // because a wall runs *between* cells rather than through one.
        let placed = snap_rule.cell(cursor);

        emit_event(json!({
            "type": "create_light",
            "light": {
                "x": placed.x,
                "y": placed.y,
                "radius": DEFAULT_LIGHT_RADIUS,
                "intensity": DEFAULT_LIGHT_INTENSITY,
                "color": Value::Null,
                "attachedTokenId": Value::Null,
                "castsShadows": true,
            },
            "worldId": active_world.0,
        }));
        // Deliberately no local LightSet entry yet: the server assigns the
        // light's real id, so this stays untracked until the matching
        // `upsert_light` command arrives, same rationale as `WallPlugin`'s
        // create flow.
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if let LightDragMode::Moving { light_id, .. } = &drag.mode
            && let Some(light) = light_set.get(light_id).cloned()
        {
            // Snapped while dragging, not only when placed. A light moved to
            // a raw cursor position sits off-lattice, so the same lamp lands
            // in a different place depending on whether it was placed there or
            // dragged there — which is the inconsistency FR-025 is about.
            let moved = snap_rule.cell(cursor);
            let mut updated = light;
            updated.x = moved.x;
            updated.y = moved.y;
            // Optimistic local move so the sprite tracks the cursor;
            // reconciled by the next `upsert_light` confirmation from the
            // server.
            light_set.upsert(updated);
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left)
        && let LightDragMode::Moving {
            light_id,
            prior_x,
            prior_y,
        } = std::mem::take(&mut drag.mode)
    {
        if let Some(light) = light_set.get(&light_id) {
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "x": light.x, "y": light.y },
                "worldId": active_world.0,
            }));
        }
        light_set.push_undo(LightEdit::Move {
            light_id,
            prior_x,
            prior_y,
        });
    }
}

/// T037: scroll-wheel resize control for the selected light's radius,
/// GM-only. Clamped at `MIN_LIGHT_RADIUS` so scrolling can never produce a
/// zero-or-negative radius (T041 applies to resize as well as creation).
pub(crate) fn handle_light_resize(
    mut wheel_events: MessageReader<MouseWheel>,
    mut light_set: ResMut<LightSet>,
    selected_light: Res<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        wheel_events.clear();
        return;
    }

    // Notches, not raw deltas — see `read_wheel_notches`. Unnormalised, a
    // single browser wheel notch was 100 units of radius change here for the
    // same reason it was a hundred zoom steps in the camera.
    let scroll = read_wheel_notches(&mut wheel_events);
    if scroll == 0.0 {
        return;
    }

    let Some(light_id) = selected_light.get_selected().cloned() else {
        return;
    };

    let Some(light) = light_set.get(&light_id).cloned() else {
        return;
    };

    let prior_radius = light.radius;
    let prior_intensity = light.intensity;
    let mut updated = light;
    updated.radius = (updated.radius + scroll * RESIZE_STEP).max(MIN_LIGHT_RADIUS);
    light_set.upsert(updated.clone());
    light_set.push_undo(LightEdit::Resize {
        light_id: light_id.clone(),
        prior_radius,
        prior_intensity,
    });
    emit_event(json!({
        "type": "update_light",
        "lightId": light_id,
        "changes": { "radius": updated.radius },
        "worldId": active_world.0,
    }));
}

/// T037: `L` toggles `casts_shadows` on the selected light; Delete/Backspace
/// removes it. GM-only, same gating as `handle_light_input`. Keybinds
/// chosen to avoid `systems/wall.rs`'s `V`/`B`/`O`/Ctrl+Z and
/// `move_player`'s WASD (`S` is taken by player-token movement, so `L` is
/// used instead of the spec's suggested `S`).
pub(crate) fn handle_light_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut light_set: ResMut<LightSet>,
    mut selected_light: ResMut<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(light_id) = selected_light.get_selected().cloned() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(light) = light_set.get(&light_id).cloned() {
            let prior_casts_shadows = light.casts_shadows;
            let mut updated = light;
            updated.casts_shadows = !updated.casts_shadows;
            light_set.upsert(updated.clone());
            light_set.push_undo(LightEdit::FlagsToggle {
                light_id: light_id.clone(),
                prior_casts_shadows,
            });
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "castsShadows": updated.casts_shadows },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if (keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace))
        && let Some(deleted) = light_set.remove(&light_id)
    {
        light_set.push_undo(LightEdit::Delete { deleted });
        selected_light.deselect();
        emit_event(json!({
            "type": "delete_light",
            "lightId": light_id,
            "worldId": active_world.0,
        }));
    }
}

/// T039: light undo (FR-012). Ctrl+Z pops `LightSet`'s undo stack and
/// re-issues the inverse mutation through the same outbound-event path a
/// normal edit uses (research.md §4) — applied locally first (optimistic),
/// then emitted so other clients converge once the server confirms it.
/// Mirrors `systems/wall.rs`'s `handle_wall_undo` exactly.
pub(crate) fn handle_light_undo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut light_set: ResMut<LightSet>,
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

    let Some(edit) = light_set.pop_undo() else {
        return;
    };

    match edit {
        LightEdit::Move {
            light_id,
            prior_x,
            prior_y,
        } => {
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.x = prior_x;
                light.y = prior_y;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "x": prior_x, "y": prior_y },
                "worldId": active_world.0,
            }));
        }
        LightEdit::Resize {
            light_id,
            prior_radius,
            prior_intensity,
        } => {
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.radius = prior_radius;
                light.intensity = prior_intensity;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "radius": prior_radius, "intensity": prior_intensity },
                "worldId": active_world.0,
            }));
        }
        LightEdit::FlagsToggle {
            light_id,
            prior_casts_shadows,
        } => {
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.casts_shadows = prior_casts_shadows;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "castsShadows": prior_casts_shadows },
                "worldId": active_world.0,
            }));
        }
        LightEdit::Delete { deleted } => {
            // Re-creates the light; the server assigns a new id (the
            // original id cannot be resurrected, same caveat as
            // `WallEdit::Delete`'s undo).
            emit_event(json!({
                "type": "create_light",
                "light": {
                    "x": deleted.x,
                    "y": deleted.y,
                    "radius": deleted.radius,
                    "intensity": deleted.intensity,
                    "color": deleted.color,
                    "attachedTokenId": deleted.attached_token_id,
                    "castsShadows": deleted.casts_shadows,
                },
                "worldId": active_world.0,
            }));
        }
    }
}

/// T037: keeps one square sprite (scaled by radius, first-pass indicator —
/// not final art) per `LightSet` light in sync (spawn on new id, update
/// transform/color on change, despawn on removal), rendered at
/// `CanvasLayer::Lighting.z()`. Also renders a GM-only selection handle at
/// the selected light's position, gated the same way `WallPlugin`'s
/// endpoint handles are (`IsGameMaster`).
pub(crate) fn sync_light_visuals(
    mut commands: Commands,
    light_set: Res<LightSet>,
    selected_light: Res<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    mut light_entities: ResMut<LightEntities>,
    token_positions: Query<(&Transform, &TokenIdentity)>,
    mut sprite_query: Query<
        (&mut Transform, &mut Sprite),
        (
            With<LightVisual>,
            Without<LightHandle>,
            Without<TokenIdentity>,
        ),
    >,
    handle_query: Query<Entity, With<LightHandle>>,
) {
    let z = CanvasLayer::Lighting.z();

    let mut positions: HashMap<String, Vec2> = HashMap::new();
    for (transform, identity) in token_positions.iter() {
        positions.insert(identity.0.clone(), transform.translation.truncate());
    }

    let stale_ids: Vec<String> = light_entities
        .0
        .keys()
        .filter(|id| light_set.get(id).is_none())
        .cloned()
        .collect();
    for id in stale_ids {
        if let Some(entity) = light_entities.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }

    for light in light_set.lights() {
        // A light's marker is a grab handle, and handles are the Game
        // Master's (`CanvasLayer::Lighting.editing_is_gm_only`). Drawn for the
        // table, it was a small amber square over every light — the "yellow
        // dot" of two playtests, and a map of where each light is hidden.
        if !is_gm.0 {
            if let Some(entity) = light_entities.0.remove(&light.id) {
                commands.entity(entity).despawn();
            }
            continue;
        }
        let selected = selected_light.is_selected(&light.id);
        let color = light_color(light, selected);
        let position = effective_light_position(light, &positions);
        // A fixed-size marker at the light's position, NOT a quad the size of
        // its radius. It used to be `radius * 2` across, which drew an opaque
        // square over everything the light was supposed to illuminate — a
        // 320-unit torch became a 640-unit block hiding the map beneath it.
        // That went unnoticed for as long as no sprite rendered at all.
        //
        // A light's *extent* is communicated by the overlay rings in
        // `plugins::lighting_overlay`, which is the right way to show a radius:
        // an outline, not a fill. This sprite is only the grab handle.
        let size = Vec2::splat(LIGHT_MARKER_SIZE);
        let translation_z = if selected { z + 1.0 } else { z };
        let transform = Transform::from_translation(position.extend(translation_z));

        if let Some(&entity) = light_entities.0.get(&light.id) {
            if let Ok((mut t, mut sprite)) = sprite_query.get_mut(entity) {
                *t = transform;
                sprite.color = color;
                sprite.custom_size = Some(size);
            }
        } else {
            let entity = commands
                .spawn((Sprite::from_color(color, size), transform, LightVisual))
                .id();
            light_entities.0.insert(light.id.clone(), entity);
        }
    }

    for entity in handle_query.iter() {
        commands.entity(entity).despawn();
    }

    if is_gm.0
        && let Some(selected_id) = selected_light.get_selected()
        && let Some(light) = light_set.get(selected_id)
    {
        let position = effective_light_position(light, &positions);
        commands.spawn((
            Sprite::from_color(HANDLE_COLOR, HANDLE_SIZE),
            Transform::from_translation(position.extend(z + 2.0)),
            LightHandle,
        ));
    }
}

/// Converts a stored light into the form the vision core consumes.
///
/// The stored model has a single `radius`; the illumination model has a bright
/// core and a dim ring. The stored radius is taken as the **outer, dim** edge
/// and bright as half of it — the 1:2 ratio a torch has (20ft bright / 40ft
/// total). Mapping it this way keeps every existing light's outer footprint
/// exactly where it is today while giving it a bright centre, so no scene
/// changes shape when this lands.
fn resolve_light(light: &LightSource, positions: &HashMap<String, Vec2>) -> ResolvedLight {
    ResolvedLight {
        position: effective_light_position(light, positions),
        bright_radius: light.radius * 0.5,
        dim_radius: light.radius,
        color: light
            .color
            .as_deref()
            .and_then(Rgb::parse_hex)
            .unwrap_or(Rgb::WHITE),
        intensity: light.intensity,
        casts_shadows: light.casts_shadows,
    }
}

/// Resolves token visibility: occlusion, facing and illumination, in one pass.
///
/// This replaces two systems that both wrote `Visibility` from different
/// criteria — this one (light coverage) and `wall::apply_vision_occlusion`
/// (player line-of-sight) — where whichever ran later in the schedule won for
/// a given frame. Their own doc comments described that as an accepted
/// limitation of a first pass. It is resolved now: vision is computed once,
/// here, by `vision::visibility_of`, which combines all three questions in the
/// order that lets each short-circuit.
///
/// Three behavioural changes fall out of using the real model:
///
/// - **Dim light exists.** Previously a token was fully visible or fully gone,
///   so a figure at the edge of torchlight popped in and out. Dim now renders
///   at reduced alpha.
/// - **Darkness is modelled, not just light**, which is what gives darkvision
///   something to work against.
/// - **Facing is honoured**, so a vision cone actually restricts what its
///   owner sees.
///
/// The observer is the token this client sees the board through: the local
/// player's own, as the application names it (`ViewerToken`). A Game Master
/// has none — there is no single point of view to occlude from, so occlusion
/// and facing are skipped — and loses no token to the dark: one a player
/// could not see is drawn dimmed for them instead. A GM sees the board, not
/// one character's slice of it, and a token they cannot find is one they
/// cannot run.
///
/// Playtest 2026-09-10 P9: the observer used to be whichever entity carried
/// `PlayerToken`, which is the engine's own demo token, spawned in every
/// session. Every client — the Game Master's included — saw the board from a
/// red square at (-180, 0), and no real player's token ever had a point of
/// view.
///
/// A scene with no lights and a bright ambient returns early untouched
/// (FR-013: a scene using none of these capabilities still renders normally).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_light_illumination(
    light_set: Res<LightSet>,
    wall_set: Res<WallSet>,
    ambient: Option<Res<SceneAmbient>>,
    is_gm: Option<Res<IsGameMaster>>,
    viewer: Option<Res<ViewerToken>>,
    token_positions: Query<(&Transform, &TokenIdentity, Option<&TokenVision>)>,
    mut tokens: Query<(
        &Transform,
        &TokenIdentity,
        Option<&TokenVision>,
        &mut Sprite,
        &mut Visibility,
    )>,
    // Whether the last pass hid or dimmed anything, so the quiet path below
    // can put it back rather than leave a token hidden from a view that no
    // longer applies.
    mut touched: Local<bool>,
) {
    let ambient = ambient.map_or_else(AmbientLight::daylight, |a| a.0);

    // Nothing to resolve in a lit scene with no lights — unless someone is
    // looking through a token, whose walls hide things in daylight too.
    let sees_through_a_token = !is_gm.as_ref().is_some_and(|gm| gm.0)
        && viewer.as_ref().is_some_and(|viewer| viewer.0.is_some());
    if light_set.lights().is_empty()
        && ambient.level == Illumination::Bright
        && !sees_through_a_token
    {
        // Everything is in plain sight again. Whatever an earlier pass hid —
        // a token behind a wall from a viewer since unset, or in a dark scene
        // since lit — is shown, not left hidden with nobody to unhide it.
        if *touched {
            for (_, _, _, mut sprite, mut visibility) in tokens.iter_mut() {
                *visibility = Visibility::Inherited;
                undim(&mut sprite);
            }
            *touched = false;
        }
        mirror_hidden_tokens(Vec::new());
        return;
    }
    *touched = true;

    let mut positions: HashMap<String, Vec2> = HashMap::new();
    for (transform, identity, _) in token_positions.iter() {
        positions.insert(identity.0.clone(), transform.translation.truncate());
    }

    let lights: Vec<ResolvedLight> = light_set
        .lights()
        .iter()
        .map(|light| resolve_light(light, &positions))
        .collect();

    let game_master = is_gm.is_some_and(|gm| gm.0);
    let observer = if game_master {
        None
    } else {
        viewer
            .as_ref()
            .and_then(|viewer| viewer.0.as_deref())
            .and_then(|id| {
                token_positions
                    .iter()
                    .find(|(_, identity, _)| identity.0 == id)
            })
            .map(|(transform, _, vision)| {
                (
                    transform.translation.truncate(),
                    vision.map_or_else(VisionProfile::default, |v| v.0),
                )
            })
    };

    let mut hidden = Vec::new();
    for (transform, identity, token_vision, mut sprite, mut visibility) in tokens.iter_mut() {
        let target = transform.translation.truncate();

        let perceived = match observer {
            Some((observer_pos, observer_vision)) => {
                // A token never hides from itself. Without this the observer
                // vanishes whenever it stands in its own darkness.
                if observer_pos.distance(target) <= f32::EPSILON {
                    Perceived::Clear
                } else {
                    visibility_of(
                        observer_pos,
                        &observer_vision,
                        target,
                        &lights,
                        &wall_set,
                        ambient,
                    )
                }
            }
            None => {
                // GM view: illumination only. Darkvision on the token itself
                // still lets it be picked out of the dark.
                let (level, _color) = illumination_at(target, &lights, &wall_set, ambient);
                match level {
                    Illumination::Bright => Perceived::Clear,
                    Illumination::Dim => Perceived::Dim,
                    Illumination::Dark => {
                        if game_master || token_vision.map_or(0.0, |v| v.darkvision) > 0.0 {
                            Perceived::Dim
                        } else {
                            Perceived::Hidden
                        }
                    }
                }
            }
        };

        match perceived {
            Perceived::Clear => {
                *visibility = Visibility::Inherited;
                // Plain sight takes whatever alpha the token already has —
                // selection feedback's, typically — undoing only a dim this
                // system applied. Forcing 1.0 overwrote selection's feedback.
                undim(&mut sprite);
            }
            Perceived::Dim => {
                *visibility = Visibility::Inherited;
                sprite.color = sprite.color.with_alpha(DIM_ALPHA);
            }
            Perceived::Hidden => {
                *visibility = UNLIT_VISIBILITY;
                hidden.push(identity.0.clone());
            }
        }
    }
    mirror_hidden_tokens(hidden);
}

/// Undo a dim this system applied, and nothing else: a token's own alpha —
/// selection feedback draws unselected tokens slightly transparent — is not
/// illumination's to reset.
fn undim(sprite: &mut Sprite) {
    if (sprite.color.alpha() - DIM_ALPHA).abs() < 1e-3 {
        sprite.color = sprite.color.with_alpha(1.0);
    }
}

/// The token this client sees the board through — the local player's own,
/// named by the application (`set_viewer_token`). `None` for a Game Master,
/// and for anyone the application has not named one for.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub(crate) struct ViewerToken(pub Option<String>);

type ViewerRequest = Option<Option<String>>;

static REQUESTED_VIEWER: std::sync::OnceLock<std::sync::Mutex<ViewerRequest>> =
    std::sync::OnceLock::new();

/// Name the token this client sees the board through; `""` for none.
///
/// Queued and applied on the next frame, like the engine's other web
/// commands. Local session state, not world state: which token is "mine" is
/// a fact about this viewer, so it is never synced or broadcast.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_viewer_token(token_id: &str) -> bool {
    let request = (!token_id.is_empty()).then(|| token_id.to_string());
    let slot = REQUESTED_VIEWER.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(request);
        return true;
    }
    false
}

pub(crate) fn apply_requested_viewer(mut viewer: ResMut<ViewerToken>) {
    let Some(slot) = REQUESTED_VIEWER.get() else {
        return;
    };
    let Ok(mut pending) = slot.lock() else {
        return;
    };
    if let Some(request) = pending.take() {
        viewer.set_if_neq(ViewerToken(request));
    }
}

static HIDDEN_TOKENS: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
    std::sync::OnceLock::new();

fn mirror_hidden_tokens(mut hidden: Vec<String>) {
    hidden.sort_unstable();
    let slot = HIDDEN_TOKENS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock()
        && *current != hidden
    {
        *current = hidden;
    }
}

/// The ids of the tokens this client currently hides, as a JSON array.
///
/// Read-only, and here so a test can ask what a player's canvas withholds —
/// a token out of their sight — rather than infer it from pixels.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hidden_tokens() -> String {
    let list = HIDDEN_TOKENS
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    serde_json::Value::from(list).to_string()
}

pub(crate) fn init_lighting_systems_resources(app: &mut App) {
    app.init_resource::<LightDragState>()
        .init_resource::<LightEntities>();
}

/// Perform the switching effect this subsystem contributed to the interaction
/// seam (spec 030, US3).
///
/// Reads the activation message like any other contributor and filters for the
/// one identifier `canvas_core::lighting` declared. Nothing in the interaction
/// plugin knows this system exists (FR-039, FR-040).
///
/// # Why the prior brightness is remembered
///
/// A light is off when its intensity is zero, so switching one off destroys
/// the only record of how bright it was. Without remembering it, a lever
/// pulled twice would leave the room at a brightness nobody chose. The server
/// stashes it authoritatively; this mirror keeps the optimistic path agreeing
/// with it rather than inventing its own answer.
///
/// This is the optimistic half. The server has already performed the same
/// change; applying it here makes it visible now rather than a round trip
/// later (ADR-054).
pub(crate) fn handle_switch_effects(
    mut activations: MessageReader<crate::plugins::interaction::InteractionActivated>,
    mut light_set: ResMut<LightSet>,
    mut remembered: Local<std::collections::HashMap<String, f32>>,
) {
    use thunderforge_canvas_core::lighting::{TOGGLE, lights_of, requested_on};

    for activation in activations.read() {
        if activation.effect_id != TOGGLE {
            continue;
        }
        for reference in lights_of(&activation.config) {
            let Some(existing) = light_set.get(reference) else {
                // Named a light this client does not hold. Not an error — the
                // next sync brings whatever the server thinks is true.
                continue;
            };
            let currently_on = existing.is_on();
            let Some(want_on) = requested_on(&activation.config, currently_on) else {
                continue;
            };
            if want_on == currently_on {
                continue;
            }

            let mut updated = existing.clone();
            if want_on {
                updated.intensity = remembered
                    .get(reference)
                    .copied()
                    .filter(|v| *v > 0.0)
                    // Full rather than nothing: a light turned on to zero
                    // looks exactly like the switch being broken.
                    .unwrap_or(1.0);
            } else {
                remembered.insert(reference.to_string(), updated.intensity);
                updated.intensity = 0.0;
            }
            light_set.upsert(updated);
        }
    }
}

#[cfg(test)]
#[path = "lighting_tests.rs"]
mod tests;
