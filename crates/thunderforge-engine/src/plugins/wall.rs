use bevy::prelude::*;

use crate::plugins::authoring_mode::AuthoringMode;

use crate::resources::{
    ActiveWallPrimitive, GridSnapEnabled, IsGameMaster, SceneGrid, SelectedWall, WallPrimitive,
    WallSet,
};
use crate::systems::wall::{
    handle_door_effects, handle_wall_input, handle_wall_keyboard_toggles, handle_wall_undo,
    init_wall_systems_resources, sync_wall_visuals,
};

/// Wires up wall authoring (T011-T014): the `WallSet` resource, GM-only
/// input systems (create/select/move-endpoint/delete/toggle), undo, the
/// sprite-sync render pass into `CanvasLayer::Walls`, and the vision-
/// occlusion first-pass. Depends on `CanvasLayers` existing
/// (`CanvasLayerPlugin` must be added first — see lib.rs) since
/// `sync_wall_visuals` reads `CanvasLayer::Walls.z()`.
///
/// Independently addable/removable per Constitution Principle II: nothing
/// outside this plugin depends on walls existing (`apply_external_commands`
/// in lib.rs degrades gracefully — an `upsert_wall`/`remove_wall` command
/// arriving with no `WallSet` present would simply not be dispatched,
/// since wall command handling is registered by this plugin, not lib.rs's
/// core command loop).
pub struct WallPlugin;

impl Plugin for WallPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WallSet>()
            .init_resource::<SelectedWall>()
            .init_resource::<IsGameMaster>()
            // Which of segment/room/door a drag draws (FR-026).
            .init_resource::<ActiveWallPrimitive>()
            // Snapping, and the lattice to snap to. Both are owned by other
            // plugins and both are read by `handle_wall_input`, so this plugin
            // registers them itself rather than requiring `TokenPlugin` and
            // `GridPlugin` to have been added first. `init_resource` is
            // idempotent, so this costs nothing when they have been
            // (Constitution Principle II — the same argument the
            // `IsGameMaster` line above already makes).
            //
            // The defaults are the ones FR-024 asks for: snapping on, and a
            // gridless scene until the server says otherwise, which
            // `SnapRule::is_active` treats as nothing to snap to.
            .init_resource::<GridSnapEnabled>()
            .init_resource::<SceneGrid>()
            // Registered here as well as in `InteractionPlugin`, and
            // `add_message` is idempotent. A contributor that could only be
            // added *after* the seam would not be independently addable, and
            // Principle II asks for plugins that are.
            .add_message::<crate::plugins::interaction::InteractionActivated>();

        // What this subsystem contributes to the seam's vocabulary, declared
        // beside the systems that perform it so the two cannot drift apart.
        crate::plugins::interaction::contribute(
            app,
            thunderforge_canvas_core::wall::interaction_effects(),
        );

        init_wall_systems_resources(app);

        app.add_systems(OnExit(AuthoringMode::Walls), abandon_wall_gesture);

        app.add_systems(
            Update,
            (
                // Before the input system, so a primitive chosen this frame is
                // the one this frame's click draws.
                apply_requested_primitive,
                // Only while the wall tool is armed.
                //
                // This system used to be gated on `IsGameMaster` alone, which
                // meant it competed for every Game Master click with token
                // dragging, shapes and lighting — all of which were armed at
                // the same time, because the engine had no idea which tool the
                // rail was showing. Spec 031 FR-040a: exactly one authority
                // decides the mode, and it is the engine.
                //
                // `IsGameMaster` stays as the inner check. The mode says *what*
                // a click means; the role says whether this person may author
                // at all, and that is not the mode's business.
                handle_wall_input
                    .run_if(in_state(AuthoringMode::Walls))
                    // And only while this viewer may use the tool at all.
                    // The mode gate above cannot cover a revocation: the
                    // state change that takes a lost tool away lands a frame
                    // later, and a click in that frame would still draw
                    // (spec 031 SC-012).
                    .run_if(crate::plugins::authoring_mode::authoring_tool_allowed(
                        AuthoringMode::Walls,
                    )),
                handle_wall_keyboard_toggles,
                handle_wall_undo,
                // Spec 030: doors, contributed to the interaction seam. Reads
                // the activation message; nothing in the interaction plugin
                // knows this system exists (FR-039, FR-040). Placed before
                // `sync_wall_visuals` so a door that opened this frame is
                // drawn open this frame.
                handle_door_effects,
                sync_wall_visuals,
            )
                .chain(),
        );
    }
}

/// Discard this tool's unfinished gesture when its mode is left.
///
/// `OnExit` is the whole reason the authoring mode is a Bevy state rather than
/// a resource holding a tool name: there is exactly one place where leaving a
/// mode happens, so "abandon whatever was in progress" is written once instead
/// of at every path that could change tools.
///
/// Spec 031 FR-040a and its edge case: a drag begun under one tool must not
/// complete under another's rules. The user changed what a click means partway
/// through; reinterpreting the half-finished gesture would be guessing.
fn abandon_wall_gesture(
    mut drag: ResMut<crate::systems::wall::WallDragState>,
    mut chain: ResMut<crate::systems::wall::WallChainState>,
) {
    drag.abandon();
    chain.abandon();
}

/// What the web app has most recently asked the wall tool to draw.
///
/// A slot rather than a direct write, for the reason every other boundary in
/// this crate uses one: `App::run()` owns the `World` and never returns on
/// wasm, so there is no handle to set a resource from outside the schedule.
static REQUESTED_PRIMITIVE: std::sync::OnceLock<std::sync::Mutex<Option<WallPrimitive>>> =
    std::sync::OnceLock::new();

fn requested_primitive_slot() -> &'static std::sync::Mutex<Option<WallPrimitive>> {
    REQUESTED_PRIMITIVE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Choose what the wall tool draws: `segment`, `room` or `door`.
///
/// FR-026 asks for these to be selectable *while drawing*, so this is a
/// setting on the armed tool rather than a separate tool of its own — the wall
/// tool stays the wall tool, and the primitive says what a drag means.
///
/// Returns whether the name was recognised. `false` leaves the current
/// primitive untouched, matching `set_authoring_mode`: a name this build does
/// not know must not silently change what the next drag draws.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_wall_primitive(primitive_id: &str) -> bool {
    let Some(primitive) = WallPrimitive::from_id(primitive_id) else {
        return false;
    };
    if let Ok(mut slot) = requested_primitive_slot().lock() {
        *slot = Some(primitive);
    }
    true
}

/// Mirrors the live primitive outward, for `wall_primitive()`.
static CURRENT_PRIMITIVE: std::sync::OnceLock<std::sync::Mutex<WallPrimitive>> =
    std::sync::OnceLock::new();

/// What the wall tool is currently drawing.
///
/// Observation only, so a test can ask the engine rather than infer it from
/// the walls that came out — the same reason `authoring_mode()` exists.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn wall_primitive() -> String {
    CURRENT_PRIMITIVE
        .get()
        .and_then(|slot| slot.lock().ok().map(|primitive| *primitive))
        .unwrap_or_default()
        .as_id()
        .to_string()
}

/// Apply whatever the web app asked for, once per frame.
///
/// Changing the primitive abandons an unfinished chain, for the same reason
/// leaving the tool does: the points were placed under a rule about what the
/// next click means, and that rule has just changed. Finishing them under the
/// new one would be guessing (spec 031 FR-040a's edge case, one level down).
fn apply_requested_primitive(
    mut active: ResMut<ActiveWallPrimitive>,
    mut chain: ResMut<crate::systems::wall::WallChainState>,
) {
    if let Some(requested) = requested_primitive_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
        && requested != active.0
    {
        active.0 = requested;
        chain.abandon();
    }

    // Mirrored unconditionally, so the readable value follows the resource
    // even when it changed for a reason other than a request.
    if let Ok(mut mirror) = CURRENT_PRIMITIVE
        .get_or_init(|| std::sync::Mutex::new(WallPrimitive::default()))
        .lock()
    {
        *mirror = active.0;
    }
}
