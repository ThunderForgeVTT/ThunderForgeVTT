//! How a die comes to rest showing the server's face (research R3).

use glam::{Quat, Vec3};

use super::shapes::Solid;

/// The orientation that turns `face`'s outward normal toward the viewer
/// (`+z`), then spins the die by `spin` radians about `+z`. The spin cannot
/// move the face: it only varies how the landed die sits.
pub fn landing(solid: &Solid, face: usize, spin: f32) -> Quat {
    let to_viewer = Quat::from_rotation_arc(solid.normal(face), Vec3::Z);
    (Quat::from_rotation_z(spin) * to_viewer).normalize()
}

#[cfg(test)]
#[path = "landing_tests.rs"]
mod tests;
