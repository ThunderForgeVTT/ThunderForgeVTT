//! Carrying a token to where it goes.
//!
//! # The gesture
//!
//! A Game Master picks an actor in the play screen's actors pane and chooses
//! Place. That actor's token attaches to the cursor, follows it, snaps as a
//! dragged token would, and is dropped by a left click. Escape — or losing the
//! right to the tool, or the connection — abandons it, leaving nothing.
//!
//! # Why this is a state and not a boolean
//!
//! `Carrying` has an exit, and everything that matters here happens on it.
//! "Leave no trace" is a single `OnExit` system rather than a cleanup call at
//! every path out of a carry: the drop, Escape, a mode change, a revoked
//! permission, a scene change. Spec 031 lists a dropped connection mid-carry
//! as an edge case, and that path never runs any of this module's code — the
//! state simply exits with the page, and nothing was ever persisted.
//!
//! # Why the engine owns the preview
//!
//! The real `<canvas>` is a `body`-level element inserted by Bevy/winit, and
//! screen-to-world is the camera's business. A React element following the
//! mouse would drift against the camera, ignore the grid, and become a second
//! source of truth for canvas state — the failure Constitution I exists to
//! prevent. Chrome asks for a placement and is told what happened; it never
//! positions anything.
//!
//! # What is not here
//!
//! Persistence. The engine reports a confirmed placement and chrome calls the
//! server, which decides. A refusal is a token that never existed rather than
//! one that has to be taken back.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::snapping::SnapRule;

use crate::emit_event;
use crate::plugins::authoring_mode::AuthoringMode;
use crate::resources::grid::SceneGrid;
use crate::resources::token_grid::GridSnapEnabled;
use crate::resources::wall::IsGameMaster;

/// Whether a token is currently attached to the cursor.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlacementState {
    #[default]
    Idle,
    Carrying,
}

/// What is being carried, while it is being carried.
///
/// Cleared on `OnExit(Carrying)`, which is the single guarantee that a
/// cancelled placement leaves nothing behind.
#[derive(Resource, Default)]
pub struct CarriedToken {
    /// The actor whose token this will become. Empty when nothing is carried.
    pub actor_id: String,
    /// Where the preview currently sits, already snapped.
    pub at: Vec2,
}

/// Marks the provisional sprite, so `OnExit` can despawn it without knowing
/// how it was built.
#[derive(Component)]
struct PlacementPreview;

/// The colour of something that is not real yet.
///
/// Deliberately not the token's own art: a preview that looks exactly like a
/// placed token invites the reading that it *is* placed, and the whole point
/// of this state is that nothing has happened yet.
const PREVIEW_COLOR: Color = Color::srgba(0.55, 0.78, 1.0, 0.55);
const PREVIEW_SIZE: f32 = 48.0;

/// What the web app has asked to carry, if anything.
static REQUESTED_PLACEMENT: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();

fn requested_placement_slot() -> &'static std::sync::Mutex<Option<String>> {
    REQUESTED_PLACEMENT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Whether the web app has asked to cancel.
static CANCEL_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Begin carrying `actor_id`'s token.
///
/// Returns false for an empty id. Whether this person may place anything is
/// decided by the same rules that govern creating a token, server-side; this
/// only begins a gesture, and a gesture creates nothing.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn begin_token_placement(actor_id: &str) -> bool {
    if actor_id.is_empty() {
        return false;
    }
    if let Ok(mut slot) = requested_placement_slot().lock() {
        *slot = Some(actor_id.to_string());
    }
    true
}

/// Abandon a placement in progress, from outside the engine.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn cancel_token_placement() {
    CANCEL_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Start carrying when chrome asks.
fn begin_requested_placement(
    is_gm: Res<IsGameMaster>,
    mut carried: ResMut<CarriedToken>,
    mut next: ResMut<NextState<PlacementState>>,
) {
    let requested = requested_placement_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());

    let Some(actor_id) = requested else {
        return;
    };
    if !is_gm.0 {
        return;
    }

    carried.actor_id = actor_id;
    next.set(PlacementState::Carrying);
}

/// Follow the cursor, snapped.
fn follow_cursor(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    grid: Res<SceneGrid>,
    snap_enabled: Res<GridSnapEnabled>,
    mut carried: ResMut<CarriedToken>,
    mut preview: Query<&mut Transform, With<PlacementPreview>>,
) {
    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    // The same rule a drag uses, so a placed token cannot land where a dragged
    // one could not (FR-006). `SnapRule::token` is `GridSpec::snap_footprint`,
    // which is the call the drag path already makes.
    let rule = SnapRule::new(grid.0, snap_enabled.0);
    carried.at = rule.token(cursor, Footprint::default());

    if let Ok(mut transform) = preview.single_mut() {
        transform.translation.x = carried.at.x;
        transform.translation.y = carried.at.y;
    }
}

/// Drop it, or abandon it.
fn resolve_placement(
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    carried: Res<CarriedToken>,
    mut next: ResMut<NextState<PlacementState>>,
) {
    let cancelled_from_chrome = CANCEL_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst);

    if cancelled_from_chrome || keyboard.just_pressed(KeyCode::Escape) {
        emit_event(json!({ "type": "token_placement_cancelled" }));
        next.set(PlacementState::Idle);
        return;
    }

    if mouse_button.just_pressed(MouseButton::Left) {
        // Reported, not persisted. Chrome calls the server and the server
        // decides; a refusal is simply a token that never came to exist.
        emit_event(json!({
            "type": "token_placement_confirmed",
            "actorId": carried.actor_id,
            "x": carried.at.x,
            "y": carried.at.y,
        }));
        next.set(PlacementState::Idle);
    }
}

/// Spawn the provisional sprite.
fn show_preview(mut commands: Commands, carried: Res<CarriedToken>) {
    commands.spawn((
        Sprite::from_color(PREVIEW_COLOR, Vec2::splat(PREVIEW_SIZE)),
        Transform::from_xyz(carried.at.x, carried.at.y, 500.0),
        PlacementPreview,
    ));
}

/// Leave nothing behind.
///
/// The single place a carry ends, whichever way it ended: dropped, escaped,
/// cancelled by chrome, interrupted by a mode change, or ended because the
/// person may no longer use the tool. That is the reason this is a state.
fn clear_carry(
    mut commands: Commands,
    mut carried: ResMut<CarriedToken>,
    preview: Query<Entity, With<PlacementPreview>>,
) {
    for entity in preview.iter() {
        commands.entity(entity).despawn();
    }
    *carried = CarriedToken::default();
}

/// A carry does not survive leaving the tool it was begun under.
fn abandon_on_mode_change(
    state: Res<State<PlacementState>>,
    mut next: ResMut<NextState<PlacementState>>,
) {
    if *state.get() == PlacementState::Carrying {
        emit_event(json!({ "type": "token_placement_cancelled" }));
        next.set(PlacementState::Idle);
    }
}

/// Duplicated from the other canvas-authoring modules, which each keep their
/// own copy rather than widening `systems/selection.rs`'s visibility.
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    camera.viewport_to_world_2d(camera_transform, cursor).ok()
}

pub struct PlacementPlugin;

impl Plugin for PlacementPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<PlacementState>()
            .init_resource::<CarriedToken>()
            .add_systems(
                Update,
                begin_requested_placement.run_if(in_state(PlacementState::Idle)),
            )
            .add_systems(
                Update,
                (follow_cursor, resolve_placement)
                    .chain()
                    .run_if(in_state(PlacementState::Carrying)),
            )
            .add_systems(OnEnter(PlacementState::Carrying), show_preview)
            .add_systems(OnExit(PlacementState::Carrying), clear_carry)
            // Leaving *any* authoring mode ends a carry. Registered per mode
            // because Bevy's transitions are per-state-value; the effect is
            // "any mode change abandons the gesture", which is FR-040a's edge
            // case applied to placement.
            .add_systems(OnExit(AuthoringMode::Select), abandon_on_mode_change)
            .add_systems(OnExit(AuthoringMode::Walls), abandon_on_mode_change)
            .add_systems(OnExit(AuthoringMode::Lights), abandon_on_mode_change)
            .add_systems(OnExit(AuthoringMode::Shapes), abandon_on_mode_change)
            .add_systems(OnExit(AuthoringMode::Tokens), abandon_on_mode_change)
            .add_systems(OnExit(AuthoringMode::Interactions), abandon_on_mode_change);
    }
}
