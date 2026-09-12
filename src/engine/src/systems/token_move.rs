//! Keyboard token movement, with a plan-then-commit path for turn-based play.
//!
//! # The gesture
//!
//! - **WASD / arrows** — move the owned token one cell immediately.
//! - **Shift + WASD / arrows** — extend a planned route instead of moving.
//!   The route draws as a line with its cost in the scene's own units.
//! - **Space** — commit the plan: the token moves to the end of the route.
//! - **Escape** — discard it.
//!
//! Planning exists because a move costs a resource the player is budgeting.
//! Seeing the route and its price before paying is the entire point; a token
//! that teleports on keypress gives them nothing to reason about.
//!
//! # Whose token
//!
//! Only the token this client owns. Movement is authored from an ownership
//! position, not a selection one — a player nudging the arrow keys must never
//! move the monster they happen to have clicked on.

use bevy::prelude::*;

use crate::movement::PlayerControlled;
use crate::resources::{SceneGrid, TokenGridBehaviour};
use crate::{ActiveWorld, TokenIdentity, emit_event};
use serde_json::json;
use thunderforge_canvas_core::grid::GridKind;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::movement::{PlannedPath, Step};

/// The scene's distance vocabulary — 5 ft, 1.5 m, 1 Unit.
#[derive(Resource, Clone, Debug, Default, Deref, DerefMut)]
pub struct SceneUnits(pub GridUnits);

/// The route currently being planned, if any.
#[derive(Resource, Default, Debug)]
pub struct MovementPlan {
    pub path: Option<PlannedPath>,
}

/// Marks the entities drawing the current plan's distance label.
#[derive(Component)]
pub(crate) struct PlanLabel;

fn pressed_step(keyboard: &ButtonInput<KeyCode>) -> Option<Step> {
    // `just_pressed`, not `pressed`: a movement step is a discrete action. Held
    // keys repeating every frame would run a token off the board in under a
    // second and make a planned route impossible to author.
    if keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp) {
        Some(Step::North)
    } else if keyboard.just_pressed(KeyCode::KeyS) || keyboard.just_pressed(KeyCode::ArrowDown) {
        Some(Step::South)
    } else if keyboard.just_pressed(KeyCode::KeyA) || keyboard.just_pressed(KeyCode::ArrowLeft) {
        Some(Step::West)
    } else if keyboard.just_pressed(KeyCode::KeyD) || keyboard.just_pressed(KeyCode::ArrowRight) {
        Some(Step::East)
    } else {
        None
    }
}

fn shift_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

/// Handles movement input for the owned token.
pub(crate) fn handle_token_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    grid: Res<SceneGrid>,
    mut plan: ResMut<MovementPlan>,
    active_world: Res<ActiveWorld>,
    mut owned: Query<
        (&mut Transform, &TokenIdentity, Option<&TokenGridBehaviour>),
        With<PlayerControlled>,
    >,
) {
    MOVE_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let Ok((mut transform, identity, behaviour)) = owned.single_mut() else {
        // No owned token — nothing to move. A spectator or GM view lands here.
        return;
    };

    let footprint = behaviour.copied().unwrap_or_default().footprint;
    let current = transform.translation.truncate();

    if keyboard.just_pressed(KeyCode::Escape) {
        plan.path = None;
        return;
    }

    // Commit.
    if keyboard.just_pressed(KeyCode::Space) {
        if let Some(path) = plan.path.take()
            && !path.is_empty()
        {
            let destination = grid.cell_center(path.head());
            let snapped = grid.snap_footprint(destination, footprint);
            transform.translation.x = snapped.x;
            transform.translation.y = snapped.y;

            // `upsert_token`, which is the event the application listens for
            // — the same one a drag emits. This used to be `update_token`,
            // carrying the route as `pathCells`: a shape nothing in the web
            // has ever handled, so a committed route moved this canvas and
            // told nobody (spec 045, found by the playtest's keyboard step
            // after the controlled token was fixed and the key *still* did
            // nothing anywhere but here).
            //
            // The route goes back when the server can judge it — phase 3 of
            // this feature's plan adds a path to `moveOwnToken`.
            emit_token_move(&transform, &identity.0, &active_world.0);
        }
        return;
    }

    let Some(step) = pressed_step(&keyboard) else {
        return;
    };
    MOVE_PRESSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Gridless scenes have no cells to step between, so keyboard movement
    // falls back to nudging by the nominal cell size rather than doing nothing.
    if grid.kind == GridKind::Gridless {
        let nudge = match step {
            Step::North => Vec2::new(0.0, grid.size),
            Step::South => Vec2::new(0.0, -grid.size),
            Step::East => Vec2::new(grid.size, 0.0),
            Step::West => Vec2::new(-grid.size, 0.0),
        };
        transform.translation += nudge.extend(0.0);
        // Told, like every other move. Without this the token moved on this
        // one canvas and nowhere else: not to the server, not to the table,
        // and not across a reload (spec 045).
        emit_token_move(&transform, &identity.0, &active_world.0);
        return;
    }

    if shift_held(&keyboard) {
        let path = plan
            .path
            .get_or_insert_with(|| PlannedPath::new(grid.world_to_cell(current)));
        path.push(step, grid.kind);
        return;
    }

    // Unmodified: move now, and abandon any plan — the player has clearly
    // stopped planning.
    plan.path = None;
    let next = step.apply(grid.world_to_cell(current), grid.kind);
    let snapped = grid.snap_footprint(grid.cell_center(next), footprint);
    transform.translation.x = snapped.x;
    transform.translation.y = snapped.y;

    emit_token_move(&transform, &identity.0, &active_world.0);
}

/// Tell the application a token moved, in the one shape it listens for.
///
/// The same event a drag emits (`systems/token.rs`), carrying the whole
/// transform: the bridge forwards rotation and scale only when they are
/// present, and a move that omitted them would be read as a move that
/// cleared them.
fn emit_token_move(transform: &Transform, token_id: &str, world_id: &str) {
    let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
    emit_event(json!({
        "type": "upsert_token",
        "token": {
            "id": token_id,
            "x": transform.translation.x,
            "y": transform.translation.y,
            "z": transform.translation.z,
            "scale": transform.scale.x,
            "rotation": rotation_radians,
        },
        "worldId": world_id,
    }));
}

type ControlRequest = Option<Option<String>>;

static REQUESTED_CONTROL: std::sync::OnceLock<std::sync::Mutex<ControlRequest>> =
    std::sync::OnceLock::new();

/// Name the token this client's own player may move; `""` for none.
///
/// Queued and applied on the next frame, exactly like `set_viewer_token`,
/// and for the same reason: which token is mine to move is a fact about this
/// viewer, never synced and never broadcast.
///
/// Control and point of view are different questions. A Game Master sees
/// through no token and may still move any of them, so this is not
/// `set_viewer_token` under another name.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_controlled_token(token_id: &str) -> bool {
    let request = (!token_id.is_empty()).then(|| token_id.to_string());
    let slot = REQUESTED_CONTROL.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(request);
        return true;
    }
    false
}

/// The token this client's player may move, as last named by the application.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub(crate) struct ControlledToken(pub Option<String>);

static MOVEMENT_STATE: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();

/// How many times the movement system has run, and how many movement keys it
/// has actually seen. Together they separate "the system never runs" from
/// "it runs and the key never arrives" from "the key arrives and the move
/// goes nowhere" — three causes that look identical from outside.
static MOVE_RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static MOVE_PRESSES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// What this client believes about moving: which token is its player's, how
/// many are tagged, and whether the scene has a grid to step on.
///
/// Read-only, and here for the same reason `hidden_tokens` is: a keypress
/// that moves nothing has several possible causes — no controlled token, a
/// token named before it arrived, a scene with no grid — and a test should be
/// able to ask rather than infer from a canvas.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn movement_state() -> String {
    MOVEMENT_STATE
        .get()
        .and_then(|slot| slot.lock().ok().map(|state| state.clone()))
        .unwrap_or_else(|| String::from("{}"))
}

/// Keeps exactly the named token tagged `PlayerControlled`.
///
/// Before spec 045 the only tagged entity was the placeholder `setup_scene`
/// spawns at startup, so `handle_token_movement_input` — which drives the
/// single controlled token — moved a red square nobody could see, and a
/// player's keyboard did nothing to their own hero.
///
/// # Why a resource, reconciled every frame
///
/// The application names a token as soon as it knows which one is the
/// player's, which is routinely *before* the engine holds that token: the
/// scene's tokens arrive on their own schedule. A request applied once and
/// dropped is therefore lost exactly when it arrives first, and the web has no
/// reason to say it again — which is what the playtest saw when this was a
/// one-shot: the key still moved nothing. Remembering the name and reconciling
/// each frame makes arrival order stop mattering.
pub(crate) fn reconcile_controlled_token(
    mut commands: Commands,
    mut controlled: ResMut<ControlledToken>,
    grid: Option<Res<SceneGrid>>,
    tokens: Query<(Entity, &TokenIdentity, &Transform)>,
    tagged: Query<Entity, With<PlayerControlled>>,
) {
    if let Some(slot) = REQUESTED_CONTROL.get()
        && let Ok(mut pending) = slot.lock()
        && let Some(request) = pending.take()
    {
        controlled.set_if_neq(ControlledToken(request));
    }

    let found = controlled.0.as_deref().and_then(|wanted| {
        tokens
            .iter()
            .find(|(_, identity, _)| identity.0 == wanted)
            .map(|(entity, _, transform)| (entity, transform.translation.x))
    });
    let desired = found.map(|(entity, _)| entity);

    for entity in tagged.iter() {
        if Some(entity) != desired {
            commands.entity(entity).remove::<PlayerControlled>();
        }
    }
    if let Some(entity) = desired
        && tagged.get(entity).is_err()
    {
        commands.entity(entity).insert(PlayerControlled);
    }

    let state = format!(
        r#"{{"named":{},"found":{},"tagged":{},"grid":{},"runs":{},"presses":{},"x":{}}}"#,
        controlled
            .0
            .as_deref()
            .map(|id| format!("\"{id}\""))
            .unwrap_or_else(|| String::from("null")),
        desired.is_some(),
        tagged.iter().count(),
        grid.is_some(),
        MOVE_RUNS.load(std::sync::atomic::Ordering::Relaxed),
        MOVE_PRESSES.load(std::sync::atomic::Ordering::Relaxed),
        // `null`, not NaN: a token the engine has not found is the case this
        // probe exists for, and NaN is not JSON — the reader would throw
        // instead of reporting, exactly when it was needed.
        found
            .map(|(_, x)| x.to_string())
            .unwrap_or_else(|| String::from("null")),
    );
    let slot = MOVEMENT_STATE.get_or_init(|| std::sync::Mutex::new(String::new()));
    if let Ok(mut held) = slot.lock() {
        *held = state;
    }
}

/// Colour of the planned route. Distinct from the grid and from light colours
/// so a plan never reads as scene geometry.
const PLAN_COLOR: Color = Color::srgba(0.45, 0.85, 1.0, 0.95);

/// Draws the planned route and its cost.
pub(crate) fn draw_movement_plan(
    plan: Res<MovementPlan>,
    grid: Res<SceneGrid>,
    units: Res<SceneUnits>,
    mut commands: Commands,
    labels: Query<Entity, With<PlanLabel>>,
    mut gizmos: Gizmos,
) {
    // The label is a spawned entity rather than a gizmo because gizmos cannot
    // draw text. Despawned and respawned on change, which is cheap for one
    // entity and avoids tracking its state.
    for entity in labels.iter() {
        commands.entity(entity).despawn();
    }

    let Some(path) = plan.path.as_ref() else {
        return;
    };
    if path.is_empty() {
        return;
    }

    let points = path.world_points(&grid);
    gizmos.linestrip_2d(points.clone(), PLAN_COLOR);

    // A marker at every cell the route passes through, so a doubled-back path
    // is readable rather than a line drawn over itself.
    for point in &points {
        gizmos.circle_2d(*point, grid.size * 0.12, PLAN_COLOR);
    }

    // Destination ring, larger, so the endpoint is unambiguous.
    if let Some(end) = points.last() {
        gizmos.circle_2d(*end, grid.size * 0.42, PLAN_COLOR);

        commands.spawn((
            Text2d::new(units.format(path.cost_in_cells())),
            TextFont {
                // Scaled to the grid so the label stays readable at any zoom
                // and any cell size.
                font_size: FontSize::Px((grid.size * 0.28).clamp(12.0, 48.0)),
                ..default()
            },
            TextColor(PLAN_COLOR),
            Transform::from_translation((*end + Vec2::new(0.0, grid.size * 0.62)).extend(90.0)),
            PlanLabel,
        ));
    }
}
