//! Gizmo overlay for lighting and vision.
//!
//! Lighting is the one subsystem whose behaviour is almost impossible to
//! confirm from its output alone: a token that is hidden looks identical
//! whether it was occluded by a wall, outside a vision cone, or simply
//! unlit — and identical again to a token that is not there. Drawing the
//! inputs makes the difference legible.
//!
//! Each light is drawn as two rings (bright core, dim edge) tinted with its
//! own colour, and each token carrying a vision cone gets its cone drawn.
//! GM-facing and off by default; toggled by the `set_lighting_overlay`
//! command.

use bevy::prelude::*;

use crate::resources::{LightSet, LightingOverlay, TokenVision};
use crate::TokenIdentity;
use thunderforge_canvas_core::vision::Rgb;

/// Segments per circle. Enough to read as round at play zoom without
/// flooding the gizmo buffer when a scene has many lights.
const CIRCLE_SEGMENTS: u32 = 48;

/// Arc segments across a vision cone.
const CONE_SEGMENTS: u32 = 24;

pub struct LightingOverlayPlugin;

impl Plugin for LightingOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightingOverlay>()
            .add_systems(Update, (draw_light_radii, draw_vision_cones));
    }
}

fn to_color(rgb: Rgb, alpha: f32) -> Color {
    Color::srgba(rgb.r, rgb.g, rgb.b, alpha)
}

fn draw_light_radii(
    overlay: Res<LightingOverlay>,
    light_set: Res<LightSet>,
    mut gizmos: Gizmos,
) {
    if !overlay.0 {
        return;
    }

    for light in light_set.lights() {
        // Lights with zero intensity are off; drawing their rings would
        // suggest illumination that is not there.
        if light.intensity <= 0.0 {
            continue;
        }

        let position = light.position();
        let color = light
            .color
            .as_deref()
            .and_then(Rgb::parse_hex)
            .unwrap_or(Rgb::WHITE);

        // Mirrors `systems::lighting::resolve_light`'s mapping of the stored
        // single radius onto a bright core and dim edge.
        let dim = light.radius;
        let bright = light.radius * 0.5;

        gizmos
            .circle_2d(position, bright, to_color(color, 0.55))
            .resolution(CIRCLE_SEGMENTS);
        gizmos
            .circle_2d(position, dim, to_color(color, 0.25))
            .resolution(CIRCLE_SEGMENTS);
    }
}

fn draw_vision_cones(
    overlay: Res<LightingOverlay>,
    tokens: Query<(&Transform, &TokenVision), With<TokenIdentity>>,
    mut gizmos: Gizmos,
) {
    if !overlay.0 {
        return;
    }

    for (transform, vision) in tokens.iter() {
        let origin = transform.translation.truncate();

        // Darkvision is omnidirectional and has no cone to draw, but its reach
        // is worth seeing — a dashed-looking ring at low alpha.
        if vision.darkvision > 0.0 {
            gizmos
                .circle_2d(origin, vision.darkvision, Color::srgba(0.4, 0.6, 1.0, 0.35))
                .resolution(CIRCLE_SEGMENTS);
        }

        let Some(facing) = vision.facing else {
            continue;
        };
        if vision.fov >= std::f32::consts::TAU {
            continue;
        }

        // A cone needs a length to draw. Prefer an explicit sight limit; fall
        // back to darkvision, then to a nominal reach — the shape is what
        // matters here, not the exact extent.
        let reach = vision
            .max_range
            .or(if vision.darkvision > 0.0 { Some(vision.darkvision) } else { None })
            .unwrap_or(300.0);

        let half = vision.fov / 2.0;
        let color = Color::srgba(1.0, 0.95, 0.6, 0.4);

        // The two edges, then the arc between them, so the cone reads as a
        // wedge rather than two loose lines.
        let left = origin + Vec2::from_angle(facing - half) * reach;
        let right = origin + Vec2::from_angle(facing + half) * reach;
        gizmos.line_2d(origin, left, color);
        gizmos.line_2d(origin, right, color);

        let arc: Vec<Vec2> = (0..=CONE_SEGMENTS)
            .map(|i| {
                let t = i as f32 / CONE_SEGMENTS as f32;
                let angle = facing - half + vision.fov * t;
                origin + Vec2::from_angle(angle) * reach
            })
            .collect();
        gizmos.linestrip_2d(arc, color);
    }
}
