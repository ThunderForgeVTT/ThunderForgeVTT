//! Sizing token art inside its grid footprint.
//!
//! A token's footprint is square — one cell, or N cells for a larger
//! creature — and its on-screen size is derived from the grid so that
//! importing a map with a different `pixels_per_grid` resizes every token
//! with it. Art is not square, though. Stretching a 2048x924 starship into
//! a square makes it a squat blob, and stretching a portrait bust makes it
//! a fat one.
//!
//! So the footprint is a *box* to fit within, not a size to assume: the art
//! keeps its aspect ratio and touches the box on its longer axis. A round
//! token, which is the common case, is square already and fills the box
//! exactly — the fit is a no-op for it and only earns its keep on art that
//! isn't.

use glam::Vec2;

/// The on-screen size for art of `image` pixel dimensions occupying a
/// square footprint of `side` world units.
///
/// Preserves aspect ratio, touching the footprint on the longer axis.
/// Falls back to the full square when the dimensions are unusable — zero,
/// negative or non-finite — because a token that renders at the wrong
/// aspect is a cosmetic problem, while one that renders at zero size has
/// silently vanished from the board.
pub fn fit_within_footprint(side: f32, image: Vec2) -> Vec2 {
    let square = Vec2::splat(side);

    if !image.x.is_finite() || !image.y.is_finite() || image.x <= 0.0 || image.y <= 0.0 {
        return square;
    }

    let scale = (side / image.x).min(side / image.y);
    image * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The common case: round token art is square and fills its cell, so
    /// this must not change what already worked.
    #[test]
    fn square_art_fills_the_footprint_exactly() {
        assert_eq!(
            fit_within_footprint(64.0, Vec2::new(128.0, 128.0)),
            Vec2::new(64.0, 64.0)
        );
    }

    #[test]
    fn wide_art_touches_the_sides_and_is_letterboxed() {
        // A 2048x924 starship in a 64-unit cell: full width, proportional
        // height, and emphatically not a 64x64 square.
        let size = fit_within_footprint(64.0, Vec2::new(2048.0, 924.0));
        assert_eq!(size.x, 64.0);
        assert!((size.y - 28.875).abs() < 0.001, "got {size:?}");
    }

    #[test]
    fn tall_art_touches_the_top_and_bottom() {
        let size = fit_within_footprint(64.0, Vec2::new(512.0, 1024.0));
        assert_eq!(size.y, 64.0);
        assert_eq!(size.x, 32.0);
    }

    #[test]
    fn aspect_ratio_is_preserved() {
        let image = Vec2::new(1920.0, 1080.0);
        let size = fit_within_footprint(100.0, image);
        assert!(
            (size.x / size.y - image.x / image.y).abs() < 0.0001,
            "got {size:?}"
        );
    }

    #[test]
    fn art_never_exceeds_its_footprint() {
        for image in [
            Vec2::new(2048.0, 924.0),
            Vec2::new(1.0, 4096.0),
            Vec2::new(4096.0, 1.0),
            Vec2::new(37.0, 41.0),
        ] {
            let size = fit_within_footprint(64.0, image);
            assert!(
                size.x <= 64.0 + 0.0001 && size.y <= 64.0 + 0.0001,
                "got {size:?}"
            );
        }
    }

    #[test]
    fn a_larger_footprint_scales_the_art_up() {
        // Footprints are N cells for bigger creatures; the art follows.
        let small = fit_within_footprint(64.0, Vec2::new(2048.0, 924.0));
        let large = fit_within_footprint(128.0, Vec2::new(2048.0, 924.0));
        assert_eq!(large, small * 2.0);
    }

    /// Bevy reports an image's size before it has loaded, and a
    /// yet-to-load or broken image reports zero. Falling back to the full
    /// square keeps the token visible; scaling by it would make the token
    /// disappear.
    #[test]
    fn unusable_dimensions_fall_back_to_the_full_square() {
        for image in [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 128.0),
            Vec2::new(128.0, 0.0),
            Vec2::new(-128.0, 128.0),
            Vec2::new(f32::NAN, 128.0),
            Vec2::new(f32::INFINITY, 128.0),
        ] {
            assert_eq!(
                fit_within_footprint(64.0, image),
                Vec2::new(64.0, 64.0),
                "for {image:?}"
            );
        }
    }
}
