//! Scene lighting conditions and per-token eyes.
//!
//! Thin Bevy wrappers over `thunderforge_canvas_core::vision`, following this
//! crate's convention of keeping tested logic in the pure core.

use bevy::prelude::*;
use thunderforge_canvas_core::vision::{AmbientLight, VisionProfile};

/// The scene's baseline illumination — daylight outdoors, dark in a dungeon.
///
/// Defaults to `Bright`, not `Dark`. A scene that has configured nothing must
/// render normally rather than going black: FR-013's rule that a scene using
/// none of these capabilities still has to work. Darkness is opt-in.
#[derive(Resource, Clone, Copy, Debug, Deref, DerefMut)]
pub struct SceneAmbient(pub AmbientLight);

impl Default for SceneAmbient {
    fn default() -> Self {
        Self(AmbientLight::daylight())
    }
}

/// One token's eyes. Absent means unaided, omnidirectional sight.
#[derive(Component, Clone, Copy, Debug, Default, Deref, DerefMut)]
pub struct TokenVision(pub VisionProfile);

/// Whether the lighting/vision debug overlay is drawn (light radii, vision
/// cones). GM-facing diagnostic, off by default.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct LightingOverlay(pub bool);
