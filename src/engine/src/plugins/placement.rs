//! Carrying a token to where it goes.
//!
//! # The gesture
//!
//! A Game Master picks an actor in the play screen's actors pane and chooses
//! Place. That actor's token attaches to the cursor, follows it, snaps as a
//! dragged token would, and is dropped by a left click. Escape — or losing the
//! right to the tool, or the connection — abandons it, leaving nothing.
//!
//! # Why the carry is a kind and a reference rather than an actor
//!
//! Spec 031 FR-011 asks for a lore marker to be placed on a map, and that is
//! the same gesture with a different thing in hand: a prop, which is a token
//! with no actor at all. So what is carried is a `kind` and an opaque
//! `reference`, neither of which this module interprets — the confirmation
//! hands both back and chrome, which asked for the carry, knows what they
//! meant. A second placement machine for props was the alternative, and it
//! would have had to re-derive snapping, the preview, the mode-change abandon
//! and the one-frame gap `abandon_on_mode_change` documents below.
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
    /// What sort of thing is in hand — `actor`, `prop`. Chrome's word, and
    /// empty when nothing is carried. Nothing here branches on its value:
    /// the engine's job is the gesture, and what the drop becomes is decided
    /// by whoever asked for it.
    pub kind: String,
    /// Opaque to the engine, handed back on the drop. The actor for an
    /// actor's token; for anything else, whatever chrome needs to recognise
    /// its own request.
    pub reference: String,
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

/// What the web app has asked to carry, if anything: its kind and reference.
static REQUESTED_PLACEMENT: std::sync::OnceLock<std::sync::Mutex<Option<(String, String)>>> =
    std::sync::OnceLock::new();

fn requested_placement_slot() -> &'static std::sync::Mutex<Option<(String, String)>> {
    REQUESTED_PLACEMENT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Whether the web app has asked to cancel.
static CANCEL_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Begin carrying something of `kind`, described to chrome by `reference`.
///
/// Returns false for an empty kind, because a confirmation chrome cannot
/// attribute to a request is worse than a carry that never began. The
/// reference may be empty: a prop placed from the interactions panel is
/// described entirely by the draft chrome is holding, and inventing an id
/// here would be a second name for it.
///
/// Whether this person may place anything is decided by the same rules that
/// govern creating a token, server-side; this only begins a gesture, and a
/// gesture creates nothing.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn begin_placement(kind: &str, reference: &str) -> bool {
    if kind.is_empty() {
        return false;
    }
    // A cancel that arrived while nothing was being carried was for a carry
    // that has already ended, and `resolve_placement` only ever reads the flag
    // while `Carrying` — so left set, it would abandon *this* request the
    // frame after it began. Chrome cancels on its way out of a panel, which
    // makes that the ordinary order rather than a rare race.
    CANCEL_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut slot) = requested_placement_slot().lock() {
        *slot = Some((kind.to_string(), reference.to_string()));
    }
    true
}

/// Begin carrying `actor_id`'s token.
///
/// Kept as its own entry point rather than folded into `begin_placement`: it
/// is what the actors pane calls, and an actor with no id is a request that
/// cannot be honoured, which `begin_placement` alone would accept.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn begin_token_placement(actor_id: &str) -> bool {
    if actor_id.is_empty() {
        return false;
    }
    begin_placement("actor", actor_id)
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

    let Some((kind, reference)) = requested else {
        return;
    };
    if !is_gm.0 {
        return;
    }

    carried.kind = kind;
    carried.reference = reference;
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
            "kind": carried.kind,
            "reference": carried.reference,
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
///
/// # Why this clears the carry as well as requesting the state change
///
/// Setting `NextState` is not enough on its own, and the gap is exactly one
/// frame wide. `OnExit(AuthoringMode::…)` runs in the `StateTransition`
/// schedule, *before* `Update`; the `PlacementState::Idle` it asks for is a
/// different state machine and does not apply until the following frame's
/// transition. So `resolve_placement` — gated on `PlacementState::Carrying`,
/// which is still true — would run once more in this frame's `Update`, under
/// the new tool, and a left click in that frame would confirm the placement.
///
/// That is spec 031's edge case in its most literal form: a gesture in flight
/// completing under a newly entered mode's rules. The wall, shape and lighting
/// tools do not have it, because each is gated on the very state whose `OnExit`
/// abandons it, and `in_state` is already false by the time `Update` runs.
///
/// Emptying `CarriedToken` closes it. Nothing is being carried the instant the
/// mode is left, and `carry_is_live` refuses to run the completion path over
/// nothing. The preview sprite is despawned a frame later by `clear_carry`,
/// which is the only place that has ever despawned it.
fn abandon_on_mode_change(
    state: Res<State<PlacementState>>,
    mut next: ResMut<NextState<PlacementState>>,
    mut carried: ResMut<CarriedToken>,
) {
    if *state.get() == PlacementState::Carrying {
        emit_event(json!({ "type": "token_placement_cancelled" }));
        carried.kind.clear();
        carried.reference.clear();
        next.set(PlacementState::Idle);
    }
}

/// Whether there is still something in hand.
///
/// The second half of the gate, alongside `in_state(Carrying)`: the state says
/// the gesture has not formally ended, and this says it has not been abandoned
/// within the frame. See `abandon_on_mode_change` for why one frame matters.
fn carry_is_live(carried: Res<CarriedToken>) -> bool {
    !carried.kind.is_empty()
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
                    .run_if(in_state(PlacementState::Carrying))
                    .run_if(carry_is_live),
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
            .add_systems(OnExit(AuthoringMode::Interactions), abandon_on_mode_change)
            // A scene change is a mode change too. Whatever the Game Master
            // was carrying belonged to the scene being left; dropping it into
            // the new one would place a token from the wrong map, and the
            // preview would be the only thing on screen that survived the
            // swap. Registered here rather than in the scene-transition plugin
            // so that plugin keeps knowing nothing about placement — and if it
            // is not added at all, this schedule simply never runs.
            .add_systems(
                OnEnter(crate::plugins::scene_transition::SceneTransition::Unloading),
                abandon_on_mode_change,
            );
    }
}
