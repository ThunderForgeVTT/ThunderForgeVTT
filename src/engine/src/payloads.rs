//! The engine's own state slots, its marker components, and every JSON
//! payload the host sends in or reads out.

use super::*;

pub(crate) static ENGINE_STARTED: AtomicBool = AtomicBool::new(false);
pub(crate) static EVENT_CALLBACK: OnceLock<Mutex<Option<Function>>> = OnceLock::new();
pub(crate) static EXTERNAL_COMMANDS: OnceLock<Mutex<Vec<ExternalCommand>>> = OnceLock::new();
/// Latest engine performance counters, mirrored out of the ECS.
///
/// A mirror rather than a direct query because `App::run()` takes ownership of
/// its `World` and never returns on wasm — there is no handle left to read
/// from. A system inside the schedule writes here each frame, and
/// `engine_stats()` below reads it.
pub(crate) static ENGINE_STATS: OnceLock<Mutex<EngineStatsSnapshot>> = OnceLock::new();

pub(crate) const ARENA_WIDTH: f32 = 1280.0;
pub(crate) const ARENA_HEIGHT: f32 = 720.0;
pub(crate) const PLAYER_SPEED: f32 = 320.0;
pub(crate) const TOKEN_SIZE: Vec2 = Vec2::new(96.0, 96.0);

#[derive(Component)]
pub(crate) struct PlayerToken;

#[derive(Component)]
pub(crate) struct TokenIdentity(pub(crate) String);

#[derive(Resource, Default)]
pub(crate) struct ActiveWorld(pub(crate) String);

/// Which entity is drawing which token id.
///
/// `pub(crate)` so the scene-transition plugin can drop the whole map when a
/// scene is unloaded (spec 031 FR-018). Despawning the entities without
/// clearing this would leave the map pointing at dead entities, and the next
/// `upsert_token` for a reused id would update nothing.
#[derive(Resource, Default)]
pub(crate) struct TokenEntities(pub(crate) HashMap<String, Entity>);

impl TokenEntities {
    /// Forget every token id, for a scene that is no longer on the canvas.
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Resource)]
pub(crate) struct LastPlayerSent(pub(crate) Vec2);

// Fields aren't read yet — grid rendering doesn't consult this resource
// today, but the value is inserted at startup ahead of that wiring landing.
#[allow(dead_code)]
#[derive(Resource, Clone, Debug)]
pub(crate) struct GridConfig {
    pub(crate) grid_size: f32,
    pub(crate) grid_type: String, // "square" or "hex"
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
pub(crate) struct WorldTokenPayload {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    // Deserialized from the server payload but not yet read anywhere —
    // no nameplate/tooltip rendering consumes it yet.
    #[allow(dead_code)]
    pub(crate) label: Option<String>,
    // Spec 004 (US2): resize/rotate. Optional so the pre-existing
    // position-only `upsert_token` events (e.g. the WASD demo token, or a
    // plain drag that doesn't touch either) keep working unchanged —
    // `None` means "don't touch this token's current scale/rotation",
    // mirroring the server's own `TokenUpdate` partial-update semantics.
    #[serde(default)]
    pub(crate) rotation: Option<f32>,
    #[serde(default)]
    pub(crate) scale: Option<f32>,
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
    pub(crate) photo_url: Option<String>,
    /// What this token represents, deciding the colour it is drawn in when
    /// it has no art.
    ///
    /// Until this existed, a player character, a hostile NPC, the cart they
    /// were escorting and a barrel all rendered in the same blue, and the
    /// only way to tell them apart was to click one.
    ///
    /// `None` — an older payload — keeps the character colour, so nothing
    /// that worked before changes.
    #[serde(default, rename = "tokenType")]
    pub(crate) token_type: Option<String>,
    /// Current and maximum for the token's primary pool.
    ///
    /// The web client has been sending these since spec 004
    /// (`WorldToken.health` / `.maxHealth`) and this struct did not
    /// deserialize them, so they were dropped at the boundary — the same
    /// shape of gap as `photo_url` before it was wired, and as the `Token`
    /// component that was never attached.
    ///
    /// Spec 029 gives them a consumer: they populate `Token`, which
    /// `calculate_derived_stats` finally has input from.
    #[serde(default)]
    pub(crate) health: Option<i32>,
    #[serde(default, rename = "maxHealth")]
    pub(crate) max_health: Option<i32>,
    /// The actor's attribute scores, keyed by the system's own identifiers.
    ///
    /// Optional because most `upsert_token` events are positional and carry
    /// no sheet at all, and because a system may declare no attributes —
    /// both of which mean "leave this alone" rather than "clear it".
    #[serde(default)]
    pub(crate) attributes: Option<std::collections::BTreeMap<String, i32>>,
}

/// The colour a token is drawn in when it carries no art.
///
/// The kind and its appearance are decided together in
/// `thunderforge_canvas_core::token_kind`, where the palette is tested for
/// separation in lightness as well as hue — four colours that look distinct
/// to whoever picked them can collapse into two for a viewer with a red-green
/// deficiency, and a battle map is a bad place to discover that.
///
/// An unknown or absent kind falls back to `Character` rather than refusing
/// to draw: a token you cannot see is worse than a token wearing the wrong
/// colour, and the server already rejects unknown kinds on the way in, so
/// reaching this fallback means an older payload rather than bad data.
pub(crate) fn token_kind_color(token_type: &Option<String>) -> Color {
    let kind = token_type
        .as_deref()
        .and_then(TokenKind::from_stored)
        .unwrap_or_default();
    let (r, g, b) = kind.fill();
    Color::srgb(r, g, b)
}

/// One resource on one token, as the server resolved it for this viewer.
///
/// `disclosed` is the tagged union from `thunderforge-canvas-core`, so the
/// payload carries exactly the one field its state permits — an
/// over-disclosing message does not deserialize.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusResourcePayload {
    pub definition: ResourceDefinition,
    pub disclosed: Disclosed,
}

/// Confirmed/authoritative wall state from the server (T008), matching
/// the `upsert_wall` inbound command's `wall` payload shape.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorldWallPayload {
    pub(crate) id: String,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
    pub(crate) x2: f32,
    pub(crate) y2: f32,
    #[serde(rename = "blocksVision")]
    pub(crate) blocks_vision: bool,
    #[serde(rename = "blocksMovement")]
    pub(crate) blocks_movement: bool,
    #[serde(rename = "doorState")]
    pub(crate) door_state: String,
    /// Spec 030. Absent in an older payload, and `false` is what every wall
    /// was before this existed — so a stale sender keeps working rather than
    /// producing a wall the engine refuses.
    #[serde(default)]
    pub(crate) locked: bool,
    #[serde(default)]
    pub(crate) secret: bool,
}

/// Confirmed/authoritative light state from the server (T036-T040),
/// matching the `upsert_light` inbound command's `light` payload shape.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorldLightPayload {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) radius: f32,
    pub(crate) intensity: f32,
    pub(crate) color: Option<String>,
    #[serde(rename = "attachedTokenId")]
    pub(crate) attached_token_id: Option<String>,
    #[serde(rename = "castsShadows")]
    pub(crate) casts_shadows: bool,
}

/// Confirmed/authoritative shape state from the server (T053), matching
/// the `upsert_shape` inbound command's `shape` payload shape
/// (contracts/graphql.md's `geometry`/`style` blobs are opaque JSON, so
/// they're kept as raw `serde_json::Value` rather than typed fields).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WorldShapePayload {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) geometry: Value,
    pub(crate) text: Option<String>,
    pub(crate) style: Option<Value>,
    #[serde(rename = "visibleToPlayers")]
    pub(crate) visible_to_players: bool,
}

/// One interactive, on its way into the engine (spec 030).
///
/// `subjectKind` is the server's own spelling — `prop`, `door`, `region` —
/// mapped here to the engine's spatial categories. That mapping is the seam:
/// the interaction plugin knows a subject is a segment of map geometry and
/// deliberately does not know what a segment means in the fiction.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InteractivePayload {
    pub(crate) id: String,
    pub(crate) subject_kind: String,
    #[serde(default)]
    pub(crate) subject_ref: Option<String>,
    #[serde(default)]
    pub(crate) geometry: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) effect_id: Option<String>,
    #[serde(default)]
    pub(crate) effect_config: Option<serde_json::Value>,
    /// `click` or `enter`.
    pub(crate) trigger: String,
    #[serde(default)]
    pub(crate) can_activate: bool,
    #[serde(default)]
    pub(crate) fire_mode: Option<String>,
    #[serde(default)]
    pub(crate) fired: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum ExternalCommand {
    SetWorld {
        world_id: String,
    },
    UpsertToken {
        token: WorldTokenPayload,
    },
    RemoveToken {
        token_id: String,
    },
    /// Spec 029: the resolved, already-entitlement-filtered status for one
    /// token. The engine draws what it is given and cannot widen it — the
    /// coarsening happened on the server, and a value this viewer may not see
    /// never entered the process.
    SetTokenStatus {
        token_id: String,
        resources: Vec<StatusResourcePayload>,
    },
    ClearTokenStatus {
        token_id: String,
    },
    /// Spec 029 FR-022: presentation values come from the application.
    ///
    /// Carries an *override*, not a complete appearance, so an application
    /// that wants a different bar height does not have to restate the whole
    /// palette — and, more importantly, does not silently freeze the rest of
    /// the appearance at whatever the defaults happened to be on the day it
    /// was written.
    SetDisplayAppearance {
        override_values: AppearanceOverride,
    },
    /// Spec 030: one interactive on the active scene, as this viewer knows
    /// it. A player's payload carries less than a Game Master's; the engine
    /// treats an absent effect as "nothing to dispatch locally" rather than as
    /// missing data.
    UpsertInteractive {
        interactive: InteractivePayload,
    },
    RemoveInteractive {
        interactive_id: String,
    },
    /// Spec 030: run an effect the server has already permitted.
    ///
    /// The engine is not a second authority on whether it was allowed — this
    /// exists so the change is visible immediately rather than a round trip
    /// later (ADR-054).
    DispatchInteraction {
        interactive_id: String,
        effect_id: String,
        config: Value,
    },
    /// Spec 030 FR-032: whether movement is play or preparation.
    ///
    /// A Game Master dragging a token in preparation and in play is the same
    /// gesture, so this cannot be inferred and has to be told.
    SetScenePlaying {
        playing: bool,
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
pub(crate) struct DiceRollDiePayload {
    #[serde(rename = "finalValue")]
    pub(crate) final_value: i64,
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
    /// Update ticks since `start()`, monotonic. See `EngineStats::frames` —
    /// the one counter here that reports whether the loop is turning at all,
    /// which is what a caller outside the engine has no other way to ask.
    pub frames: u64,
}

/// Everything currently displayed, keyed by token id.
///
/// Spec 029 FR-021. Mirrored here as each `set_token_status` is applied, so
/// the state can be read back synchronously from JavaScript — an ECS query
/// cannot be run from outside the schedule.
///
/// Two callers, and the second is the reason this is a supported surface
/// rather than a debugging one:
///
/// 1. Tests assert what *would* be drawn without rendering a pixel, which
///    matters because this crate's own tests never execute.
/// 2. The React corner panel reads it, so the engine stays the single source
///    of truth for resolved status and React observes rather than recomputes
///    (Constitution I, ADR-053).
///
/// It is deliberately read-only. A debugging surface that can also mutate
/// state becomes a way to write tests that pass against situations the
/// application cannot reach.
pub(crate) static TOKEN_STATUS: OnceLock<Mutex<BTreeMap<String, Vec<StatusResourcePayload>>>> =
    OnceLock::new();

pub(crate) fn token_status_slot() -> &'static Mutex<BTreeMap<String, Vec<StatusResourcePayload>>> {
    TOKEN_STATUS.get_or_init(|| Mutex::new(BTreeMap::new()))
}
