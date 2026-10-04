//! Swapping one scene for another, as a state machine with three positions.
//!
//! Spec 031 US4 (FR-018), research R11. `Ready → Unloading → Loading → Ready`.
//!
//! # Why a state machine and not a function
//!
//! "Change the scene" is not one instant. The previous scene's content has to
//! stop existing, the new scene's content arrives over the network in an
//! unknown number of commands, and in between there is a window where the
//! canvas holds neither. Every system that touches scene content therefore has
//! a question it would otherwise have to ask for itself — *has the switch
//! finished?* — and a boolean answer to that question is one that each caller
//! gets to interpret.
//!
//! `OnEnter` owns unload and load, so nothing else has to ask. That is R11's
//! argument, and it is the same one `placement.rs` makes for carrying: the
//! transitions are where the ordering bugs live, and a state machine is the
//! shape that writes them down.
//!
//! # Why the engine does not fetch anything
//!
//! It cannot, and should not. Chrome owns the network and the server owns what
//! the new scene contains (ADR-046 makes the active scene server-authoritative
//! and broadcast). So `Loading` means *waiting*: the engine says which scene it
//! wants, chrome sends the content down the existing command path — the same
//! `upsert_token`/`upsert_wall`/`upsert_light` commands a live edit uses — and
//! tells the engine when it has finished. There is no second ingest path, and
//! nothing here parses a scene.
//!
//! That is why `Loading` has no timeout. A timeout would be the engine
//! deciding that chrome had failed, which it is in no position to know; the
//! honest failure is a canvas that stays empty and a state anyone can read.
//!
//! # What this plugin does not do
//!
//! It does not know what a token, a wall or a light *is* beyond the resource
//! that holds them, and it never asks another plugin to do anything. Each
//! collection is an `Option<ResMut<…>>`: remove `WallPlugin` and this still
//! compiles, runs, and clears everything else. Constitution Principle II asks
//! for plugins that can be taken out; that is what the options are for.

use bevy::prelude::*;
use serde_json::json;

use crate::emit_event;

/// Where the canvas is in a scene swap.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SceneTransition {
    /// Showing a scene, and not in the middle of changing it.
    #[default]
    Ready,
    /// The previous scene's content is being taken off the canvas.
    Unloading,
    /// Waiting for the new scene's content to arrive.
    Loading,
}

impl SceneTransition {
    /// The identifier the web app reads back, for observing the machine.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unloading => "unloading",
            Self::Loading => "loading",
        }
    }
}

/// Which scene the canvas is moving to.
///
/// Empty outside a transition. Carried as a resource rather than inside the
/// state so the state stays `Copy` and comparable — a `States` value that
/// contained a scene id would make `Loading("a")` and `Loading("b")` different
/// states, and every `OnEnter` would have to be registered per scene.
#[derive(Resource, Default, Debug)]
pub struct PendingScene(pub String);

/// The scene the web app has asked to move to, if any.
static REQUESTED_SCENE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();

fn requested_scene_slot() -> &'static std::sync::Mutex<Option<String>> {
    REQUESTED_SCENE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Whether the web app has said the new scene's content is all here.
static LOAD_COMPLETE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Mirrors the live state outward, for `scene_transition_state()`.
///
/// The same arrangement `authoring_mode()` uses and for the same reason:
/// `App::run()` owns the `World` and never returns on wasm, so a reader
/// outside the schedule has no handle to ask.
static CURRENT_STATE: std::sync::OnceLock<std::sync::Mutex<SceneTransition>> =
    std::sync::OnceLock::new();

fn current_state_slot() -> &'static std::sync::Mutex<SceneTransition> {
    CURRENT_STATE.get_or_init(|| std::sync::Mutex::new(SceneTransition::Ready))
}

/// Begin moving the canvas to `scene_id`.
///
/// Returns false for an empty id, which is the one request that cannot mean
/// anything — "change to no scene" is not a scene change, and clearing the
/// canvas without arriving anywhere is not something any surface asks for.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn begin_scene_transition(scene_id: &str) -> bool {
    if scene_id.is_empty() {
        return false;
    }
    if let Ok(mut slot) = requested_scene_slot().lock() {
        *slot = Some(scene_id.to_string());
    }
    true
}

/// Report that the new scene's content has all been sent.
///
/// Called by chrome once every `upsert_*` for the destination has been
/// dispatched. Idempotent, and harmless outside a transition: a stale call
/// simply sets a flag that the next `Loading` would have set anyway.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn complete_scene_transition() {
    LOAD_COMPLETE.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Where the machine currently is: `ready`, `unloading` or `loading`.
///
/// Read-only, for a test or a probe. Spec 031's success criteria are about
/// what a scene change leaves behind, and asserting that against a state is
/// far less circumstantial than asserting it against a screenshot.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn scene_transition_state() -> String {
    current_state_slot()
        .lock()
        .map(|state| *state)
        .unwrap_or_default()
        .as_str()
        .to_string()
}

/// Take up whatever scene change chrome asked for.
///
/// Not run while `Unloading`, which is a single frame: a request that arrives
/// during it stays in the slot and is taken next frame, rather than restarting
/// a teardown that is already half-done.
fn apply_requested_transition(
    mut pending: ResMut<PendingScene>,
    mut next: ResMut<NextState<SceneTransition>>,
) {
    let Some(scene_id) = requested_scene_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
    else {
        return;
    };

    // A late `complete_scene_transition` for the *previous* move must not end
    // this one the moment it starts.
    LOAD_COMPLETE.store(false, std::sync::atomic::Ordering::SeqCst);
    pending.0 = scene_id;
    next.set(SceneTransition::Unloading);
}

/// Keep the readable state following the live one.
fn mirror_state(state: Res<State<SceneTransition>>) {
    if let Ok(mut mirror) = current_state_slot().lock() {
        *mirror = *state.get();
    }
}

/// Everything the previous scene put on the canvas, taken back off.
///
/// FR-018 names tokens, walls and lights. Shapes and interactives are cleared
/// with them because they belong to a scene in exactly the same way: an arrow
/// drawn over the tavern floor is not part of the cellar, and an interactive
/// still holding the tavern's door id would dispatch against a wall that no
/// longer exists.
///
/// Selection goes too. A selected id that survives the swap is a panel in
/// chrome describing something nobody can see.
#[allow(clippy::too_many_arguments)]
fn unload_previous_scene(
    mut commands: Commands,
    pending: Res<PendingScene>,
    tokens: Query<Entity, With<crate::TokenIdentity>>,
    mut token_entities: ResMut<crate::TokenEntities>,
    mut walls: Option<ResMut<crate::resources::WallSet>>,
    mut lights: Option<ResMut<crate::resources::LightSet>>,
    mut shapes: Option<ResMut<crate::resources::ShapeSet>>,
    mut interactives: Option<ResMut<crate::plugins::interaction::Interactives>>,
    mut previous_positions: Option<ResMut<crate::plugins::interaction::PreviousPositions>>,
    mut selected_token: Option<ResMut<crate::resources::SelectedToken>>,
    mut selected_wall: Option<ResMut<crate::resources::SelectedWall>>,
    mut selected_light: Option<ResMut<crate::resources::SelectedLight>>,
    mut selected_shape: Option<ResMut<crate::resources::SelectedShape>>,
    mut next: ResMut<NextState<SceneTransition>>,
) {
    let mut token_count = 0;
    for entity in tokens.iter() {
        commands.entity(entity).despawn();
        token_count += 1;
    }
    token_entities.clear();

    // Stale entries here are worse than they look: entry detection compares a
    // token's previous position with its current one, so a token id that
    // reappears in the new scene would be read as having *travelled* there
    // from wherever it stood in the old one, crossing every region in between.
    if let Some(previous_positions) = previous_positions.as_deref_mut() {
        previous_positions.0.clear();
    }

    if let Some(walls) = walls.as_deref_mut() {
        walls.clear();
    }
    if let Some(lights) = lights.as_deref_mut() {
        lights.clear();
    }
    if let Some(shapes) = shapes.as_deref_mut() {
        shapes.clear();
    }
    if let Some(interactives) = interactives.as_deref_mut() {
        interactives.clear();
    }

    if let Some(selected) = selected_token.as_deref_mut() {
        selected.deselect();
    }
    if let Some(selected) = selected_wall.as_deref_mut() {
        selected.deselect();
    }
    if let Some(selected) = selected_light.as_deref_mut() {
        selected.deselect();
    }
    if let Some(selected) = selected_shape.as_deref_mut() {
        selected.deselect();
    }

    emit_event(json!({
        "type": "scene_unloaded",
        "sceneId": pending.0,
        "tokensRemoved": token_count,
    }));

    next.set(SceneTransition::Loading);
}

/// Ask for the new scene's content.
///
/// The engine states what it needs and stops. Chrome fetches, because chrome
/// owns the network; see the module docs.
fn request_new_scene(pending: Res<PendingScene>) {
    emit_event(json!({
        "type": "scene_load_requested",
        "sceneId": pending.0,
    }));
}

/// Finish once chrome says the content is all here.
fn finish_loading(pending: Res<PendingScene>, mut next: ResMut<NextState<SceneTransition>>) {
    if !LOAD_COMPLETE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    emit_event(json!({
        "type": "scene_transition_complete",
        "sceneId": pending.0,
    }));
    next.set(SceneTransition::Ready);
}

/// The scene id stops being *pending* once the canvas is showing it.
fn clear_pending_scene(mut pending: ResMut<PendingScene>) {
    pending.0.clear();
}

pub struct SceneTransitionPlugin;

impl Plugin for SceneTransitionPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SceneTransition>()
            .init_resource::<PendingScene>()
            .add_systems(
                Update,
                (
                    apply_requested_transition.run_if(not(in_state(SceneTransition::Unloading))),
                    finish_loading.run_if(in_state(SceneTransition::Loading)),
                    // Last, so the readable value reflects the state this frame
                    // ran under rather than the one before it.
                    mirror_state,
                )
                    .chain(),
            )
            .add_systems(OnEnter(SceneTransition::Unloading), unload_previous_scene)
            .add_systems(OnEnter(SceneTransition::Loading), request_new_scene)
            .add_systems(OnEnter(SceneTransition::Ready), clear_pending_scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-only under wasm32, like every other test in this crate — the
    // rules with real consequences live in `thunderforge-canvas-core`. Kept
    // because they pin the vocabulary the web app matches on.

    #[test]
    fn state_names_are_the_ones_the_spec_uses() {
        assert_eq!(SceneTransition::Ready.as_str(), "ready");
        assert_eq!(SceneTransition::Unloading.as_str(), "unloading");
        assert_eq!(SceneTransition::Loading.as_str(), "loading");
    }

    #[test]
    fn ready_is_the_default() {
        // A canvas that has never changed scene is not mid-transition.
        assert_eq!(SceneTransition::default(), SceneTransition::Ready);
    }

    #[test]
    fn an_empty_scene_id_is_refused() {
        // Clearing the canvas without arriving anywhere is not a scene change.
        assert!(!begin_scene_transition(""));
    }
}
