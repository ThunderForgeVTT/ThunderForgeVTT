//! Each die as a `Mesh2d`, drawn on the CPU (research R1).
//!
//! The engine has one orthographic `Camera2d` and no 3D pipeline, so a die is
//! a flat mesh rewritten each frame: its solid's vertices are rotated by the
//! die's current orientation, projected by dropping z, culled where the face
//! points away (every die is convex, so this is a correct hidden-face test),
//! and shaded by `n · L`. The mesh is written in place through
//! `Assets<Mesh>::get_mut`, never re-added, because per-frame inserts leak
//! (`systems/shape.rs`).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use thunderforge_canvas_core::dice_throw::shapes::Solid;
use thunderforge_canvas_core::dice_throw::tumble::DIE_PX;

/// The light, from the upper left and in front.
const LIGHT: Vec3 = Vec3::new(-0.42, 0.55, 0.72);
/// The least light a face gets, so a face edge-on to the light still reads.
const AMBIENT: f32 = 0.38;

/// A die's colour: ivory, or grey for a die that does not count.
pub fn base_colour(dimmed: bool) -> LinearRgba {
    if dimmed {
        LinearRgba::from(Color::srgba(0.55, 0.55, 0.58, 0.5))
    } else {
        LinearRgba::from(Color::srgb(0.95, 0.92, 0.84))
    }
}

/// A mesh with nothing in it yet, kept in the main world so it can be
/// rewritten.
pub fn empty() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

/// Rewrites `mesh` as `solid` turned by `rotation`, `DIE_PX` across, in
/// `colour` with `alpha` on top of the colour's own.
pub fn write(mesh: &mut Mesh, solid: &Solid, rotation: Quat, colour: LinearRgba, alpha: f32) {
    let radius = DIE_PX / 2.0;
    let light = LIGHT.normalize();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (face, f) in solid.faces.iter().enumerate() {
        let normal = rotation * solid.normal(face);
        if normal.z <= 1e-4 {
            continue;
        }
        let shade = AMBIENT + (1.0 - AMBIENT) * normal.dot(light).max(0.0);
        let rgba = [
            colour.red * shade,
            colour.green * shade,
            colour.blue * shade,
            colour.alpha * alpha,
        ];
        let start = positions.len() as u32;
        for &vertex in &f.indices {
            let p = rotation * solid.vertices[vertex] * radius;
            positions.push([p.x, p.y, 0.0]);
            colours.push(rgba);
        }
        for k in 1..f.indices.len().saturating_sub(1) as u32 {
            indices.extend_from_slice(&[start, start + k, start + k + 1]);
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
}

/// Where `face`'s centre is on screen, from the die's centre, when the die
/// is turned by `rotation`.
pub fn face_centre(solid: &Solid, face: usize, rotation: Quat) -> Vec2 {
    let c = rotation * solid.centroid(face) * (DIE_PX / 2.0);
    Vec2::new(c.x, c.y)
}
