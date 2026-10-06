//! A read-only window onto the sight row (spec 076 FR-013, FR-014).
//!
//! The darkness sheet decides on the GPU what the viewer cannot see, and a
//! test cannot read a fragment shader. So `sync_darkness` mirrors the sight
//! row it uploads here — the viewer's position, the row's reach and its
//! texels, exactly as the shader will read them — and `sight_probe` answers
//! the shader's question for one point with the shader's own arithmetic:
//! which bin the point falls in, how far the row reaches there, and whether
//! the point is within it. The same tolerance as `darkness.wgsl`'s `fragment`,
//! so the two never disagree about a point on the line.

use std::f32::consts::{PI, TAU};
use std::sync::{Mutex, OnceLock};

use bevy::math::Vec2;

use super::darkness::SHADOW_BINS;

/// What the shader reads for sight: where the viewer stands, how far the row
/// reaches, and `SHADOW_BINS` texels of four bytes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SightRow {
    pub origin: Vec2,
    pub reach: f32,
    pub texels: Vec<u8>,
}

static SIGHT: OnceLock<Mutex<Option<SightRow>>> = OnceLock::new();

/// Record the sight row as uploaded, or `None` when nobody looks through a
/// token and the shader bounds nothing.
pub(crate) fn mirror_sight(row: Option<SightRow>) {
    let slot = SIGHT.get_or_init(|| Mutex::new(None));
    if let Ok(mut current) = slot.lock()
        && *current != row
    {
        *current = row;
    }
}

/// `darkness.wgsl`'s `reach`: how far the row lets sight travel towards
/// `offset`, in world units, from the packed 16-bit fraction in red and green.
pub(crate) fn unpack_reach(row: &SightRow, offset: Vec2) -> f32 {
    let angle = offset.y.atan2(offset.x);
    let bin = (((angle + PI) / TAU * SHADOW_BINS as f32).floor() as usize).min(SHADOW_BINS - 1);
    let texel = &row.texels[bin * 4..bin * 4 + 4];
    let packed = f32::from(texel[0]) * 256.0 + f32::from(texel[1]);
    packed / 65535.0 * row.reach
}

/// Whether the sheet shows the world point `(x, y)` to the viewer: `seen` is
/// false where the shader draws the unseen tint. With nobody looking through
/// a token there is no sheet to hide anything, so everything is seen.
pub(crate) fn sight_at(row: Option<&SightRow>, point: Vec2) -> bool {
    match row {
        None => true,
        Some(row) => {
            let offset = point - row.origin;
            // The same +1.0 as `fragment`, so a point on the wall itself is
            // seen rather than left to rounding.
            offset.length() <= unpack_reach(row, offset) + 1.0
        }
    }
}

/// `{ "looking": bool, "seen": bool }` for a world point: whether this canvas
/// looks through a token at all, and whether that token sees the point.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn sight_probe(x: f32, y: f32) -> String {
    let row = SIGHT
        .get()
        .and_then(|slot| slot.lock().ok().map(|row| row.clone()))
        .unwrap_or_default();
    serde_json::json!({
        "looking": row.is_some(),
        "seen": sight_at(row.as_ref(), Vec2::new(x, y)),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row reaching 100 in every direction but east, where it stops at 40.
    fn row() -> SightRow {
        let mut texels = [255u8, 255, 0, 255].repeat(SHADOW_BINS);
        let east = SHADOW_BINS / 2;
        let packed = (0.4f32 * 65535.0).round() as u16;
        texels[east * 4] = (packed >> 8) as u8;
        texels[east * 4 + 1] = (packed & 0xff) as u8;
        SightRow {
            origin: Vec2::new(10.0, 10.0),
            reach: 100.0,
            texels,
        }
    }

    #[test]
    fn the_probe_reads_the_row_the_way_the_shader_does() {
        let row = row();
        assert!(
            sight_at(Some(&row), Vec2::new(10.0 + 39.0, 10.0)),
            "short of the wall"
        );
        assert!(
            !sight_at(Some(&row), Vec2::new(10.0 + 60.0, 10.0)),
            "behind the wall"
        );
        assert!(
            sight_at(Some(&row), Vec2::new(10.0, 10.0 + 60.0)),
            "north, no wall"
        );
        assert!(
            !sight_at(Some(&row), Vec2::new(10.0, 10.0 + 160.0)),
            "past the reach"
        );
        assert!(
            sight_at(None, Vec2::new(1e6, 1e6)),
            "nobody looking sees everything"
        );
    }

    #[test]
    fn the_export_says_whether_anyone_is_looking() {
        mirror_sight(None);
        assert_eq!(sight_probe(0.0, 0.0), r#"{"looking":false,"seen":true}"#);
        mirror_sight(Some(row()));
        assert_eq!(sight_probe(70.0, 10.0), r#"{"looking":true,"seen":false}"#);
        mirror_sight(None);
    }
}
