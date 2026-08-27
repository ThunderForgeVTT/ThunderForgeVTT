use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use bevy::asset::{AssetPlugin, UnapprovedPathMode};
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};
use js_sys::Function;
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

// Public module exports for Phase 4.2/4.3 Bevy integration
pub mod components;
pub mod derived_data;
pub mod movement;
pub mod network;
pub mod sync_test;
pub mod systems;

// Phase 4.7: Canvas Rendering Infrastructure
pub mod grid;
pub mod plugins;
pub mod resources;
pub mod transforms;

// Phase 4.7.G2: Integration & E2E Tests
mod integration_tests;

use derived_data::*;
use movement::PlayerControlled;
use plugins::{
    BackgroundPlugin, CameraPlugin, CanvasLayerPlugin, DarknessPlugin, DiceRollPlugin, GridPlugin,
    LightingOverlayPlugin, LightingPlugin, RenderProbeEnabled, RenderProbePlugin, ScenePlugin,
    SelectionPlugin, ShapePlugin, SystemRegistrationPlugin, TokenPlugin, WallPlugin,
};
use resources::{
    CameraManager, DoorState, GridSnapEnabled, GridVisible, IsGameMaster, LightSet,
    LightSource as EngineLight, LightingOverlay, PlacedCanvasImage, PlacedCanvasImages,
    SceneAmbient, SceneBackground, SceneGrid, Shape as EngineShape, ShapeKind, ShapeSet,
    TokenGridBehaviour, TokenVision, Wall as EngineWall, WallSet,
};
use sync_test::*;
use systems::*;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::vision::{Illumination, Rgb, VisionProfile};

static ENGINE_STARTED: AtomicBool = AtomicBool::new(false);
static EVENT_CALLBACK: OnceLock<Mutex<Option<Function>>> = OnceLock::new();
static EXTERNAL_COMMANDS: OnceLock<Mutex<Vec<ExternalCommand>>> = OnceLock::new();
/// Latest engine performance counters, mirrored out of the ECS.
///
/// A mirror rather than a direct query because `App::run()` takes ownership of
/// its `World` and never returns on wasm — there is no handle left to read
/// from. A system inside the schedule writes here each frame, and
/// `engine_stats()` below reads it.
static ENGINE_STATS: OnceLock<Mutex<EngineStatsSnapshot>> = OnceLock::new();

const ARENA_WIDTH: f32 = 1280.0;
const ARENA_HEIGHT: f32 = 720.0;
const PLAYER_SPEED: f32 = 320.0;
pub(crate) const TOKEN_SIZE: Vec2 = Vec2::new(96.0, 96.0);

#[derive(Component)]
pub(crate) struct PlayerToken;

#[derive(Component)]
pub(crate) struct TokenIdentity(pub(crate) String);

#[derive(Resource, Default)]
pub(crate) struct ActiveWorld(pub(crate) String);

#[derive(Resource, Default)]
struct TokenEntities(HashMap<String, Entity>);

#[derive(Resource)]
struct LastPlayerSent(Vec2);

// Fields aren't read yet — grid rendering doesn't consult this resource
// today, but the value is inserted at startup ahead of that wiring landing.
#[allow(dead_code)]
#[derive(Resource, Clone, Debug)]
struct GridConfig {
    grid_size: f32,
    grid_type: String, // "square" or "hex"
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            grid_size: 32.0,
            grid_type: "square".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct WorldTokenPayload {
    id: String,
    x: f32,
    y: f32,
    z: f32,
    // Deserialized from the server payload but not yet read anywhere —
    // no nameplate/tooltip rendering consumes it yet.
    #[allow(dead_code)]
    label: Option<String>,
    // Spec 004 (US2): resize/rotate. Optional so the pre-existing
    // position-only `upsert_token` events (e.g. the WASD demo token, or a
    // plain drag that doesn't touch either) keep working unchanged —
    // `None` means "don't touch this token's current scale/rotation",
    // mirroring the server's own `TokenUpdate` partial-update semantics.
    #[serde(default)]
    rotation: Option<f32>,
    #[serde(default)]
    scale: Option<f32>,
    /// Token art. Carried end-to-end by the server (`tokens.photo_url`)
    /// and the web client (`WorldToken.photoUrl`) since spec 004, and
    /// ignored here until now — every token rendered as the same flat blue
    /// swatch no matter what art was set on it.
    ///
    /// `None` keeps that swatch, so every existing payload — the WASD demo
    /// token, a plain drag — behaves exactly as before.
    ///
    /// The art is fitted inside the token's grid footprint rather than
    /// stretched to it (`systems/token_grid::size_tokens_to_grid`), because
    /// almost no real token art is square.
    #[serde(default, rename = "photoUrl")]
    photo_url: Option<String>,
}

/// Confirmed/authoritative wall state from the server (T008), matching
/// the `upsert_wall` inbound command's `wall` payload shape.
#[derive(Debug, Clone, Deserialize)]
struct WorldWallPayload {
    id: String,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    #[serde(rename = "blocksVision")]
    blocks_vision: bool,
    #[serde(rename = "blocksMovement")]
    blocks_movement: bool,
    #[serde(rename = "doorState")]
    door_state: String,
}

/// Confirmed/authoritative light state from the server (T036-T040),
/// matching the `upsert_light` inbound command's `light` payload shape.
#[derive(Debug, Clone, Deserialize)]
struct WorldLightPayload {
    id: String,
    x: f32,
    y: f32,
    radius: f32,
    intensity: f32,
    color: Option<String>,
    #[serde(rename = "attachedTokenId")]
    attached_token_id: Option<String>,
    #[serde(rename = "castsShadows")]
    casts_shadows: bool,
}

/// Confirmed/authoritative shape state from the server (T053), matching
/// the `upsert_shape` inbound command's `shape` payload shape
/// (contracts/graphql.md's `geometry`/`style` blobs are opaque JSON, so
/// they're kept as raw `serde_json::Value` rather than typed fields).
#[derive(Debug, Clone, Deserialize)]
struct WorldShapePayload {
    id: String,
    kind: String,
    geometry: Value,
    text: Option<String>,
    style: Option<Value>,
    #[serde(rename = "visibleToPlayers")]
    visible_to_players: bool,
}

#[derive(Debug, Clone)]
enum ExternalCommand {
    SetWorld {
        world_id: String,
    },
    UpsertToken {
        token: WorldTokenPayload,
    },
    RemoveToken {
        token_id: String,
    },
    UpsertWall {
        wall: WorldWallPayload,
    },
    RemoveWall {
        wall_id: String,
    },
    UpsertLight {
        light: WorldLightPayload,
    },
    RemoveLight {
        light_id: String,
    },
    UpsertShape {
        shape: WorldShapePayload,
    },
    RemoveShape {
        shape_id: String,
    },
    /// Switches the active scene's background image (map import), or
    /// clears it (`path: None`) when the newly active scene has none.
    /// `width`/`height` are the scene's pixel dimensions, already computed
    /// server-side from `Scene.width`/`Scene.height` — this command does
    /// not fetch scene metadata itself.
    SetSceneBackground {
        path: Option<String>,
        width: f32,
        height: f32,
    },
    /// FR-010: whether the local session may author walls/shapes
    /// (`WallPlugin`/`ShapePlugin` gate all authoring input on
    /// `IsGameMaster`). Previously nothing ever sent this — the resource
    /// defaulted to `false` and stayed there for every session, so no GM
    /// could hand-draw a wall or shape through the real app at all. The
    /// frontend now sends this once on scene-owner status becoming known
    /// and whenever it changes (`WorldPage.tsx`).
    SetIsGameMaster {
        is_game_master: bool,
    },
    /// Spec 002 (US3): adds or updates one pasted canvas image on the
    /// active scene. `asset_id` is the `CanvasImageAsset.id` from
    /// `uploadCanvasImage`'s response.
    UpsertCanvasImageAsset {
        asset_id: String,
        path: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    RemoveCanvasImageAsset {
        asset_id: String,
    },
    /// Sets the active scene's grid — the lattice everything snaps, measures
    /// and draws against (`resources/grid.rs`). `grid_type` is the server's
    /// raw `scenes.grid_type` string; `size` is its `grid_size`, which for an
    /// imported map is the dd2vtt's own `pixels_per_grid`.
    SetSceneGrid {
        grid_type: String,
        size: f32,
        /// The map's pixel dimensions. When given, the lattice is anchored to
        /// the map's corner so it lands on the grid already painted on the
        /// art — which an origin-anchored lattice only does when the map has
        /// an even number of cells on both axes. Prefer this to `origin_*`.
        map_size: Option<Vec2>,
        origin_x: f32,
        origin_y: f32,
        visible: bool,
    },
    /// Sets a token's grid behaviour: how many cells across it is, and whether
    /// it snaps. Tokens are one cell and snapping unless told otherwise.
    ///
    /// `footprint` is in cells and is clamped at half a cell — a Tiny creature.
    /// Below that a token is smaller than the square it stands on and has no
    /// position to snap to.
    SetTokenGrid {
        token_id: String,
        footprint: f32,
        snap: bool,
    },
    /// Sets how the scene talks about distance: how much one cell is worth,
    /// and what that unit is called. 5/"ft" for D&D 5e, 1.5/"m" for a metric
    /// system, 1/"Unit" for an abstract one.
    SetGridUnits {
        per_cell: f32,
        label: String,
    },
    /// Scene-wide snapping switch. Turning it off suspends snapping for every
    /// token without editing any of them; turning it back on restores each
    /// token's own setting.
    SetGridSnap {
        enabled: bool,
    },
    /// Moves and zooms the camera. Any field may be omitted to leave it
    /// alone. `zoom` is world units per screen unit — larger is zoomed *out*.
    SetCamera {
        x: Option<f32>,
        y: Option<f32>,
        zoom: Option<f32>,
    },
    /// Frames a rectangle of world space, centring it and choosing the zoom
    /// that fits it. This is "zoom to fit the map".
    FitCameraTo {
        center_x: f32,
        center_y: f32,
        width: f32,
        height: f32,
    },
    /// Configures a token's eyes: darkvision range, facing, cone width and
    /// sight limit. Without this a token has unaided, omnidirectional sight —
    /// which means it cannot see anything standing in darkness.
    SetTokenVision {
        token_id: String,
        darkvision: f32,
        /// Facing in radians. `None` sees in all directions.
        facing: Option<f32>,
        /// Cone width in radians. Ignored when `facing` is `None`.
        fov: f32,
        max_range: Option<f32>,
    },
    /// Sets the scene's baseline illumination — daylight outdoors, dark in an
    /// unlit dungeon. This is the floor every light builds on, and what
    /// darkvision is measured against.
    SetAmbientLight {
        level: String,
        color: Option<String>,
    },
    /// Draws the lighting/vision debug overlay: light radii and vision cones.
    SetLightingOverlay {
        enabled: bool,
    },
    /// Turns the renderer self-test on or off (`plugins/render_probe.rs`).
    /// Draws through the gizmo pipeline rather than the sprite pipeline, so
    /// it can tell "the 2D render graph is dead" apart from "sprites
    /// specifically are not drawing".
    SetRenderProbe {
        enabled: bool,
    },
    /// Spec 014 (US4): the per-die final values from a `rollDice`
    /// response, already authoritative — this command only ever tells
    /// `DiceRollPlugin` what to animate toward, never asks it to decide
    /// an outcome (FR-015).
    TriggerDiceRoll {
        dice: Vec<DiceRollDiePayload>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct DiceRollDiePayload {
    #[serde(rename = "finalValue")]
    final_value: i64,
}

/// Plain-data mirror of `plugins::render_probe::EngineStats`.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub(crate) struct EngineStatsSnapshot {
    pub frame_time_ms: f64,
    pub fps: f64,
    pub sprites: usize,
    pub tokens: usize,
    pub lights: usize,
    pub walls: usize,
    pub shadow_quads: usize,
}

pub(crate) fn engine_stats_slot() -> &'static Mutex<EngineStatsSnapshot> {
    ENGINE_STATS.get_or_init(|| Mutex::new(EngineStatsSnapshot::default()))
}

/// The engine's own performance counters, as JSON.
///
/// Reports the engine's update-loop cost, which is what a benchmark needs:
/// a browser pins `requestAnimationFrame` to the display refresh, so a
/// JS-side timer cannot tell a lightly-loaded engine from a nearly saturated
/// one. This can.
#[wasm_bindgen]
pub fn engine_stats() -> String {
    let snapshot = engine_stats_slot()
        .lock()
        .map(|stats| *stats)
        .unwrap_or_default();
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string())
}

/// Every retained frame's real duration, as JSON, oldest first.
///
/// Unlike `engine_stats`, which reports a smoothed average, this is the raw
/// per-frame series — the only form in which a one-frame stall survives.
/// See `plugins/frame_trace.rs`.
#[wasm_bindgen]
pub fn frame_trace() -> String {
    plugins::frame_trace_json()
}

/// Empties the frame trace. Call right before the thing being measured, so
/// the retained window contains it and nothing else.
#[wasm_bindgen]
pub fn clear_frame_trace() {
    plugins::clear_frame_trace();
}

fn event_callback_slot() -> &'static Mutex<Option<Function>> {
    EVENT_CALLBACK.get_or_init(|| Mutex::new(None))
}

fn external_command_queue() -> &'static Mutex<Vec<ExternalCommand>> {
    EXTERNAL_COMMANDS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn emit_event(event: Value) {
    let event_text = event.to_string();

    if let Ok(callback_guard) = event_callback_slot().lock()
        && let Some(callback) = callback_guard.as_ref()
    {
        let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&event_text));
    }
}

fn parse_command(input: &str) -> Option<ExternalCommand> {
    let value: Value = serde_json::from_str(input).ok()?;
    let command_type = value.get("type")?.as_str()?;

    match command_type {
        "set_world" => Some(ExternalCommand::SetWorld {
            world_id: value.get("worldId")?.as_str()?.to_owned(),
        }),
        "upsert_token" => {
            let token_value = value.get("token")?.clone();
            let token: WorldTokenPayload = serde_json::from_value(token_value).ok()?;
            Some(ExternalCommand::UpsertToken { token })
        }
        "remove_token" => Some(ExternalCommand::RemoveToken {
            token_id: value.get("tokenId")?.as_str()?.to_owned(),
        }),
        "upsert_wall" => {
            let wall_value = value.get("wall")?.clone();
            let wall: WorldWallPayload = serde_json::from_value(wall_value).ok()?;
            Some(ExternalCommand::UpsertWall { wall })
        }
        "remove_wall" => Some(ExternalCommand::RemoveWall {
            wall_id: value.get("wallId")?.as_str()?.to_owned(),
        }),
        "upsert_light" => {
            let light_value = value.get("light")?.clone();
            let light: WorldLightPayload = serde_json::from_value(light_value).ok()?;
            Some(ExternalCommand::UpsertLight { light })
        }
        "remove_light" => Some(ExternalCommand::RemoveLight {
            light_id: value.get("lightId")?.as_str()?.to_owned(),
        }),
        "upsert_shape" => {
            let shape_value = value.get("shape")?.clone();
            let shape: WorldShapePayload = serde_json::from_value(shape_value).ok()?;
            Some(ExternalCommand::UpsertShape { shape })
        }
        "remove_shape" => Some(ExternalCommand::RemoveShape {
            shape_id: value.get("shapeId")?.as_str()?.to_owned(),
        }),
        "set_scene_background" => {
            let path = match value.get("backgroundImagePath") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str()?.to_owned()),
            };
            let width = value.get("width")?.as_f64()? as f32;
            let height = value.get("height")?.as_f64()? as f32;
            Some(ExternalCommand::SetSceneBackground {
                path,
                width,
                height,
            })
        }
        "set_scene_grid" => Some(ExternalCommand::SetSceneGrid {
            grid_type: value
                .get("gridType")
                .and_then(Value::as_str)
                .unwrap_or("square")
                .to_owned(),
            size: value.get("size")?.as_f64()? as f32,
            map_size: match (
                value.get("mapWidth").and_then(Value::as_f64),
                value.get("mapHeight").and_then(Value::as_f64),
            ) {
                (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Vec2::new(w as f32, h as f32)),
                _ => None,
            },
            origin_x: value.get("originX").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            origin_y: value.get("originY").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            visible: value
                .get("visible")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }),
        "set_token_grid" => Some(ExternalCommand::SetTokenGrid {
            token_id: value.get("tokenId")?.as_str()?.to_owned(),
            footprint: value
                .get("footprint")
                .and_then(Value::as_f64)
                .unwrap_or(1.0) as f32,
            snap: value.get("snap").and_then(Value::as_bool).unwrap_or(true),
        }),
        "set_grid_units" => Some(ExternalCommand::SetGridUnits {
            per_cell: value.get("perCell").and_then(Value::as_f64).unwrap_or(5.0) as f32,
            label: value
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("ft")
                .to_owned(),
        }),
        "set_grid_snap" => Some(ExternalCommand::SetGridSnap {
            enabled: value.get("enabled")?.as_bool()?,
        }),
        "set_camera" => Some(ExternalCommand::SetCamera {
            x: value.get("x").and_then(Value::as_f64).map(|v| v as f32),
            y: value.get("y").and_then(Value::as_f64).map(|v| v as f32),
            zoom: value.get("zoom").and_then(Value::as_f64).map(|v| v as f32),
        }),
        "fit_camera_to" => Some(ExternalCommand::FitCameraTo {
            center_x: value.get("centerX").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            center_y: value.get("centerY").and_then(Value::as_f64).unwrap_or(0.0) as f32,
            width: value.get("width")?.as_f64()? as f32,
            height: value.get("height")?.as_f64()? as f32,
        }),
        "set_token_vision" => Some(ExternalCommand::SetTokenVision {
            token_id: value.get("tokenId")?.as_str()?.to_owned(),
            darkvision: value
                .get("darkvision")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            facing: value
                .get("facing")
                .and_then(Value::as_f64)
                .map(|f| f as f32),
            fov: value
                .get("fov")
                .and_then(Value::as_f64)
                .unwrap_or(std::f64::consts::TAU) as f32,
            max_range: value
                .get("maxRange")
                .and_then(Value::as_f64)
                .map(|f| f as f32),
        }),
        "set_ambient_light" => Some(ExternalCommand::SetAmbientLight {
            level: value
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or("bright")
                .to_owned(),
            color: value
                .get("color")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        "set_lighting_overlay" => Some(ExternalCommand::SetLightingOverlay {
            enabled: value.get("enabled")?.as_bool()?,
        }),
        "set_render_probe" => Some(ExternalCommand::SetRenderProbe {
            enabled: value.get("enabled")?.as_bool()?,
        }),
        "set_is_game_master" => Some(ExternalCommand::SetIsGameMaster {
            is_game_master: value.get("isGameMaster")?.as_bool()?,
        }),
        "upsert_canvas_image_asset" => Some(ExternalCommand::UpsertCanvasImageAsset {
            asset_id: value.get("assetId")?.as_str()?.to_owned(),
            path: value.get("path")?.as_str()?.to_owned(),
            x: value.get("x")?.as_f64()? as f32,
            y: value.get("y")?.as_f64()? as f32,
            width: value.get("width")?.as_f64()? as f32,
            height: value.get("height")?.as_f64()? as f32,
        }),
        "remove_canvas_image_asset" => Some(ExternalCommand::RemoveCanvasImageAsset {
            asset_id: value.get("assetId")?.as_str()?.to_owned(),
        }),
        "trigger_dice_roll" => {
            let dice_value = value.get("dice")?.clone();
            let dice: Vec<DiceRollDiePayload> = serde_json::from_value(dice_value).ok()?;
            Some(ExternalCommand::TriggerDiceRoll { dice })
        }
        _ => None,
    }
}

#[wasm_bindgen]
pub fn set_event_callback(callback: Function) {
    if let Ok(mut callback_guard) = event_callback_slot().lock() {
        *callback_guard = Some(callback);
    }
}

#[wasm_bindgen]
pub fn apply_world_command(json_command: &str) {
    if let Some(command) = parse_command(json_command)
        && let Ok(mut queue) = external_command_queue().lock()
    {
        queue.push(command);
    }
}

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
        .insert_resource(SystemHooksRegistry { hooks: None })
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
        // Phase 4.7.F2: System Registration & Plugin Setup
        .add_plugins(SystemRegistrationPlugin)
        // Phase 4.7: Canvas Rendering Infrastructure
        .add_plugins(ScenePlugin)
        .add_plugins(GridPlugin)
        .add_plugins(TokenPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SelectionPlugin) // Phase 4.7.E1: Token Selection
        // Native canvas authoring (specs/001-bevy-canvas-authoring): shared
        // layer-ordering resource, must be added before Wall/Lighting/Shape
        // plugins so it exists when they build (Constitution Principle II)
        .add_plugins(CanvasLayerPlugin)
        // T015: wall authoring (specs/001-bevy-canvas-authoring). Depends
        // on CanvasLayerPlugin (above) for the `CanvasLayers` resource.
        .add_plugins(WallPlugin)
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
        .add_systems(Update, (calculate_derived_stats, calculate_ability_stats))
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

fn setup_scene(mut commands: Commands, mut token_entities: ResMut<TokenEntities>) {
    // NO camera is spawned here. `CameraPlugin` (plugins/camera.rs) owns the
    // one and only camera — it is the one `CameraManager` drives for pan and
    // zoom. This function used to spawn a second `Camera2d` as well, leaving
    // two active cameras with the same order (0) on the same render target.
    // Bevy warned about that every frame ("Camera order ambiguities
    // detected ..."), and the consequence is not cosmetic: each camera
    // clears the target on its own pass, so with an undefined order between
    // them one pass can wipe the other's output.
    //
    // The warning was invisible until `bevy_log` was added to this crate's
    // features; see the note there. Removing the duplicate silences it and
    // leaves exactly one active camera.
    //
    // This was a real bug but not the cause of the "canvas renders nothing
    // but the clear colour" symptom — that was the missing `*_render`
    // features in Cargo.toml. Both are fixed; they were independent.
    let player_entity = commands
        .spawn((
            Sprite::from_color(Color::srgb(0.851, 0.278, 0.306), TOKEN_SIZE),
            Transform::from_xyz(-180.0, 0.0, 0.0),
            PlayerToken,
            TokenIdentity("player".to_string()),
            PlayerControlled,
        ))
        .id();

    token_entities.0.insert("player".to_string(), player_entity);

    let npc_entity = commands
        .spawn((
            Sprite::from_color(Color::srgb(0.282, 0.565, 0.996), TOKEN_SIZE),
            Transform::from_xyz(180.0, 0.0, 0.0),
            TokenIdentity("npc".to_string()),
        ))
        .id();

    token_entities.0.insert("npc".to_string(), npc_entity);

}

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut Transform, With<PlayerToken>>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }

    if direction == Vec2::ZERO {
        return;
    }

    let delta = direction.normalize() * PLAYER_SPEED * time.delta_secs();
    let half_bounds = Vec2::new(ARENA_WIDTH / 2.0, ARENA_HEIGHT / 2.0) - (TOKEN_SIZE / 2.0);
    let translation = &mut player.translation;

    translation.x = (translation.x + delta.x).clamp(-half_bounds.x, half_bounds.x);
    translation.y = (translation.y + delta.y).clamp(-half_bounds.y, half_bounds.y);
}

fn emit_player_state(
    mut last_sent: ResMut<LastPlayerSent>,
    active_world: Res<ActiveWorld>,
    player: Single<(&Transform, &TokenIdentity), With<PlayerToken>>,
) {
    let (transform, token_identity) = *player;
    let current = transform.translation.truncate();

    if current.distance(last_sent.0) < 0.5 {
        return;
    }

    last_sent.0 = current;

    emit_event(json!({
        "type": "upsert_token",
        "token": {
            "id": token_identity.0,
            "x": transform.translation.x,
            "y": transform.translation.y,
            "z": transform.translation.z,
            "label": "Player"
        },
        "worldId": active_world.0,
    }));
}

/// Scene-level resources the command loop writes to.
///
/// Grouped into one `SystemParam` because Bevy caps a system at 16 parameters
/// and this loop had reached it. Every field is `Option` for the same reason
/// the loose parameters were: each belongs to a plugin that may not be
/// registered, and a command for an absent plugin is dropped rather than
/// panicking (Constitution Principle II).
#[derive(bevy::ecs::system::SystemParam)]
struct SceneParams<'w, 's> {
    grid: Option<ResMut<'w, SceneGrid>>,
    grid_visible: Option<ResMut<'w, GridVisible>>,
    ambient: Option<ResMut<'w, SceneAmbient>>,
    lighting_overlay: Option<ResMut<'w, LightingOverlay>>,
    camera: Option<ResMut<'w, CameraManager>>,
    grid_snap: Option<ResMut<'w, GridSnapEnabled>>,
    units: Option<ResMut<'w, crate::systems::token_move::SceneUnits>>,
    camera_viewport: Query<'w, 's, &'static Camera, With<Camera2d>>,
}

fn apply_external_commands(
    mut commands: Commands,
    mut active_world: ResMut<ActiveWorld>,
    mut token_entities: ResMut<TokenEntities>,
    mut token_query: Query<(Entity, &mut Transform, &TokenIdentity, &mut Sprite)>,
    // `WallSet` only exists once `WallPlugin` is registered (Constitution
    // Principle II: plugins are independently addable) — `Option` so this
    // core command loop degrades gracefully (wall commands are simply
    // dropped) if the wall plugin isn't present.
    wall_set: Option<ResMut<WallSet>>,
    // Same rationale as `wall_set`, for `LightingPlugin`/`LightSet`.
    light_set: Option<ResMut<LightSet>>,
    // `ShapeSet` only exists once `ShapePlugin` is registered, same
    // graceful-degradation rationale as `wall_set` above.
    shape_set: Option<ResMut<ShapeSet>>,
    // `SceneBackground` only exists once `BackgroundPlugin` is registered,
    // same graceful-degradation rationale as `wall_set` above.
    background: Option<ResMut<SceneBackground>>,
    // `PlacedCanvasImages` only exists once `BackgroundPlugin` is
    // registered (spec 002 added it alongside `SceneBackground` in that
    // same plugin), same graceful-degradation rationale as `wall_set`.
    placed_canvas_images: Option<ResMut<PlacedCanvasImages>>,
    // `IsGameMaster` exists once either `WallPlugin` or `ShapePlugin` is
    // registered (both `init_resource` it idempotently) — same
    // graceful-degradation rationale as `wall_set` above.
    is_game_master: Option<ResMut<IsGameMaster>>,
    // `RenderProbeEnabled` only exists once `RenderProbePlugin` is
    // registered, same graceful-degradation rationale as `wall_set` above.
    mut render_probe: Option<ResMut<RenderProbeEnabled>>,
    mut scene: SceneParams,
    // `PendingDiceRoll` only exists once `DiceRollPlugin` is registered,
    // same graceful-degradation rationale as `wall_set` above.
    pending_dice_roll: Option<ResMut<plugins::dice_roll::PendingDiceRoll>>,
    // For token art (`upsert_token`'s optional `image`). Not `Option`: the
    // asset server is part of `DefaultPlugins`, not a plugin this crate can
    // choose to leave out.
    asset_server: Res<AssetServer>,
) {
    let drained = if let Ok(mut queue) = external_command_queue().lock() {
        queue.drain(..).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut wall_set = wall_set;
    let mut light_set = light_set;
    let mut shape_set = shape_set;
    let mut background = background;
    let mut placed_canvas_images = placed_canvas_images;
    let mut is_game_master = is_game_master;
    let mut pending_dice_roll = pending_dice_roll;

    for command in drained {
        match command {
            ExternalCommand::SetWorld { world_id } => {
                active_world.0 = world_id;
            }
            ExternalCommand::UpsertToken { token } => {
                if let Some(existing_entity) = token_entities.0.get(&token.id).copied() {
                    if let Ok((_, mut transform, _, mut sprite)) =
                        token_query.get_mut(existing_entity)
                    {
                        transform.translation.x = token.x;
                        transform.translation.y = token.y;
                        transform.translation.z = token.z;
                        // Spec 004 (US2): apply scale/rotation only when
                        // present — `None` leaves the entity's current
                        // Transform.scale/rotation untouched, matching the
                        // "don't touch what wasn't sent" partial-update
                        // semantics `WorldTokenPayload`'s doc comment
                        // describes.
                        if let Some(scale) = token.scale {
                            transform.scale = Vec3::splat(scale);
                        }
                        if let Some(rotation) = token.rotation {
                            transform.rotation = Quat::from_rotation_z(rotation);
                        }

                        // Same partial-update rule for the art: `None`
                        // leaves whatever the token already shows. Guarded
                        // on the handle rather than assigned outright,
                        // because `Sprite` is change-detected and assigning
                        // an identical handle would re-extract the token to
                        // the render world for nothing. Clearing the size
                        // hands it back to `size_tokens_to_grid`, which
                        // re-fits it once the new art's dimensions are
                        // known — the old art's aspect must not stick.
                        if let Some(path) = token.photo_url.clone() {
                            let handle = asset_server.load(path);
                            if sprite.image != handle {
                                sprite.image = handle;
                                sprite.custom_size = None;
                            }
                        }
                    }
                    continue;
                }

                let mut transform = Transform::from_xyz(token.x, token.y, token.z);
                if let Some(scale) = token.scale {
                    transform.scale = Vec3::splat(scale);
                }
                if let Some(rotation) = token.rotation {
                    transform.rotation = Quat::from_rotation_z(rotation);
                }

                let sprite = match token.photo_url.clone() {
                    Some(path) => Sprite {
                        // Owned: `AssetServer::load` borrows for `'static`,
                        // and `token` is dropped at the end of this arm.
                        image: asset_server.load(path),
                        // Left for `size_tokens_to_grid` to set once the
                        // image's real dimensions are known. Guessing here
                        // would just be overwritten a frame later.
                        custom_size: None,
                        ..default()
                    },
                    None => Sprite::from_color(Color::srgb(0.282, 0.565, 0.996), TOKEN_SIZE),
                };

                let entity = commands
                    .spawn((sprite, transform, TokenIdentity(token.id.clone())))
                    .id();

                token_entities.0.insert(token.id, entity);
            }
            ExternalCommand::RemoveToken { token_id } => {
                if let Some(entity) = token_entities.0.remove(&token_id) {
                    commands.entity(entity).despawn();
                }
            }
            ExternalCommand::UpsertWall { wall } => {
                if let Some(wall_set) = wall_set.as_deref_mut() {
                    wall_set.upsert(EngineWall {
                        id: wall.id,
                        x1: wall.x1,
                        y1: wall.y1,
                        x2: wall.x2,
                        y2: wall.y2,
                        blocks_vision: wall.blocks_vision,
                        blocks_movement: wall.blocks_movement,
                        door_state: DoorState::from_str_loose(&wall.door_state),
                    });
                }
            }
            ExternalCommand::RemoveWall { wall_id } => {
                if let Some(wall_set) = wall_set.as_deref_mut() {
                    wall_set.remove(&wall_id);
                }
            }
            ExternalCommand::UpsertLight { light } => {
                if let Some(light_set) = light_set.as_deref_mut() {
                    light_set.upsert(EngineLight {
                        id: light.id,
                        x: light.x,
                        y: light.y,
                        radius: light.radius,
                        intensity: light.intensity,
                        color: light.color,
                        attached_token_id: light.attached_token_id,
                        casts_shadows: light.casts_shadows,
                    });
                }
            }
            ExternalCommand::RemoveLight { light_id } => {
                if let Some(light_set) = light_set.as_deref_mut() {
                    light_set.remove(&light_id);
                }
            }
            ExternalCommand::UpsertShape { shape } => {
                if let Some(shape_set) = shape_set.as_deref_mut() {
                    shape_set.upsert(EngineShape {
                        id: shape.id,
                        kind: ShapeKind::from_str_loose(&shape.kind),
                        geometry: shape.geometry,
                        text: shape.text,
                        style: shape.style,
                        visible_to_players: shape.visible_to_players,
                    });
                }
            }
            ExternalCommand::RemoveShape { shape_id } => {
                if let Some(shape_set) = shape_set.as_deref_mut() {
                    shape_set.remove(&shape_id);
                }
            }
            ExternalCommand::SetSceneBackground {
                path,
                width,
                height,
            } => {
                if let Some(background) = background.as_deref_mut() {
                    // Bug fix: writing through `ResMut::deref_mut` trips
                    // Bevy's change detection unconditionally, even when
                    // `path`/`width`/`height` are identical to the current
                    // value — every repeat dispatch of an already-applied
                    // background (WorldPage.tsx's effect can legitimately
                    // re-run with an equivalent `selectedScene` object,
                    // e.g. after an unrelated scene-list refetch) then made
                    // `sync_scene_background` (systems/background.rs) see
                    // `is_changed() == true` again, despawning the sprite
                    // and re-issuing `asset_server.load(&path)` — dropping
                    // the previous `Handle<Image>` cancels that in-flight
                    // load (found live: a real imported background's fetch
                    // reliably got `net::ERR_ABORTED` moments after
                    // starting, leaving Play's canvas permanently blank).
                    // Comparing first keeps the write — and the spurious
                    // respawn/reload cycle — from happening at all when
                    // nothing actually changed.
                    let unchanged = background.path == path
                        && background.width == width
                        && background.height == height;
                    if !unchanged {
                        background.path = path;
                        background.width = width;
                        background.height = height;
                    }
                }
            }
            ExternalCommand::SetIsGameMaster {
                is_game_master: value,
            } => {
                if let Some(is_game_master) = is_game_master.as_deref_mut() {
                    is_game_master.0 = value;
                }
            }
            ExternalCommand::SetSceneGrid {
                grid_type,
                size,
                map_size,
                origin_x,
                origin_y,
                visible,
            } => {
                if let Some(scene_grid) = scene.grid.as_deref_mut() {
                    *scene_grid = match map_size {
                        Some(map_size) => SceneGrid::anchored_to_map(&grid_type, size, map_size),
                        None => {
                            SceneGrid::from_server(&grid_type, size, Vec2::new(origin_x, origin_y))
                        }
                    };
                    info!(
                        target: "grid",
                        "grid: {:?} size={} origin={:?} visible={visible}",
                        scene_grid.kind,
                        scene_grid.size,
                        scene_grid.origin,
                    );
                }
                if let Some(grid_visible) = scene.grid_visible.as_deref_mut() {
                    grid_visible.0 = visible;
                }
            }
            ExternalCommand::SetTokenGrid {
                token_id,
                footprint,
                snap,
            } => {
                if let Some(&entity) = token_entities.0.get(&token_id) {
                    let behaviour = TokenGridBehaviour {
                        footprint: Footprint::new(footprint),
                        snap,
                    };
                    commands.entity(entity).insert(behaviour);
                    info!(
                        target: "grid",
                        "token {token_id}: {} cells, snap={snap}",
                        behaviour.footprint.cells(),
                    );
                } else {
                    warn!(target: "grid", "set_token_grid: no token {token_id}");
                }
            }
            ExternalCommand::SetGridUnits { per_cell, label } => {
                if let Some(units) = scene.units.as_deref_mut() {
                    units.0 = GridUnits::new(per_cell, label);
                    info!(target: "grid", "units: 1 cell = {}", units.format(1.0));
                }
            }
            ExternalCommand::SetGridSnap { enabled } => {
                if let Some(snap) = scene.grid_snap.as_deref_mut() {
                    snap.0 = enabled;
                    info!(target: "grid", "grid snapping {}", if enabled { "on" } else { "off" });
                }
            }
            ExternalCommand::SetCamera { x, y, zoom } => {
                if let Some(camera_mgr) = scene.camera.as_deref_mut() {
                    if let Some(x) = x {
                        camera_mgr.translation.x = x;
                    }
                    if let Some(y) = y {
                        camera_mgr.translation.y = y;
                    }
                    if let Some(zoom) = zoom {
                        camera_mgr.set_zoom(zoom);
                    }
                }
            }
            ExternalCommand::FitCameraTo {
                center_x,
                center_y,
                width,
                height,
            } => {
                if let Some(camera_mgr) = scene.camera.as_deref_mut() {
                    // The viewport in *world units at 1:1* is just its pixel
                    // size, since one world unit is one pixel at scale 1.
                    let viewport = scene
                        .camera_viewport
                        .single()
                        .ok()
                        .and_then(|camera| camera.logical_viewport_size())
                        .unwrap_or(Vec2::new(1280.0, 720.0));
                    camera_mgr.fit_to(
                        Vec2::new(center_x, center_y),
                        Vec2::new(width, height),
                        viewport,
                    );
                    info!(
                        target: "camera",
                        "fit {width}x{height} into {viewport:?} -> zoom {}",
                        camera_mgr.scale,
                    );
                }
            }
            ExternalCommand::SetTokenVision {
                token_id,
                darkvision,
                facing,
                fov,
                max_range,
            } => {
                if let Some(&entity) = token_entities.0.get(&token_id) {
                    commands.entity(entity).insert(TokenVision(VisionProfile {
                        darkvision,
                        facing,
                        fov,
                        max_range,
                    }));
                    info!(
                        target: "lighting",
                        "vision: {token_id} darkvision={darkvision} facing={facing:?} fov={fov}",
                    );
                } else {
                    // Worth saying rather than dropping: a mistyped id would
                    // otherwise look like the vision setting simply had no
                    // effect.
                    warn!(target: "lighting", "set_token_vision: no token {token_id}");
                }
            }
            ExternalCommand::SetAmbientLight { level, color } => {
                if let Some(ambient) = scene.ambient.as_deref_mut() {
                    ambient.level = match level.trim().to_ascii_lowercase().as_str() {
                        "dark" | "dark_ness" | "darkness" | "unlit" => Illumination::Dark,
                        "dim" => Illumination::Dim,
                        // Unknown values read as bright rather than plunging a
                        // scene into darkness on a typo.
                        _ => Illumination::Bright,
                    };
                    ambient.color = color.as_deref().and_then(Rgb::parse_hex);
                    info!(target: "lighting", "ambient: {:?}", ambient.level);
                }
            }
            ExternalCommand::SetLightingOverlay { enabled } => {
                if let Some(overlay) = scene.lighting_overlay.as_deref_mut() {
                    overlay.0 = enabled;
                }
            }
            ExternalCommand::SetRenderProbe { enabled } => {
                if let Some(render_probe) = render_probe.as_deref_mut() {
                    render_probe.0 = enabled;
                    info!(
                        "render probe {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
            }
            ExternalCommand::UpsertCanvasImageAsset {
                asset_id,
                path,
                x,
                y,
                width,
                height,
            } => {
                if let Some(placed_canvas_images) = placed_canvas_images.as_deref_mut() {
                    // Same fix as `SetSceneBackground` above: skip the
                    // write (and the spurious despawn/respawn/reload it
                    // would trigger in `sync_placed_canvas_images`) when a
                    // repeat dispatch carries an identical value.
                    let new_image = PlacedCanvasImage {
                        path,
                        x,
                        y,
                        width,
                        height,
                    };
                    if placed_canvas_images.0.get(&asset_id) != Some(&new_image) {
                        placed_canvas_images.0.insert(asset_id, new_image);
                    }
                }
            }
            ExternalCommand::RemoveCanvasImageAsset { asset_id } => {
                if let Some(placed_canvas_images) = placed_canvas_images.as_deref_mut() {
                    placed_canvas_images.0.remove(&asset_id);
                }
            }
            ExternalCommand::TriggerDiceRoll { dice } => {
                if let Some(pending_dice_roll) = pending_dice_roll.as_deref_mut() {
                    pending_dice_roll.0 = Some(
                        dice.into_iter()
                            .map(|d| plugins::dice_roll::DiceRollDie {
                                final_value: d.final_value,
                            })
                            .collect(),
                    );
                }
            }
        }
    }
}
