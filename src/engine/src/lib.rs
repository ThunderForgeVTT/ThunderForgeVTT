use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin, WindowResolution};
use js_sys::Function;
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

// Public module exports for Phase 4.2/4.3 Bevy integration
pub mod components;
pub mod network;
pub mod movement;
pub mod derived_data;
pub mod sync_test;
pub mod systems;

// Phase 4.7: Canvas Rendering Infrastructure
pub mod plugins;
pub mod resources;
pub mod transforms;
pub mod grid;

// Phase 4.7.G2: Integration & E2E Tests
mod integration_tests;

use movement::{PlayerControlled, handle_keyboard_movement, sync_grid_to_transform, sync_transform_to_grid, apply_grid_snapping};
use derived_data::*;
use sync_test::*;
use systems::*;
use plugins::{ScenePlugin, GridPlugin, TokenPlugin, CameraPlugin, SelectionPlugin, SystemRegistrationPlugin, CanvasLayerPlugin, WallPlugin, LightingPlugin, ShapePlugin, BackgroundPlugin};
use resources::{DoorState, Wall as EngineWall, WallSet, LightSource as EngineLight, LightSet, Shape as EngineShape, ShapeKind, ShapeSet, SceneBackground};

static ENGINE_STARTED: AtomicBool = AtomicBool::new(false);
static EVENT_CALLBACK: OnceLock<Mutex<Option<Function>>> = OnceLock::new();
static EXTERNAL_COMMANDS: OnceLock<Mutex<Vec<ExternalCommand>>> = OnceLock::new();

const ARENA_WIDTH: f32 = 1280.0;
const ARENA_HEIGHT: f32 = 720.0;
const PLAYER_SPEED: f32 = 320.0;
pub(crate) const TOKEN_SIZE: Vec2 = Vec2::new(96.0, 96.0);

#[derive(Component)]
struct PlayerToken;

#[derive(Component)]
pub(crate) struct TokenIdentity(pub(crate) String);

#[derive(Resource, Default)]
pub(crate) struct ActiveWorld(pub(crate) String);

#[derive(Resource, Default)]
struct TokenEntities(HashMap<String, Entity>);

#[derive(Resource)]
struct LastPlayerSent(Vec2);

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
    label: Option<String>,
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
    SetWorld { world_id: String },
    UpsertToken { token: WorldTokenPayload },
    RemoveToken { token_id: String },
    UpsertWall { wall: WorldWallPayload },
    RemoveWall { wall_id: String },
    UpsertLight { light: WorldLightPayload },
    RemoveLight { light_id: String },
    UpsertShape { shape: WorldShapePayload },
    RemoveShape { shape_id: String },
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
            Some(ExternalCommand::SetSceneBackground { path, width, height })
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
        .insert_resource(network::GraphQLClient::new("http://localhost:8080".to_string()))
        .insert_resource(network::websocket::WebSocketSubscription::new())
        .insert_resource(network::WorldEventSubscription::new())
        .insert_resource(tracker)
        .insert_resource(CircularFlowTracer::new())
        .insert_resource(SystemHooksRegistry { hooks: None })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some(canvas_selector.to_owned()),
                fit_canvas_to_parent: true,
                focused: true,
                resolution: WindowResolution::new(ARENA_WIDTH as u32, ARENA_HEIGHT as u32),
                title: "ThunderForge Engine".into(),
                ..default()
            }),
            ..default()
        }))
        // Phase 4.7.F2: System Registration & Plugin Setup
        .add_plugins(SystemRegistrationPlugin)
        // Phase 4.7: Canvas Rendering Infrastructure
        .add_plugins(ScenePlugin)
        .add_plugins(GridPlugin)
        .add_plugins(TokenPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SelectionPlugin)  // Phase 4.7.E1: Token Selection
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
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                apply_external_commands,
                move_player,
                emit_player_state,
            ),
        )
        .add_systems(
            Update,
            (
                handle_keyboard_movement,
                sync_grid_to_transform,
                sync_transform_to_grid,
                apply_grid_snapping,
            ),
        )
        .add_systems(
            Update,
            (
                calculate_derived_stats,
                calculate_ability_stats,
            ),
        )
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
    commands.spawn(Camera2d);

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

    commands.spawn((
        Text::new("Bevy wasm: move red token with WASD/arrows, or click-drag any token."),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            left: px(16),
            ..default()
        },
    ));
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

fn apply_external_commands(
    mut commands: Commands,
    mut active_world: ResMut<ActiveWorld>,
    mut token_entities: ResMut<TokenEntities>,
    mut token_query: Query<(Entity, &mut Transform, &TokenIdentity)>,
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

    for command in drained {
        match command {
            ExternalCommand::SetWorld { world_id } => {
                active_world.0 = world_id;
            }
            ExternalCommand::UpsertToken { token } => {
                if let Some(existing_entity) = token_entities.0.get(&token.id).copied() {
                    if let Ok((_, mut transform, _)) = token_query.get_mut(existing_entity) {
                        transform.translation.x = token.x;
                        transform.translation.y = token.y;
                        transform.translation.z = token.z;
                    }
                    continue;
                }

                let entity = commands
                    .spawn((
                        Sprite::from_color(Color::srgb(0.282, 0.565, 0.996), TOKEN_SIZE),
                        Transform::from_xyz(token.x, token.y, token.z),
                        TokenIdentity(token.id.clone()),
                    ))
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
            ExternalCommand::SetSceneBackground { path, width, height } => {
                if let Some(background) = background.as_deref_mut() {
                    background.path = path;
                    background.width = width;
                    background.height = height;
                }
            }
        }
    }
}
