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

use movement::{PlayerControlled, handle_keyboard_movement, sync_grid_to_transform, sync_transform_to_grid, apply_grid_snapping};
use derived_data::*;
use sync_test::*;
use systems::*;

static ENGINE_STARTED: AtomicBool = AtomicBool::new(false);
static EVENT_CALLBACK: OnceLock<Mutex<Option<Function>>> = OnceLock::new();
static EXTERNAL_COMMANDS: OnceLock<Mutex<Vec<ExternalCommand>>> = OnceLock::new();

const ARENA_WIDTH: f32 = 1280.0;
const ARENA_HEIGHT: f32 = 720.0;
const PLAYER_SPEED: f32 = 320.0;
const TOKEN_SIZE: Vec2 = Vec2::new(96.0, 96.0);

#[derive(Component)]
struct PlayerToken;

#[derive(Component)]
struct TokenIdentity(String);

#[derive(Resource, Default)]
struct ActiveWorld(String);

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

#[derive(Debug, Clone)]
enum ExternalCommand {
    SetWorld { world_id: String },
    UpsertToken { token: WorldTokenPayload },
    RemoveToken { token_id: String },
}

fn event_callback_slot() -> &'static Mutex<Option<Function>> {
    EVENT_CALLBACK.get_or_init(|| Mutex::new(None))
}

fn external_command_queue() -> &'static Mutex<Vec<ExternalCommand>> {
    EXTERNAL_COMMANDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn emit_event(event: Value) {
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

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.133, 0.157, 0.192)))
        .insert_resource(ActiveWorld("default".to_string()))
        .insert_resource(TokenEntities::default())
        .insert_resource(LastPlayerSent(Vec2::new(f32::MIN, f32::MIN)))
        .insert_resource(GridConfig::default())
        .insert_resource(network::GraphQLClient::new("http://localhost:8080".to_string()))
        .insert_resource(network::WorldEventSubscription::new())
        .insert_resource(CircularFlowTracer::new())
        .insert_resource(SystemHooksRegistry { hooks: None })
        // PHASE 4.5: Add ServerEvent registration with proper Bevy event system
        // .add_event::<network::ServerEvent>()
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
                network::process_server_events,
                process_server_responses,
                handle_mutation_errors,
                trace_keyboard_input,
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
        Text::new("Bevy wasm: move red token with WASD or arrows. Changes sync to tldraw."),
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
) {
    let drained = if let Ok(mut queue) = external_command_queue().lock() {
        queue.drain(..).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

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
        }
    }
}
