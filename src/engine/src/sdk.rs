//! The `wasm_bindgen` surface: what the browser may call, and the command
//! parsing behind `applyWorldCommand`.

use super::*;

/// What the engine would draw for one token, as JSON, or `null`.
#[wasm_bindgen]
pub fn get_token_status(token_id: &str) -> String {
    token_status_slot()
        .lock()
        .ok()
        .and_then(|map| map.get(token_id).cloned())
        .and_then(|resources| serde_json::to_string(&resources).ok())
        .unwrap_or_else(|| "null".to_string())
}

/// Every token currently carrying status furniture, as JSON.
#[wasm_bindgen]
pub fn list_token_status() -> String {
    token_status_slot()
        .lock()
        .ok()
        .and_then(|map| serde_json::to_string(&*map).ok())
        .unwrap_or_else(|| "{}".to_string())
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

pub(crate) fn event_callback_slot() -> &'static Mutex<Option<Function>> {
    EVENT_CALLBACK.get_or_init(|| Mutex::new(None))
}

pub(crate) fn external_command_queue() -> &'static Mutex<Vec<ExternalCommand>> {
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

pub(crate) fn parse_command(input: &str) -> Option<ExternalCommand> {
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
        "set_token_status" => {
            let resources: Vec<StatusResourcePayload> =
                serde_json::from_value(value.get("resources")?.clone()).ok()?;
            Some(ExternalCommand::SetTokenStatus {
                token_id: value.get("tokenId")?.as_str()?.to_owned(),
                resources,
            })
        }
        "clear_token_status" => Some(ExternalCommand::ClearTokenStatus {
            token_id: value.get("tokenId")?.as_str()?.to_owned(),
        }),
        "set_display_appearance" => {
            // An absent `appearance` is an empty override, not an error: it
            // is a no-op, and treating it as malformed would report a fault
            // for a command that asked for nothing.
            let raw = value
                .get("appearance")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let override_values: AppearanceOverride = serde_json::from_value(raw).ok()?;
            Some(ExternalCommand::SetDisplayAppearance { override_values })
        }
        "upsert_interactive" => {
            let interactive: InteractivePayload =
                serde_json::from_value(value.get("interactive")?.clone()).ok()?;
            Some(ExternalCommand::UpsertInteractive { interactive })
        }
        "remove_interactive" => Some(ExternalCommand::RemoveInteractive {
            interactive_id: value.get("interactiveId")?.as_str()?.to_owned(),
        }),
        "dispatch_interaction" => Some(ExternalCommand::DispatchInteraction {
            interactive_id: value.get("interactiveId")?.as_str()?.to_owned(),
            effect_id: value.get("effectId")?.as_str()?.to_owned(),
            config: value.get("effectConfig").cloned().unwrap_or(Value::Null),
        }),
        "set_scene_playing" => Some(ExternalCommand::SetScenePlaying {
            playing: value.get("playing")?.as_bool()?,
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
    // A command that cannot be understood is now *reported*, where it used to
    // be dropped.
    //
    // Silent discard is the failure mode this boundary is being hardened
    // against (spec 029 FR-020): the engine deserialized what it recognised
    // and ignored the rest, so a renamed or mistyped field produced a display
    // that never appeared — with no error, no warning, and nothing to attach a
    // debugger to. Three bugs in this feature alone had that shape.
    match classify_command(json_command) {
        Ok(command) => {
            if let Ok(mut queue) = external_command_queue().lock() {
                queue.push(command);
            }
        }
        Err(error) => emit_event(serde_json::json!({
            "type": "sdkError",
            "code": error.code,
            "message": error.message,
            "command": error.command,
        })),
    }
}

/// Turn the server's spelling of an interactive into the engine's.
///
/// The one place `door` becomes a segment of map geometry. That mapping lives
/// here rather than in `plugins::interaction` on purpose: the plugin is
/// required to know nothing about what a subject means, and this boundary is
/// where the server's vocabulary stops.
pub(crate) fn to_engine_interactive(
    payload: InteractivePayload,
) -> plugins::interaction::Interactive {
    use plugins::interaction::Subject;

    plugins::interaction::Interactive {
        id: payload.id,
        subject: match payload.subject_kind.as_str() {
            "region" => Subject::Region,
            "prop" => Subject::Prop,
            // Anything attached to the scene's line geometry. An unrecognised
            // kind lands here rather than being dropped, because a subject the
            // engine cannot categorise is still something a Game Master
            // placed.
            _ => Subject::Segment,
        },
        subject_ref: payload.subject_ref,
        geometry: payload
            .geometry
            .and_then(|g| serde_json::from_value(g).ok()),
        effect_id: payload.effect_id,
        config: payload.effect_config.unwrap_or(Value::Null),
        on_entry: payload.trigger == "enter",
        can_activate: payload.can_activate,
        once: payload.fire_mode.as_deref() == Some("once"),
        fired: payload.fired,
    }
}

/// Every interactive the engine currently holds, as JSON.
///
/// Read-only, like `get_token_status`. A debugging or observation surface that
/// can also mutate state becomes a way to write tests that pass against
/// situations the application cannot reach.
#[wasm_bindgen]
pub fn list_interactives() -> String {
    interactive_snapshot_slot()
        .lock()
        .ok()
        .and_then(|snapshot| serde_json::to_string(&*snapshot).ok())
        .unwrap_or_else(|| "[]".to_string())
}

/// Every effect the engine has dispatched, in order, as JSON.
///
/// The seam's own observation point. A contributor's *result* is visible on
/// the canvas; what this shows is that dispatch happened at all — which is the
/// difference an end-to-end test needs when a contributor is deliberately
/// absent (US7).
#[wasm_bindgen]
pub fn dispatched_effects() -> String {
    dispatched_effects_slot()
        .lock()
        .ok()
        .and_then(|log| serde_json::to_string(&*log).ok())
        .unwrap_or_else(|| "[]".to_string())
}

/// Which walls the engine is currently drawing, as JSON.
///
/// Read-only observation. It exists because "a player is not shown a secret
/// door" is a claim about drawing and nothing else can answer it: the geometry
/// is deliberately sent to every client (a wall that did not arrive would also
/// stop blocking vision), so a payload check would prove the opposite of what
/// is wanted.
#[wasm_bindgen]
pub fn drawn_wall_ids() -> String {
    drawn_walls_slot()
        .lock()
        .ok()
        .and_then(|walls| serde_json::to_string(&*walls).ok())
        .unwrap_or_else(|| "[]".to_string())
}

pub(crate) static DRAWN_WALLS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub(crate) fn drawn_walls_slot() -> &'static Mutex<Vec<String>> {
    DRAWN_WALLS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) static INTERACTIVE_SNAPSHOT: OnceLock<Mutex<Vec<Value>>> = OnceLock::new();
pub(crate) static DISPATCHED_EFFECTS: OnceLock<Mutex<Vec<Value>>> = OnceLock::new();

pub(crate) fn interactive_snapshot_slot() -> &'static Mutex<Vec<Value>> {
    INTERACTIVE_SNAPSHOT.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn dispatched_effects_slot() -> &'static Mutex<Vec<Value>> {
    DISPATCHED_EFFECTS.get_or_init(|| Mutex::new(Vec::new()))
}

/// The SDK contract version both sides ship under.
///
/// A single integer, deliberately. The engine and the application are built
/// into one bundle, so there is no independent release cadence for semantic
/// versioning to describe — this exists to catch a *stale* bundle, which is
/// precisely the case the old boundary failed silently on.
pub const SDK_VERSION: u32 = 1;

/// Why a command could not be accepted.
pub(crate) struct SdkError {
    code: &'static str,
    message: String,
    command: Option<String>,
}

/// Parse and version-check one command.
pub(crate) fn classify_command(input: &str) -> Result<ExternalCommand, SdkError> {
    let value: Value = serde_json::from_str(input).map_err(|e| SdkError {
        code: "malformed",
        message: format!("Command is not valid JSON: {e}"),
        command: None,
    })?;

    let command_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .map(str::to_owned);

    // Version is optional so every existing caller keeps working; when it is
    // given and disagrees, nothing is applied. Partial application of a
    // command from a bundle that does not share this contract is worse than
    // refusing it, because the half that succeeded is invisible.
    if let Some(declared) = value.get("sdkVersion").and_then(|v| v.as_u64())
        && declared != u64::from(SDK_VERSION)
    {
        return Err(SdkError {
            code: "versionMismatch",
            message: format!(
                "Command declares SDK version {declared}; this engine speaks {SDK_VERSION}. \
                 Nothing was applied."
            ),
            command: command_type,
        });
    }

    parse_command(input).ok_or_else(|| SdkError {
        code: "malformed",
        message: match &command_type {
            Some(name) => format!(
                "Command {name:?} could not be read — a field is missing or has the wrong type."
            ),
            None => "Command has no `type`.".to_string(),
        },
        command: command_type,
    })
}
