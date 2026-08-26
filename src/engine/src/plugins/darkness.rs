//! The lighting layer: a darkness sheet over the map, with light pools cut out
//! of it and wall shadows painted back in.
//!
//! Illumination previously only drove *token visibility* — a token in the dark
//! was hidden, but the map itself stayed as bright as it was imported. That
//! reads as no lighting at all, because the thing a player actually looks at
//! is the map.
//!
//! # How it composes
//!
//! Three pieces, drawn in order above the background art and below the tokens:
//!
//! 1. **The darkness quad** — one map-sized mesh using `DarknessMaterial`. Its
//!    shader computes, per fragment, how lit that point is and outputs black
//!    at the inverse alpha. Lit areas come out transparent; unlit areas opaque.
//! 2. **Shadow quads** — one solid-black mesh per (light, vision-blocking
//!    wall), from `vision::shadow_quad`. Drawn *on top* of the darkness, so
//!    they re-darken the parts of a light pool that the wall should block.
//! 3. Tokens, above both.
//!
//! Doing occlusion this way — geometry on top rather than ray-marching every
//! wall inside the fragment shader — is what keeps it viable on WebGL2. A
//! scene with 12 lights and 200 walls is 2400 small quads, which the GPU does
//! not notice; the same scene as a per-fragment loop is 2400 segment
//! intersections per pixel.
//!
//! The whole layer is inert while ambient light is `Bright` and, in that case,
//! is not spawned at all — an unconfigured scene renders exactly as before.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};

use crate::resources::{CanvasLayer, LightSet, SceneAmbient, WallSet};
use crate::TokenIdentity;
use thunderforge_canvas_core::vision::{shadow_quad, Illumination, Rgb};

/// Must match `MAX_LIGHTS` in `darkness.wgsl`.
///
/// Fixed-size because WebGL2 requires compile-time uniform array lengths, so
/// this is a real ceiling rather than a growable buffer. The budget is not
/// tight: the uniform block is `2 + 2N` vec4s, so 128 lights is 4KB against
/// WebGL2's guaranteed 16KB minimum block size. An earlier value of 16 was
/// simply over-cautious.
///
/// It is a cap on lights *uploaded per frame*, not on lights a scene may
/// contain. `visible_lights` culls to what can actually affect the current
/// view first, so a scene with a thousand lights across a huge map works fine
/// — only the handful near the camera are ever sent.
pub const MAX_LIGHTS: usize = 128;

/// How far a shadow is projected past its wall, as a multiple of the casting
/// light's outer radius. Slightly over 1 so a shadow always clears the light
/// pool it is cutting into, without running to absurd coordinates.
const SHADOW_REACH_FACTOR: f32 = 1.2;

/// Margin added around the camera's view when culling, in world units.
///
/// A light just off-screen still spills its pool into view, and a shadow cast
/// from off-screen still falls across it. Culling to the exact viewport would
/// make both pop at the edges as the camera pans.
const CULL_MARGIN: f32 = 512.0;

#[derive(Clone, Copy, ShaderType, Debug)]
pub struct DarknessUniform {
    /// rgb = ambient tint, a = darkness strength for an unlit fragment.
    pub ambient: Vec4,
    /// x = active light count.
    pub params: Vec4,
    /// xy = world position, z = bright radius, w = dim radius.
    pub lights: [Vec4; MAX_LIGHTS],
    /// rgb = colour, a = intensity.
    pub light_colors: [Vec4; MAX_LIGHTS],
}

impl Default for DarknessUniform {
    fn default() -> Self {
        Self {
            ambient: Vec4::new(0.0, 0.0, 0.0, 0.0),
            params: Vec4::ZERO,
            lights: [Vec4::ZERO; MAX_LIGHTS],
            light_colors: [Vec4::ZERO; MAX_LIGHTS],
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct DarknessMaterial {
    #[uniform(0)]
    pub uniform: DarknessUniform,
}

impl Material2d for DarknessMaterial {
    fn fragment_shader() -> ShaderRef {
        // Embedded rather than fetched: the engine serves no assets of its
        // own, and a shader that fails to load renders as a black screen with
        // no error anyone sees.
        //
        // The path is what `embedded_asset!` registers below — crate name,
        // then the shader's location relative to `src/`. Keeping the `.wgsl`
        // beside the plugin that owns it is what keeps that path this short.
        "embedded://thunderforge_engine/plugins/darkness.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Marks the single darkness quad.
#[derive(Component)]
struct DarknessQuad;

/// Marks a wall's shadow mesh.
#[derive(Component)]
struct ShadowQuad;

/// Solid black at the scene's ambient darkness, so a shadow is exactly as dark
/// as the unlit ground around it.
#[derive(Resource, Default)]
struct ShadowAssets {
    material: Option<Handle<ColorMaterial>>,
}

pub struct DarknessPlugin;

impl Plugin for DarknessPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "darkness.wgsl");

        app.add_plugins(Material2dPlugin::<DarknessMaterial>::default())
            // Owns `SceneAmbient`. Nothing else registered it, so
            // `set_ambient_light` was writing into an `Option<ResMut<_>>` that
            // was always `None` — the command reported success and changed
            // nothing, and the whole lighting layer saw permanent daylight.
            .init_resource::<SceneAmbient>()
            .init_resource::<ShadowAssets>()
            .add_systems(Update, (sync_darkness_quad, sync_shadow_quads).chain());
    }
}

/// How dark an unlit fragment gets, per ambient level.
///
/// `Dim` is not half of `Dark`: a dim scene should still read as navigable,
/// so it takes far less than half the darkness. Pure black is avoided even at
/// `Dark` — leaving a little of the map visible is what every VTT does, since
/// a truly black screen hides the geometry players need to orient by.
fn darkness_strength(level: Illumination) -> f32 {
    match level {
        Illumination::Bright => 0.0,
        Illumination::Dim => 0.35,
        Illumination::Dark => 0.92,
    }
}

/// The world-space extent the darkness quad has to cover.
///
/// Sized from the tokens and lights in play plus a generous margin rather than
/// from the map, because the engine is not told the map's extent — and a quad
/// that is too small leaves a bright border where the darkness stops.
fn coverage(lights: &LightSet, tokens: &Query<&Transform, With<TokenIdentity>>) -> (Vec2, f32) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut any = false;

    for light in lights.lights() {
        let p = light.position();
        let r = light.radius.max(0.0);
        min = min.min(p - Vec2::splat(r));
        max = max.max(p + Vec2::splat(r));
        any = true;
    }
    for transform in tokens.iter() {
        let p = transform.translation.truncate();
        min = min.min(p);
        max = max.max(p);
        any = true;
    }

    if !any {
        return (Vec2::ZERO, 8192.0);
    }

    let center = (min + max) / 2.0;
    let extent = (max - min).max_element().max(2048.0) * 2.0;
    (center, extent)
}

/// The world rectangle the camera can see, padded by `CULL_MARGIN`.
fn cull_rect(projection: &Projection, transform: &GlobalTransform) -> Option<Rect> {
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };
    let center = transform.translation().truncate();
    Some(Rect {
        min: center + ortho.area.min - Vec2::splat(CULL_MARGIN),
        max: center + ortho.area.max + Vec2::splat(CULL_MARGIN),
    })
}

/// Whether a light's pool reaches into `view`.
fn light_touches(light_pos: Vec2, radius: f32, view: Rect) -> bool {
    // Closest point on the rect to the light, then a radius test — the
    // standard circle/AABB overlap check.
    let closest = light_pos.clamp(view.min, view.max);
    closest.distance_squared(light_pos) <= radius * radius
}

/// The lights that can affect the current view, nearest first, capped at
/// `MAX_LIGHTS`.
///
/// Culling here rather than in the shader is what decouples "lights a scene
/// has" from "lights the GPU pays for". A 500-light dungeon only ever uploads
/// the few whose pools reach the screen, and the fragment loop is short
/// regardless of scene size.
fn visible_lights<'a>(
    light_set: &'a LightSet,
    view: Option<Rect>,
) -> Vec<&'a thunderforge_canvas_core::lighting::LightSource> {
    let mut candidates: Vec<&thunderforge_canvas_core::lighting::LightSource> = light_set
        .lights()
        .iter()
        .filter(|light| light.intensity > 0.0)
        .filter(|light| match view {
            Some(view) => light_touches(light.position(), light.radius, view),
            None => true,
        })
        .collect();

    if candidates.len() > MAX_LIGHTS {
        // Over budget: keep the ones nearest the middle of the view, which are
        // the ones a viewer is looking at. Dropping arbitrary lights instead
        // would make distant flicker change what is lit under the cursor.
        let focus = view.map_or(Vec2::ZERO, |v| (v.min + v.max) / 2.0);
        candidates.sort_by(|a, b| {
            a.position()
                .distance_squared(focus)
                .total_cmp(&b.position().distance_squared(focus))
        });
        let dropped = candidates.len() - MAX_LIGHTS;
        warn!(
            target: "lighting",
            "{dropped} of {} in-view lights exceed the {MAX_LIGHTS}-light budget and are not lit",
            candidates.len(),
        );
        candidates.truncate(MAX_LIGHTS);
    }

    candidates
}

fn sync_darkness_quad(
    mut commands: Commands,
    ambient: Option<Res<SceneAmbient>>,
    light_set: Res<LightSet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DarknessMaterial>>,
    tokens: Query<&Transform, With<TokenIdentity>>,
    cameras: Query<(&Projection, &GlobalTransform), With<Camera2d>>,
    existing: Query<(Entity, &MeshMaterial2d<DarknessMaterial>, &mut Transform), (With<DarknessQuad>, Without<TokenIdentity>)>,
) {
    let ambient = ambient.map_or_else(
        || thunderforge_canvas_core::vision::AmbientLight::daylight(),
        |a| a.0,
    );
    let strength = darkness_strength(ambient.level);

    // A bright scene has no darkness to draw. Despawn rather than render a
    // fully-transparent quad every frame.
    if strength <= 0.0 {
        for (entity, _, _) in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let mut uniform = DarknessUniform {
        ambient: {
            let tint = ambient.color.unwrap_or(Rgb { r: 0.02, g: 0.03, b: 0.08 });
            Vec4::new(tint.r, tint.g, tint.b, strength)
        },
        ..Default::default()
    };

    let view = cameras
        .single()
        .ok()
        .and_then(|(projection, transform)| cull_rect(projection, transform));

    let mut count = 0usize;
    for light in visible_lights(&light_set, view) {
        let p = light.position();
        // Same single-radius-to-bright/dim mapping as `resolve_light`.
        uniform.lights[count] = Vec4::new(p.x, p.y, light.radius * 0.5, light.radius);
        let color = light
            .color
            .as_deref()
            .and_then(Rgb::parse_hex)
            .unwrap_or(Rgb::WHITE);
        uniform.light_colors[count] = Vec4::new(color.r, color.g, color.b, light.intensity);
        count += 1;
    }
    uniform.params.x = count as f32;

    let (center, extent) = coverage(&light_set, &tokens);
    let translation = center.extend(CanvasLayer::Background.z() + 0.5);

    let mut existing = existing;
    if let Some((_, material_handle, mut transform)) = existing.iter_mut().next() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.uniform = uniform;
        }
        transform.translation = translation;
        transform.scale = Vec3::new(extent, extent, 1.0);
        return;
    }

    // Unit quad, scaled by the transform, so resizing never rebuilds the mesh.
    let mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let material = materials.add(DarknessMaterial { uniform });

    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(translation).with_scale(Vec3::new(extent, extent, 1.0)),
        DarknessQuad,
    ));
}

fn sync_shadow_quads(
    mut commands: Commands,
    ambient: Option<Res<SceneAmbient>>,
    light_set: Res<LightSet>,
    wall_set: Res<WallSet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut shadow_assets: ResMut<ShadowAssets>,
    cameras: Query<(&Projection, &GlobalTransform), With<Camera2d>>,
    existing: Query<Entity, With<ShadowQuad>>,
) {
    let ambient = ambient.map_or_else(
        || thunderforge_canvas_core::vision::AmbientLight::daylight(),
        |a| a.0,
    );
    let strength = darkness_strength(ambient.level);

    // Rebuilt wholesale when a light or wall changes. Shadows are a pure
    // function of (lights x walls), and diffing that product costs more than
    // regenerating a few hundred four-vertex meshes.
    //
    // Camera movement deliberately does NOT trigger a rebuild. Culling below
    // uses the view, so a fast pan can leave a shadow briefly stale at the
    // screen edge — which is why `CULL_MARGIN` is generous. Rebuilding every
    // frame the camera moves would be far worse.
    if !light_set.is_changed() && !wall_set.is_changed() && !existing.is_empty() {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    if strength <= 0.0 {
        return;
    }

    // Matched to the ambient darkness — both its strength *and* its tint — so
    // a shadow is exactly as dark as the unlit ground it falls on. Pure black
    // here was visibly wrong: the darkness quad carries a cool ambient tint,
    // so an untinted shadow read as a blacker-than-night cut-out rather than
    // as ordinary shade.
    let tint = ambient.color.unwrap_or(Rgb { r: 0.02, g: 0.03, b: 0.08 });
    let shadow_color = Color::srgba(tint.r, tint.g, tint.b, strength);

    let material = shadow_assets
        .material
        .get_or_insert_with(|| color_materials.add(ColorMaterial::from_color(shadow_color)))
        .clone();
    if let Some(existing_material) = color_materials.get_mut(&material) {
        existing_material.color = shadow_color;
    }

    let z = CanvasLayer::Background.z() + 0.6;
    let view = cameras
        .single()
        .ok()
        .and_then(|(projection, transform)| cull_rect(projection, transform));

    // Every shadow goes into ONE mesh rather than one entity each.
    //
    // Measured: at 2000 walls and 200 lights the per-entity version produced
    // 8,983 `Mesh2d` entities and 75.5ms engine frame time (13fps), against
    // 18.5ms for the same walls with no lights. Isolated axes scaled fine —
    // 3000 tokens cost 14ms, 800 lights 12ms — but the product of lights and
    // walls did not, because each shadow carried its own mesh asset,
    // transform, material binding and draw call.
    //
    // The geometry is identical and every quad shares one material, so the
    // whole set collapses into a single mesh with no visual difference.
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for light in visible_lights(&light_set, view) {
        let origin = light.position();
        let reach = light.radius * SHADOW_REACH_FACTOR;

        for wall in wall_set.vision_blocking_walls() {
            // A wall beyond the light's reach casts no shadow *from this
            // light*: there is no illumination out there to subtract.
            let midpoint = wall.midpoint();
            if midpoint.distance(origin) > light.radius + wall.length() {
                continue;
            }

            let Some(quad) = shadow_quad(origin, wall.start(), wall.end(), reach) else {
                continue;
            };

            let base = positions.len() as u32;
            positions.extend(quad.iter().map(|p| [p.x, p.y, 0.0]));
            // Two triangles, fanned from the first corner. `shadow_quad`
            // returns its points already ordered around the perimeter.
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    if positions.is_empty() {
        return;
    }

    let quads = positions.len() / 4;
    let mesh = meshes.add(combined_mesh(positions, indices));
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_xyz(0.0, 0.0, z),
        ShadowQuad,
    ));

    debug!(target: "lighting", "rebuilt {quads} shadow quads as one mesh");
}

/// Builds one mesh holding every shadow quad in the scene.
///
/// Positions are already in world space, so the entity's transform stays at
/// the origin — baking the coordinates in is what allows quads cast by
/// different lights to share a single mesh.
fn combined_mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let vertex_count = positions.len();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; vertex_count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; vertex_count])
    .with_inserted_indices(Indices::U32(indices))
}
