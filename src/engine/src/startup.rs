//! `start` — building the Bevy app and handing it the canvas.

use super::*;

/// Boot the engine against a canvas.
///
/// Browser-only, because its app-builder body inserts `network::GraphQLClient`,
/// `network::websocket::WebSocketSubscription` and `network::mutations::
/// MutationTracker` and schedules `network::process_server_events`. Nothing
/// else in this file needs a browser, so the gate sits here rather than on the
/// module.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_selector: &str) {
    if ENGINE_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    console_error_panic_hook::set_once();

    let tracker = network::mutations::MutationTracker::new();

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.133, 0.157, 0.192)))
        .insert_resource(ActiveWorld("default".to_string()))
        .insert_resource(TokenEntities::default())
        .insert_resource(LastPlayerSent(Vec2::new(f32::MIN, f32::MIN)))
        .insert_resource(GridConfig::default())
        .insert_resource(network::GraphQLClient::new(
            "http://localhost:8080".to_string(),
        ))
        .insert_resource(network::websocket::WebSocketSubscription::new())
        .insert_resource(network::WorldEventSubscription::new())
        .insert_resource(tracker)
        .insert_resource(CircularFlowTracer::new())
        // Every asset this engine loads is a same-origin, server-authorized
        // URL under `/api/canvas-assets/...` (scene backgrounds and pasted
        // canvas images — see `systems/background.rs`, the only two
        // `asset_server.load` call sites). Those paths are rooted ("/…"),
        // which Bevy 0.18's `AssetPath::is_unapproved` treats as an escape
        // from the asset root, and the default `UnapprovedPathMode::Forbid`
        // then drops the load *before* any request is made — returning a
        // default handle after an `error!` that used to go nowhere, because
        // this crate did not enable `bevy_log` (it now does; see
        // Cargo.toml). Verified live: with `Forbid`, an imported dd2vtt
        // map's `set_scene_background` command arrived correctly and the
        // image was never requested at all, with nothing logged anywhere.
        // With `Allow`, the image is fetched and decoded, and the
        // background sprite reports `image_loaded == true`.
        // `Allow` is the right call for this app: the paths are not
        // filesystem paths at all, and the bytes behind them are already
        // authenticated and world-authorized server-side by
        // `canvas_assets_serve`.
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some(canvas_selector.to_owned()),
                        fit_canvas_to_parent: true,
                        focused: true,
                        resolution: WindowResolution::new(ARENA_WIDTH as u32, ARENA_HEIGHT as u32),
                        title: "ThunderForge Engine".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        // Spec 028 (T027): canvas image reads consult the local encrypted
        // cache before the network. Added after `DefaultPlugins` because it
        // needs `Assets<Image>` to exist, and deliberately depended on by
        // nothing: delete this one line and every canvas image loads through
        // `AssetServer` exactly as it did before (Constitution Principle II
        // — see the module docs in `plugins/cached_assets.rs`).
        .add_plugins(CachedAssetsPlugin)
        // Phase 4.7.F2: System Registration & Plugin Setup
        // Phase 4.7: Canvas Rendering Infrastructure
        // Which authoring tool is armed. Registered early because other
        // plugins will gate their input systems on it; adds no behaviour on
        // its own (see plugins/authoring_mode.rs).
        .add_plugins(plugins::authoring_mode::AuthoringModePlugin)
        // Spec 031 US4: `ready -> unloading -> loading -> ready`. Clears the
        // previous scene's content and asks for the next one's; it fetches
        // nothing and knows what none of the content *is*. Removing this line
        // leaves every other plugin working — the canvas simply never changes
        // scene (Constitution Principle II).
        //
        // Before `PlacementPlugin` for the same reason `AuthoringModePlugin`
        // is: placement hangs an `OnEnter` off this state to abandon a carry
        // when the scene changes, and a state is registered before anything
        // schedules against it.
        .add_plugins(plugins::scene_transition::SceneTransitionPlugin)
        .add_plugins(plugins::placement::PlacementPlugin)
        .add_plugins(plugins::selection_filter::SelectionFilterPlugin)
        // Spec 031 US6: right-clicking the map. Suppresses the browser menu on
        // the canvas element only and reports the gesture; chrome draws the
        // menu, because a menu is chrome. Removing this line restores the
        // browser menu and changes nothing else.
        .add_plugins(plugins::context_menu::ContextMenuPlugin)
        .add_plugins(ScenePlugin)
        .add_plugins(GridPlugin)
        .add_plugins(TokenPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SelectionPlugin) // Phase 4.7.E1: Token Selection
        // Spec 029: bars and counters above tokens. Independently removable —
        // taking this line out leaves every other plugin working, which is
        // what Constitution II asks of a plugin.
        .add_plugins(StatusDisplayPlugin)
        // Native canvas authoring (specs/001-bevy-canvas-authoring): shared
        // layer-ordering resource, must be added before Wall/Lighting/Shape
        // plugins so it exists when they build (Constitution Principle II)
        .add_plugins(CanvasLayerPlugin)
        // T015: wall authoring (specs/001-bevy-canvas-authoring). Depends
        // on CanvasLayerPlugin (above) for the `CanvasLayers` resource.
        .add_plugins(WallPlugin)
        // Spec 030: interactive elements. Deliberately registered *before* any
        // contributor, and deliberately depending on none of them — it is
        // addable and removable on its own, which is what FR-039 asks for and
        // what US7 tests.
        .add_plugins(plugins::InteractionPlugin)
        .add_plugins(plugins::interaction_marker::InteractionMarkerPlugin)
        // Spec 030 US1: the first contributor. Registered *after* the
        // interaction plugin and depending on nothing in it beyond the
        // message type — deleting this line removes the effect and leaves
        // everything else working, which is the property US7 tests.
        .add_plugins(plugins::LoreLinkPlugin)
        // Spec 030 US7: the contributor that exists only to be added and
        // removed. Deleting this line and its file removes the capability and
        // changes nothing else — which is the whole claim.
        .add_plugins(plugins::SeamProbePlugin)
        // Spec 031 US3: picking something up off the map. Another contributor
        // and nothing more — one file, one system, one declaration.
        .add_plugins(plugins::ItemPlugin)
        // Spec 030 US6: travel requests. Declaration only; the request and the
        // decision happen on the server and in the application, and this says
        // so rather than letting the seam report a working effect absent.
        .add_plugins(plugins::NavigationPlugin)
        // T040: light authoring (specs/001-bevy-canvas-authoring). Depends
        // on CanvasLayerPlugin (above) for the `CanvasLayers` resource, and
        // reads WallPlugin's `WallSet`/`is_visible` for occlusion.
        .add_plugins(LightingPlugin)
        // T056: shape/annotation authoring (specs/001-bevy-canvas-authoring).
        // Depends on CanvasLayerPlugin (above) for the `CanvasLayers`
        // resource; order relative to WallPlugin/LightingPlugin doesn't
        // matter (Constitution Principle II: independently addable).
        .add_plugins(ShapePlugin)
        // Scene background (map import art): renders into
        // `CanvasLayer::Background`, the lowest/furthest-back layer.
        // Depends on CanvasLayerPlugin (above); order relative to
        // WallPlugin/LightingPlugin/ShapePlugin doesn't matter.
        .add_plugins(BackgroundPlugin)
        // Spec 014 (US4): dice-bouncing reveal for a `rollDice` response
        // already handed to us — independent of every canvas plugin
        // above (Constitution Principle II).
        .add_plugins(DiceRollPlugin)
        // Raw per-frame timing ring, always on (one push per frame).
        .add_plugins(plugins::FrameTracePlugin)
        // Renderer self-test, off unless `set_render_probe` turns it on.
        .add_plugins(RenderProbePlugin)
        // Lighting/vision debug overlay (light radii, vision cones).
        .add_plugins(LightingOverlayPlugin)
        // The lighting layer itself: darkness over the map, light pools cut
        // out of it, wall shadows painted back in.
        .add_plugins(DarknessPlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (apply_external_commands, move_player, emit_player_state),
        )
        // The `GridPosition`-based movement path is NOT registered.
        //
        // `handle_keyboard_movement`, `sync_grid_to_transform`,
        // `sync_transform_to_grid` and `apply_grid_snapping` all query
        // `GridPosition`, and nothing in this engine has ever spawned an entity
        // carrying one outside a unit test — so none of them has ever matched a
        // live entity. They are the remains of an earlier sync design.
        //
        // Keeping `handle_keyboard_movement` scheduled would now be actively
        // harmful rather than merely inert: it binds the same WASD/arrow keys
        // as `systems::token_move`, so the moment anything did spawn a
        // `GridPosition` every keypress would move a token twice.
        //
        // The live equivalents work on `Transform`:
        //   movement -> `systems::token_move::handle_token_movement_input`
        //   snapping -> `systems::token_grid::snap_tokens_to_grid`
        .add_systems(Update, calculate_derived_stats)
        .add_systems(
            Update,
            (
                network::websocket::poll_websocket_stream,
                network::process_server_events,
                systems::process_mutation_results,
                process_server_responses,
                handle_mutation_errors,
                trace_keyboard_input,
            ),
        )
        // Phase 4.6: Token sync systems (temporarily disabled for Phase 4.7.F1/A1 validation)
        // TODO: Re-enable after Phase 4.6 code is refactored
        // .add_systems(
        //     Update,
        //     (
        //         systems::handle_token_move_system,
        //         systems::handle_mutation_rejection_system,
        //         systems::process_mutation_confirmations,
        //     ),
        // )
        .add_systems(
            Update,
            (
                trace_mutation_sent,
                trace_server_event,
                trace_update_confirmation,
                trace_rollback,
                print_flow_summary,
            ),
        )
        .run();
}
