//! The lighting layer: a darkness sheet over the map, with light pools cut out
//! of it — each light stopped by its own walls.
//!
//! Illumination previously only drove *token visibility* — a token in the dark
//! was hidden, but the map itself stayed as bright as it was imported. That
//! reads as no lighting at all, because the thing a player actually looks at
//! is the map.
//!
//! # How it composes
//!
//! One map-sized mesh using `DarknessMaterial`, drawn over everything that is
//! the map — its art, shapes, pasted images — and under tokens and the Game
//! Master's walls and light markers (`CanvasLayer::darkness_z`). The grid is
//! drawn with gizmos, which Bevy draws after everything else, so its lines
//! stay readable over the dark — a table still needs to count squares. Its
//! shader computes, per fragment, how lit that point is and outputs darkness
//! at the inverse alpha.
//!
//! # Shadows, per light (playtest 2026-09-10 P9)
//!
//! Each light in view has a row in a shadow-map texture: in each of
//! `SHADOW_BINS` directions, how far it reaches before a wall stops it
//! (`vision::shadow_map_row`). The shader stops a light only past its own
//! walls, so a room lit from both sides stays lit behind each wall. Shadows
//! used to be quads painted over the whole layer, which darkened *every*
//! light's pool they fell across.
//!
//! Rows are computed on the CPU and cached per light, recomputed when the
//! light moves — a torch carried by a token follows it — or the walls change.
//! The texture is reassembled when the set of lights in view changes, which is
//! what a pan does: the old shadow quads were built for the lights near the
//! camera once, and a pan left the rest unshadowed until something else
//! changed.
//!
//! The whole layer is inert while ambient light is `Bright` and, in that case,
//! is not spawned at all — an unconfigured scene renders exactly as before.

use std::collections::HashMap;

use bevy::asset::{Asset, RenderAssetUsages};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};

use crate::TokenIdentity;
use crate::resources::{CanvasLayer, LightSet, SceneAmbient, WallSet};
use crate::systems::lighting::effective_light_position;
use thunderforge_canvas_core::lighting::LightSource;
use thunderforge_canvas_core::vision::{Illumination, Rgb, shadow_map_row};

/// Must match `MAX_LIGHTS` in `darkness.wgsl`.
///
/// Fixed-size because WebGL2 requires compile-time uniform array lengths, so
/// this is a real ceiling rather than a growable buffer. The budget is not
/// tight: the uniform block is `2 + 2N` vec4s, so 128 lights is 4KB against
/// WebGL2's guaranteed 16KB minimum block size.
///
/// It is a cap on lights *uploaded per frame*, not on lights a scene may
/// contain. `visible_lights` culls to what can actually affect the current
/// view first, so a scene with a thousand lights across a huge map works fine
/// — only the handful near the camera are ever sent.
pub const MAX_LIGHTS: usize = 128;

/// Directions per light in the shadow map. Must match `SHADOW_BINS` in
/// `darkness.wgsl`. 512 puts a shadow's edge within about 7 units of true at
/// a 600-unit radius, and the whole map is 256KB.
pub const SHADOW_BINS: usize = 512;

/// Margin added around the camera's view when culling, in world units.
///
/// A light just off-screen still spills its pool into view. Culling to the
/// exact viewport would make pools pop at the edges as the camera pans.
const CULL_MARGIN: f32 = 512.0;

/// How far a light may move before its shadows are recomputed, in world
/// units. Below this a carried torch jittering by a fraction of a pixel
/// would recompute every frame for nothing.
const MOVE_EPSILON: f32 = 0.5;

#[derive(Clone, Copy, ShaderType, Debug, PartialEq)]
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
    /// One row per uploaded light, one texel per direction — see
    /// `pack_row`.
    #[texture(1)]
    #[sampler(2)]
    pub shadow_map: Handle<Image>,
}

impl Material2d for DarknessMaterial {
    fn fragment_shader() -> ShaderRef {
        // Embedded rather than fetched: the engine serves no assets of its
        // own, and a shader that fails to load renders as a black screen with
        // no error anyone sees.
        "embedded://thunderforge_engine/plugins/darkness.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Marks the single darkness quad.
#[derive(Component)]
struct DarknessQuad;

/// How many (light, wall) pairs cast a shadow in view — what `EngineStats`
/// reports as `shadow_quads`, the term that grows as lights × walls.
///
/// Counted where the shadows are computed. It used to be inferred as "every
/// `Mesh2d` in the scene but one", which counted the darkness sheet itself
/// and any drawn shape — a number that could not tell a lit room from a dark
/// one.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ShadowStats {
    pub casting_pairs: usize,
}

/// What a light's cached row was computed for.
#[derive(Clone, Copy, PartialEq)]
struct RowKey {
    position: Vec2,
    radius: f32,
    casts_shadows: bool,
    /// `walls_near` at the time — so an edit to a wall recomputes only the
    /// lights it can shadow.
    walls: u64,
}

impl RowKey {
    fn still_holds_for(&self, other: &RowKey) -> bool {
        self.position.distance(other.position) <= MOVE_EPSILON
            && self.radius == other.radius
            && self.casts_shadows == other.casts_shadows
            && self.walls == other.walls
    }
}

/// A fingerprint of the walls that can shadow a light at `origin` within
/// `reach` — the same walls `shadow_map_row` considers.
///
/// Dragging a wall changes the wall set every frame of the drag, and clearing
/// every cached row on each change recomputed every light in view each frame:
/// millions of ray tests a frame on a large map. Comparing fingerprints
/// recomputes only the lights near the wall that moved.
fn walls_near(origin: Vec2, reach: f32, walls: &WallSet) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for wall in walls
        .vision_blocking_walls()
        .filter(|wall| wall.midpoint().distance(origin) <= reach + wall.length() / 2.0)
    {
        for value in [wall.x1, wall.y1, wall.x2, wall.y2] {
            value.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

struct CachedRow {
    key: RowKey,
    texels: Vec<u8>,
    casting_walls: usize,
}

/// The shadow-map texture and what it currently holds.
#[derive(Resource, Default)]
struct ShadowMap {
    image: Option<Handle<Image>>,
    rows: HashMap<String, CachedRow>,
    /// Which light each texture row belongs to, in upload order.
    layout: Vec<String>,
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
            .init_resource::<ShadowMap>()
            .init_resource::<ShadowStats>()
            .add_systems(Update, sync_darkness);
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

/// A light, where it actually is: a light carried by a token is where the
/// token is, not where the light was stored.
struct Placed<'a> {
    light: &'a LightSource,
    position: Vec2,
}

/// The world-space extent the darkness quad has to cover.
///
/// Sized from the tokens and lights in play plus a generous margin rather than
/// from the map, because the engine is not told the map's extent — and a quad
/// that is too small leaves a bright border where the darkness stops.
fn coverage(lights: &[Placed], tokens: &HashMap<String, Vec2>) -> (Vec2, f32) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut any = false;

    for placed in lights {
        let r = placed.light.radius.max(0.0);
        min = min.min(placed.position - Vec2::splat(r));
        max = max.max(placed.position + Vec2::splat(r));
        any = true;
    }
    for p in tokens.values() {
        min = min.min(*p);
        max = max.max(*p);
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
fn visible_lights<'a>(lights: &[Placed<'a>], view: Option<Rect>) -> Vec<Placed<'a>> {
    let mut candidates: Vec<Placed<'a>> = lights
        .iter()
        .filter(|p| p.light.intensity > 0.0)
        .filter(|p| match view {
            Some(view) => light_touches(p.position, p.light.radius, view),
            None => true,
        })
        .map(|p| Placed {
            light: p.light,
            position: p.position,
        })
        .collect();

    if candidates.len() > MAX_LIGHTS {
        // Over budget: keep the ones nearest the middle of the view, which are
        // the ones a viewer is looking at.
        let focus = view.map_or(Vec2::ZERO, |v| (v.min + v.max) / 2.0);
        candidates.sort_by(|a, b| {
            a.position
                .distance_squared(focus)
                .total_cmp(&b.position.distance_squared(focus))
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

/// One light's reach in each direction, as texels: the fraction of `reach`,
/// in 16 bits across red (high byte) and green (low byte). `darkness.wgsl`'s
/// `reach` undoes exactly this.
fn pack_row(distances: &[f32], reach: f32) -> Vec<u8> {
    let mut texels = Vec::with_capacity(distances.len() * 4);
    for distance in distances {
        let fraction = if reach > 0.0 {
            (distance / reach).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let packed = (fraction * 65535.0).round() as u16;
        texels.extend_from_slice(&[(packed >> 8) as u8, (packed & 0xff) as u8, 0, 255]);
    }
    texels
}

/// A row for a light that casts no shadows: it reaches its full radius in
/// every direction.
fn unshadowed_row() -> Vec<u8> {
    [255u8, 255, 0, 255].repeat(SHADOW_BINS)
}

fn new_shadow_image() -> Image {
    Image::new(
        Extent3d {
            width: SHADOW_BINS as u32,
            height: MAX_LIGHTS as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        unshadowed_row().repeat(MAX_LIGHTS),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

#[allow(clippy::too_many_arguments)]
fn sync_darkness(
    mut commands: Commands,
    ambient: Option<Res<SceneAmbient>>,
    light_set: Res<LightSet>,
    wall_set: Res<WallSet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DarknessMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut shadows: ResMut<ShadowMap>,
    mut stats: ResMut<ShadowStats>,
    tokens: Query<(&Transform, &TokenIdentity)>,
    cameras: Query<(&Projection, &GlobalTransform), With<Camera2d>>,
    mut existing: Query<
        (Entity, &MeshMaterial2d<DarknessMaterial>, &mut Transform),
        (With<DarknessQuad>, Without<TokenIdentity>),
    >,
) {
    let ambient = ambient.map_or_else(
        thunderforge_canvas_core::vision::AmbientLight::daylight,
        |a| a.0,
    );
    let strength = darkness_strength(ambient.level);

    // A bright scene has no darkness to draw. Despawn rather than render a
    // fully-transparent quad every frame.
    if strength <= 0.0 {
        for (entity, _, _) in existing.iter() {
            commands.entity(entity).despawn();
        }
        stats.casting_pairs = 0;
        // Walls edited while the scene is lit are never seen as changes by
        // this system — it returns before looking — so nothing cached may
        // outlive daylight, or a scene darkened again would shadow the walls
        // as they were.
        shadows.rows.clear();
        shadows.layout.clear();
        return;
    }

    let positions: HashMap<String, Vec2> = tokens
        .iter()
        .map(|(transform, identity)| (identity.0.clone(), transform.translation.truncate()))
        .collect();
    let placed: Vec<Placed> = light_set
        .lights()
        .iter()
        .map(|light| Placed {
            light,
            position: effective_light_position(light, &positions),
        })
        .collect();

    let view = cameras
        .single()
        .ok()
        .and_then(|(projection, transform)| cull_rect(projection, transform));
    let in_view = visible_lights(&placed, view);

    // A light removed: its row goes. Walls changed: each row checks its own
    // fingerprint below, so only the lights near the change recompute.
    if light_set.is_changed() {
        shadows.rows.retain(|id, _| light_set.get(id).is_some());
    }
    let walls_changed = wall_set.is_changed();

    let mut uniform = DarknessUniform {
        ambient: {
            let tint = ambient.color.unwrap_or(Rgb {
                r: 0.02,
                g: 0.03,
                b: 0.08,
            });
            Vec4::new(tint.r, tint.g, tint.b, strength)
        },
        ..Default::default()
    };

    let mut rows_changed = shadows.layout.len() != in_view.len();
    for (index, placed) in in_view.iter().enumerate() {
        let light = placed.light;
        let p = placed.position;
        // Same single-radius-to-bright/dim mapping as `resolve_light`.
        uniform.lights[index] = Vec4::new(p.x, p.y, light.radius * 0.5, light.radius);
        let color = light
            .color
            .as_deref()
            .and_then(Rgb::parse_hex)
            .unwrap_or(Rgb::WHITE);
        uniform.light_colors[index] = Vec4::new(color.r, color.g, color.b, light.intensity);

        // The fingerprint is taken afresh only when it can have changed: the
        // walls did, or the light moved to where other walls are.
        let cached = shadows.rows.get(&light.id);
        let walls = match cached {
            Some(row) if !walls_changed && row.key.position.distance(p) <= MOVE_EPSILON => {
                row.key.walls
            }
            _ => walls_near(p, light.radius, &wall_set),
        };
        let key = RowKey {
            position: p,
            radius: light.radius,
            casts_shadows: light.casts_shadows,
            walls,
        };
        let fresh = shadows
            .rows
            .get(&light.id)
            .is_some_and(|row| row.key.still_holds_for(&key));
        if !fresh {
            let row = if light.casts_shadows {
                let row = shadow_map_row(p, light.radius, &wall_set, SHADOW_BINS);
                CachedRow {
                    key,
                    texels: pack_row(&row.distances, light.radius),
                    casting_walls: row.casting_walls,
                }
            } else {
                CachedRow {
                    key,
                    texels: unshadowed_row(),
                    casting_walls: 0,
                }
            };
            shadows.rows.insert(light.id.clone(), row);
            rows_changed = true;
        }
        if shadows.layout.get(index) != Some(&light.id) {
            rows_changed = true;
        }
    }
    uniform.params.x = in_view.len() as f32;
    stats.casting_pairs = in_view
        .iter()
        .filter_map(|p| shadows.rows.get(&p.light.id))
        .map(|row| row.casting_walls)
        .sum();

    let image = match shadows.image.clone() {
        Some(handle) => handle,
        None => {
            let handle = images.add(new_shadow_image());
            shadows.image = Some(handle.clone());
            handle
        }
    };
    if rows_changed {
        let mut texels = Vec::with_capacity(SHADOW_BINS * MAX_LIGHTS * 4);
        for placed in &in_view {
            match shadows.rows.get(&placed.light.id) {
                Some(row) => texels.extend_from_slice(&row.texels),
                None => texels.extend_from_slice(&unshadowed_row()),
            }
        }
        texels.resize(SHADOW_BINS * MAX_LIGHTS * 4, 255);
        if let Some(mut target) = images.get_mut(&image) {
            target.data = Some(texels);
        }
        shadows.layout = in_view.iter().map(|p| p.light.id.clone()).collect();
    }

    let (center, extent) = coverage(&placed, &positions);
    let translation = center.extend(CanvasLayer::darkness_z());

    if let Some((_, material_handle, mut transform)) = existing.iter_mut().next() {
        // Written only when it differs: every write rebuilds the material's
        // uniform buffer and bind group. A rewritten shadow map needs no
        // write — Bevy updates the image in place, under the same view.
        let stale = materials
            .get(&material_handle.0)
            .is_none_or(|m| m.uniform != uniform || m.shadow_map != image);
        if stale && let Some(mut material) = materials.get_mut(&material_handle.0) {
            material.uniform = uniform;
            material.shadow_map = image;
        }
        let scale = Vec3::new(extent, extent, 1.0);
        if transform.translation != translation || transform.scale != scale {
            transform.translation = translation;
            transform.scale = scale;
        }
        return;
    }

    // Unit quad, scaled by the transform, so resizing never rebuilds the mesh.
    let mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let material = materials.add(DarknessMaterial {
        uniform,
        shadow_map: image,
    });

    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(translation).with_scale(Vec3::new(extent, extent, 1.0)),
        DarknessQuad,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `darkness.wgsl`'s `reach` computes from a texel.
    fn unpack(texel: &[u8], reach: f32) -> f32 {
        let packed = f32::from(texel[0]) * 256.0 + f32::from(texel[1]);
        packed / 65535.0 * reach
    }

    #[test]
    fn a_packed_row_reads_back_as_the_distances_it_was_made_from() {
        let distances = [0.0, 1.0, 137.25, 599.9, 600.0];
        let texels = pack_row(&distances, 600.0);
        for (i, distance) in distances.iter().enumerate() {
            let back = unpack(&texels[i * 4..i * 4 + 4], 600.0);
            assert!(
                (back - distance).abs() < 0.01,
                "{distance} came back as {back}"
            );
        }
    }

    #[test]
    fn an_unshadowed_row_reaches_the_full_radius() {
        let row = unshadowed_row();
        assert_eq!(row.len(), SHADOW_BINS * 4);
        assert!((unpack(&row[..4], 600.0) - 600.0).abs() < 1e-3);
    }

    #[test]
    fn a_carried_light_is_culled_by_where_its_token_is() {
        let torch = LightSource {
            id: "torch".into(),
            x: 0.0,
            y: 0.0,
            radius: 100.0,
            intensity: 1.0,
            color: None,
            attached_token_id: Some("hero".into()),
            casts_shadows: true,
        };
        let positions: HashMap<String, Vec2> =
            [("hero".to_string(), Vec2::new(5000.0, 0.0))].into();
        let placed = [Placed {
            position: effective_light_position(&torch, &positions),
            light: &torch,
        }];
        let near_origin = Rect {
            min: Vec2::splat(-200.0),
            max: Vec2::splat(200.0),
        };
        assert!(
            visible_lights(&placed, Some(near_origin)).is_empty(),
            "the torch went with its token"
        );
    }
}
