//! A renderer self-test that draws through a *different* pipeline than
//! sprites.
//!
//! Sprites and gizmos take separate paths to the screen: sprites go through
//! `bevy_sprite`'s batched quad pipeline, gizmos through `bevy_gizmos`'
//! polyline pipeline. Both need the same 2D render graph, the same camera and
//! the same surface. So drawing both and comparing splits an otherwise
//! opaque "the canvas is blank" into three distinct diagnoses:
//!
//! - **Neither draws** — the 2D render graph, camera or surface is at fault.
//!   Everything downstream of "clear the target" is dead.
//! - **Gizmos draw, sprites do not** — the render graph is fine and the fault
//!   is specific to the sprite pipeline (extraction, batching, its shader, or
//!   its texture binding).
//! - **Both draw** — rendering works, and a missing map is about *that*
//!   sprite: its asset, size, position or visibility.
//!
//! This exists because the engine once reached a state where every observable
//! signal said rendering should work — one active camera, a correct viewport
//! and orthographic projection, sprites reporting `view_visible == true` with
//! `image_loaded == true`, no errors from `bevy_asset`, the pipeline cache or
//! wgpu — and the canvas still showed nothing but its clear colour. When
//! every indirect signal is green, the only way forward is a direct one.
//!
//! What it found: `Cargo.toml` enabled `bevy_sprite`/`bevy_ui`/`bevy_gizmos`
//! but none of the matching `*_render` features. Bevy 0.18 splits each of
//! those subsystems in two — components and main-world logic in one crate,
//! the code that actually draws them in another — so the engine had a fully
//! working scene graph and no renderer for it. The render-world trace showed
//! it in one line: `RenderVisibleEntities<Sprite> = 3` alongside
//! `Transparent2d items = 0`, i.e. the view could see the sprites and no
//! system existed to queue them.
//!
//! It is kept because that class of bug is invisible from the main world by
//! construction, and this is the only instrument that sees it.
//!
//! Off by default. Enable it from JavaScript with the `set_render_probe`
//! world command so it costs nothing in normal play.

use bevy::core_pipeline::core_2d::Transparent2d;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_phase::ViewSortedRenderPhases;
use bevy::render::view::{ExtractedView, RenderVisibleEntities};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::sprite_render::ExtractedSprites;

/// Whether the probe is currently drawing and tracing. Toggled by the
/// `set_render_probe` external command (see `lib.rs`).
///
/// `ExtractResource` so the render world can read it too — the render-side
/// trace below lives in the `RenderApp`, which is a separate world and cannot
/// see main-world resources unless they are extracted into it.
#[derive(Resource, Default, Debug, Clone, ExtractResource)]
pub struct RenderProbeEnabled(pub bool);

/// How many frames between trace lines. At ~60fps this is roughly one line a
/// second — frequent enough to watch state settle, sparse enough to read.
const TRACE_EVERY_N_FRAMES: u32 = 60;

pub struct RenderProbePlugin;

impl Plugin for RenderProbePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderProbeEnabled>()
            .init_resource::<EngineStats>()
            .add_plugins(ExtractResourcePlugin::<RenderProbeEnabled>::default())
            // Always on, unlike the probe. This measures the engine's own
            // update loop, which is the only way to see headroom: a browser
            // caps `requestAnimationFrame` at the display refresh, so a JS-side
            // frame timer reads 16.7ms whether the engine is using 5% of that
            // budget or 95%. Everything looks identical right up until it
            // falls off a cliff.
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(
                Update,
                (draw_render_probe, trace_main_world, publish_engine_stats),
            );

        // The render world is where "was anything actually queued to draw"
        // can be answered. Nothing in the main world can see it.
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, trace_render_phases.after(RenderSystems::PhaseSort));
        }
    }
}

/// Draws a fixed set of shapes centred on the world origin.
///
/// Sized in the hundreds of world units so it lands well inside the default
/// camera's view (±800 x ±450 at scale 1) and is impossible to mistake for
/// map content: pure magenta, green and white, none of which occur in the
/// clear colour.
fn draw_render_probe(enabled: Res<RenderProbeEnabled>, mut gizmos: Gizmos) {
    if !enabled.0 {
        return;
    }

    // A rectangle, a circle and two diagonals: if only some of these appear,
    // the failure is in a specific primitive rather than the pipeline.
    gizmos.rect_2d(
        Isometry2d::IDENTITY,
        Vec2::new(600.0, 400.0),
        Color::srgb(1.0, 0.0, 1.0),
    );
    gizmos.circle_2d(Isometry2d::IDENTITY, 150.0, Color::srgb(0.0, 1.0, 0.0));
    gizmos.line_2d(
        Vec2::new(-400.0, -300.0),
        Vec2::new(400.0, 300.0),
        Color::WHITE,
    );
    gizmos.line_2d(
        Vec2::new(-400.0, 300.0),
        Vec2::new(400.0, -300.0),
        Color::WHITE,
    );
}

/// Main-world trace: what the renderer is being *asked* to draw.
///
/// Reports the camera, and how many sprites exist versus how many survive
/// visibility computation. A sprite that is present but not `view_visible` was
/// culled or hidden; a sprite that is `view_visible` should reach the render
/// world.
fn trace_main_world(
    enabled: Res<RenderProbeEnabled>,
    mut frame: Local<u32>,
    cameras: Query<(&Camera, &Transform, &Projection)>,
    sprites: Query<(&ViewVisibility, &Sprite)>,
) {
    if !enabled.0 {
        return;
    }
    *frame += 1;
    if *frame % TRACE_EVERY_N_FRAMES != 0 {
        return;
    }

    for (camera, transform, projection) in cameras.iter() {
        let area = match projection {
            Projection::Orthographic(ortho) => format!("{:?}", ortho.area),
            _ => "non-orthographic".to_string(),
        };
        info!(
            target: "render_probe",
            "main: camera active={} order={} viewport={:?} translation={:?} area={}",
            camera.is_active,
            camera.order,
            camera.logical_viewport_size(),
            transform.translation,
            area,
        );
    }

    let total = sprites.iter().count();
    let visible = sprites.iter().filter(|(vv, _)| vv.get()).count();
    info!(
        target: "render_probe",
        "main: sprites total={total} view_visible={visible}",
    );
}

/// Render-world trace: what the renderer actually *queued*.
///
/// This is the measurement that separates the two remaining explanations for
/// a canvas that clears but never draws:
///
/// - `items=0` — nothing was queued. The camera extracted (it must have, or
///   there would be no clear), but drawable entities did not, so the fault is
///   in extraction/queueing.
/// - `items>0` — draws were queued and the phase still produced no pixels, so
///   the fault is downstream: pass execution, the pipeline, or the final blit
///   to the swapchain.
fn trace_render_phases(
    enabled: Option<Res<RenderProbeEnabled>>,
    phases: Option<Res<ViewSortedRenderPhases<Transparent2d>>>,
    // Mirrors the shape of `queue_sprites`' own view query. `Msaa` is in
    // there deliberately: that system requires it on the view, so a view
    // entity without one is silently skipped and queues nothing at all.
    views: Query<(&RenderVisibleEntities, &ExtractedView, &Msaa)>,
    views_without_msaa: Query<&ExtractedView, Without<Msaa>>,
    // The output of `extract_sprites`. `queue_sprites` iterates exactly this
    // list, so an empty one queues nothing regardless of how healthy the
    // view and its visible-entity set look.
    extracted: Option<Res<ExtractedSprites>>,
    mut frame: Local<u32>,
) {
    if !enabled.is_some_and(|e| e.0) {
        return;
    }
    *frame += 1;
    if *frame % TRACE_EVERY_N_FRAMES != 0 {
        return;
    }

    let Some(phases) = phases else {
        warn!(target: "render_probe", "render: no Transparent2d phase resource — Core2dPlugin missing?");
        return;
    };

    if phases.is_empty() {
        warn!(
            target: "render_probe",
            "render: ZERO Transparent2d views — no camera reached the render world",
        );
        return;
    }

    for (view, phase) in phases.iter() {
        info!(
            target: "render_probe",
            "render: Transparent2d view items={}",
            phase.items.len(),
        );
        let _ = view;
    }

    // Does `queue_sprites`' view query match anything at all?
    info!(
        target: "render_probe",
        "render: views matching (RenderVisibleEntities, ExtractedView, Msaa)={} · views missing Msaa={}",
        views.iter().count(),
        views_without_msaa.iter().count(),
    );

    info!(
        target: "render_probe",
        "render: ExtractedSprites={}",
        extracted.map_or_else(|| "<resource missing>".to_string(), |e| e.sprites.len().to_string()),
    );

    // And for the views that do match, is the sprite visibility class
    // populated? `queue_sprites` skips every extracted sprite that is not in
    // this set, so an empty set queues nothing even when sprites extracted
    // fine and reported `view_visible == true` in the main world.
    for (visible, _, _) in views.iter() {
        info!(
            target: "render_probe",
            "render: view RenderVisibleEntities<Sprite>={}",
            // Bevy 0.19 replaced `iter::<Sprite>()` with a per-class lookup.
            // Sprites are CPU-culled, so `entities_cpu_culling` is the list
            // `queue_sprites` walks; an absent class means nothing sprite-like
            // is visible from this view, which is the same zero.
            visible
                .get::<Sprite>()
                .map_or(0, |class| class.entities_cpu_culling.len()),
        );
    }
}

/// Engine-side performance counters, published for the host page to read.
///
/// Kept as a resource updated each frame rather than pushed through the event
/// callback: stats are polled on demand by a benchmark harness, and emitting
/// them every frame would flood the same channel real world events use.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct EngineStats {
    /// Smoothed engine frame time, milliseconds. **Not** the presented frame
    /// interval — this is what the engine's update loop actually costs, so it
    /// keeps reporting real numbers below the vsync ceiling.
    pub frame_time_ms: f64,
    pub fps: f64,
    pub sprites: usize,
    pub lights: usize,
    pub walls: usize,
    pub tokens: usize,
    /// Meshes spawned for wall shadows — the term that grows as
    /// lights x walls, and the first thing to check when a scene gets heavy.
    pub shadow_quads: usize,
}

fn publish_engine_stats(
    diagnostics: Res<DiagnosticsStore>,
    mut stats: ResMut<EngineStats>,
    sprites: Query<(), With<Sprite>>,
    tokens: Query<(), With<crate::TokenIdentity>>,
    light_set: Option<Res<crate::resources::LightSet>>,
    wall_set: Option<Res<crate::resources::WallSet>>,
    meshes: Query<(), With<Mesh2d>>,
) {
    if let Some(frame_time) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
    {
        stats.frame_time_ms = frame_time;
    }
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        stats.fps = fps;
    }

    stats.sprites = sprites.iter().count();
    stats.tokens = tokens.iter().count();
    stats.lights = light_set.map_or(0, |set| set.lights().len());
    stats.walls = wall_set.map_or(0, |set| set.walls().len());
    // Every Mesh2d that is not the single darkness quad is a shadow.
    stats.shadow_quads = meshes.iter().count().saturating_sub(1);

    // Mirror out to the wasm-visible slot. `App::run()` owns the `World` and
    // never returns on wasm, so a static is the only way out.
    if let Ok(mut slot) = crate::engine_stats_slot().lock() {
        slot.frame_time_ms = stats.frame_time_ms;
        slot.fps = stats.fps;
        slot.sprites = stats.sprites;
        slot.tokens = stats.tokens;
        slot.lights = stats.lights;
        slot.walls = stats.walls;
        slot.shadow_quads = stats.shadow_quads;
    }
}
