//! The light a map was drawn in — playtest 2026-09-10 P9.
//!
//! A UVTT file records its map's ambient light as a colour: `AARRGGBB` hex,
//! which Dungeondraft writes as opaque white, `ffffffff`, for daylight. A scene
//! has three levels rather than a colour, because three are what the lighting
//! layer draws (`plugins/darkness.rs`) and what a Game Master picks between.
//! So the colour's brightness chooses the level: a night map exported in dark
//! blue imports dark, a candle-lit hall dim, and anything near white — which
//! is nearly every map — bright, rendering exactly as it always has.

use super::types::UvttEnvironment;

/// Brightness at or above which a map reads as daylight. Warm whites like
/// `fff7e4` sit well above it.
const BRIGHT_AT: f64 = 0.75;
/// Brightness at or above which a map is dim rather than dark.
const DIM_AT: f64 = 0.35;

/// The scene's `ambient_light` for a map with this environment: `"bright"`,
/// `"dim"` or `"dark"`. A file that records nothing, or something that is not
/// a colour, is bright — a typo must not plunge a map into darkness.
pub(super) fn ambient_level(environment: &UvttEnvironment) -> &'static str {
    match environment.ambient_light.as_deref().and_then(brightness) {
        Some(b) if b < DIM_AT => "dark",
        Some(b) if b < BRIGHT_AT => "dim",
        _ => "bright",
    }
}

/// Perceived brightness of an `AARRGGBB` or `RRGGBB` colour, 0–1.
///
/// Alpha is ignored: exporters write it opaque, and reading a transparent
/// ambient as "no light" would turn a malformed file into a black map.
fn brightness(hex: &str) -> Option<f64> {
    let hex = hex.trim().trim_start_matches('#');
    if !hex.is_ascii() {
        return None;
    }
    let rgb = match hex.len() {
        8 => &hex[2..],
        6 => hex,
        _ => return None,
    };
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&rgb[range], 16)
            .ok()
            .map(|v| f64::from(v) / 255.0)
    };
    let (r, g, b) = (channel(0..2)?, channel(2..4)?, channel(4..6)?);
    Some(0.2126 * r + 0.7152 * g + 0.0722 * b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(value: Option<&str>) -> &'static str {
        ambient_level(&UvttEnvironment {
            baked_lighting: false,
            ambient_light: value.map(str::to_string),
        })
    }

    #[test]
    fn daylight_and_warm_white_are_bright() {
        assert_eq!(level(None), "bright");
        assert_eq!(level(Some("ffffffff")), "bright");
        // `little-fish-academy.dd2vtt`'s own value.
        assert_eq!(level(Some("fffff7e4")), "bright");
    }

    #[test]
    fn a_dusky_map_is_dim_and_a_night_map_dark() {
        assert_eq!(level(Some("ff8a7a60")), "dim");
        assert_eq!(level(Some("ff3a3a3a")), "dark");
        assert_eq!(level(Some("#1a2233")), "dark");
    }

    #[test]
    fn anything_that_is_not_a_colour_is_bright() {
        assert_eq!(level(Some("")), "bright");
        assert_eq!(level(Some("night")), "bright");
        assert_eq!(level(Some("ffzzzzzz")), "bright");
        assert_eq!(level(Some("ffé0000")), "bright");
    }
}
