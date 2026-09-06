use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::AtomicBool;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};

// Used only by `start`, which is browser-only — see its gate below.
#[cfg(target_arch = "wasm32")]
use bevy::asset::{AssetPlugin, UnapprovedPathMode};
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use bevy::window::{Window, WindowPlugin, WindowResolution};
use js_sys::Function;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

// Public module exports for Phase 4.2/4.3 Bevy integration
pub mod components;
pub mod derived_data;
pub mod movement;
// The only genuinely browser-bound module in the crate: `gloo-net`,
// `wasm_bindgen_futures::spawn_local` and the `web_sys` WebSocket. Everything
// else that carried a wasm gate was gated for sitting next to this, not for
// needing it (spec 032 T083).
#[cfg(target_arch = "wasm32")]
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

use components::{DerivedStats, Token, TokenAttributes};
#[cfg(target_arch = "wasm32")]
use derived_data::*;
use movement::PlayerControlled;
#[cfg(target_arch = "wasm32")]
use plugins::{
    BackgroundPlugin, CachedAssetsPlugin, CameraPlugin, CanvasLayerPlugin, DarknessPlugin,
    DiceRollPlugin, GridPlugin, LightingOverlayPlugin, LightingPlugin, RenderProbePlugin,
    ScenePlugin, SelectionPlugin, ShapePlugin, StatusDisplayPlugin, TokenPlugin, WallPlugin,
};
use plugins::{RenderProbeEnabled, ResolvedResource, TokenStatus};
use resources::{
    CameraManager, DoorState, GridSnapEnabled, GridVisible, IsGameMaster, LightSet,
    LightSource as EngineLight, LightingOverlay, PlacedCanvasImage, PlacedCanvasImages,
    SceneAmbient, SceneBackground, SceneGrid, Shape as EngineShape, ShapeKind, ShapeSet,
    TokenGridBehaviour, TokenVision, Wall as EngineWall, WallSet,
};
#[cfg(target_arch = "wasm32")]
use sync_test::*;
#[cfg(target_arch = "wasm32")]
use systems::*;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::resource_display::{
    AppearanceOverride, Disclosed, ResourceDefinition,
};
use thunderforge_canvas_core::token_kind::TokenKind;
use thunderforge_canvas_core::vision::{Illumination, Rgb, VisionProfile};

mod payloads;
pub(crate) use payloads::*;

mod sdk;
pub(crate) use sdk::*;

mod startup;

mod app;
pub(crate) use app::*;
