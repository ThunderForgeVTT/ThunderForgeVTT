//! Tile-pyramid geometry.
//!
//! A battle map is too large to hand a GPU whole: WebGL2 only guarantees a
//! 2048px texture and 4096 is common on integrated hardware, while these maps
//! run to 6144px. Capping the image solves the upload failure but throws away
//! detail permanently. Tiling solves it without that loss — the map is cut
//! into GPU-sized pieces and only the ones on screen are ever resident.
//!
//! # The pyramid
//!
//! Level 0 is the full-resolution image. Each subsequent level halves both
//! dimensions, so level `n` is `1/2^n` scale. A viewer picks the level whose
//! pixels are closest to one screen pixel and loads tiles from it — which is
//! what makes zooming out cheap instead of catastrophic. Without levels, a
//! fully zoomed-out 6144px map means every tile resident at once.
//!
//! This is the same structure slippy maps and Deep Zoom use. It is well-trodden
//! precisely because the alternatives are worse.

use serde::{Deserialize, Serialize};

/// Side length of a square tile, in pixels.
///
/// 512 rather than 256: at 256 a 6144px map is 576 tiles at level 0, which is
/// a lot of HTTP requests and a lot of draw calls. At 512 it is 144. Larger
/// still (1024) would cut requests further but wastes bandwidth at the edges,
/// where a partial tile is mostly padding.
pub const TILE_SIZE: u32 = 512;

/// One tile's address within a pyramid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId {
    /// 0 is full resolution; each level is half the previous.
    pub level: u32,
    pub col: u32,
    pub row: u32,
}

/// The shape of one level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelInfo {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub cols: u32,
    pub rows: u32,
}

impl LevelInfo {
    pub fn tile_count(&self) -> u32 {
        self.cols * self.rows
    }
}

/// A full pyramid description for one image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pyramid {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub levels: Vec<LevelInfo>,
}

/// Number of tiles needed to cover `extent` pixels.
fn tiles_across(extent: u32, tile_size: u32) -> u32 {
    if extent == 0 {
        return 0;
    }
    extent.div_ceil(tile_size)
}

impl Pyramid {
    /// Builds the pyramid description for an image of `width` x `height`.
    ///
    /// Levels continue until the whole image fits in a single tile — going
    /// further would produce levels smaller than one tile, which no viewer can
    /// use and which cost a request each.
    pub fn describe(width: u32, height: u32, tile_size: u32) -> Self {
        let tile_size = tile_size.max(1);
        let mut levels = Vec::new();
        let (mut level_width, mut level_height) = (width.max(1), height.max(1));
        let mut level = 0;

        loop {
            levels.push(LevelInfo {
                level,
                width: level_width,
                height: level_height,
                cols: tiles_across(level_width, tile_size),
                rows: tiles_across(level_height, tile_size),
            });

            if level_width <= tile_size && level_height <= tile_size {
                break;
            }

            // `max(1)` so an extremely lopsided image (8192x1) still shrinks
            // its long axis instead of looping forever on a zero-height level.
            level_width = (level_width / 2).max(1);
            level_height = (level_height / 2).max(1);
            level += 1;
        }

        Self {
            width,
            height,
            tile_size,
            levels,
        }
    }

    pub fn level(&self, level: u32) -> Option<&LevelInfo> {
        self.levels.get(level as usize)
    }

    /// Total tiles across every level — the pyramid's full storage cost.
    pub fn total_tiles(&self) -> u32 {
        self.levels.iter().map(LevelInfo::tile_count).sum()
    }

    /// The pixel rectangle a tile covers *within its own level*, clipped to
    /// that level's bounds.
    ///
    /// Edge tiles are partial: a 6144px level at 512 divides evenly, but
    /// 3456 does not — the bottom row is 384px tall. Returning the clipped
    /// rect rather than a full tile is what keeps a viewer from sampling
    /// past the image and picking up garbage along the seam.
    pub fn tile_rect(&self, tile: TileId) -> Option<(u32, u32, u32, u32)> {
        let level = self.level(tile.level)?;
        if tile.col >= level.cols || tile.row >= level.rows {
            return None;
        }
        let x = tile.col * self.tile_size;
        let y = tile.row * self.tile_size;
        let width = self.tile_size.min(level.width.saturating_sub(x));
        let height = self.tile_size.min(level.height.saturating_sub(y));
        Some((x, y, width, height))
    }

    /// The level whose pixels are closest to one screen pixel at `scale`.
    ///
    /// `scale` is world units per screen unit — the camera's zoom, where
    /// larger means zoomed out. At scale 4 the viewer shows four map pixels
    /// per screen pixel, so level 2 (quarter size) is the right choice, and
    /// loading level 0 would cost 16x the bytes for detail the screen cannot
    /// show.
    pub fn level_for_scale(&self, scale: f32) -> u32 {
        if !scale.is_finite() || scale <= 1.0 {
            return 0;
        }
        // log2 of the scale, since each level halves.
        let level = scale.log2().floor().max(0.0) as u32;
        level.min(self.levels.len().saturating_sub(1) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pyramid_halves_until_one_tile_covers_it() {
        let pyramid = Pyramid::describe(6144, 3456, 512);

        assert_eq!(pyramid.levels[0].width, 6144);
        assert_eq!(pyramid.levels[0].cols, 12); // 6144 / 512
        assert_eq!(pyramid.levels[0].rows, 7); // ceil(3456 / 512)

        // Each level halves.
        assert_eq!(pyramid.levels[1].width, 3072);
        assert_eq!(pyramid.levels[2].width, 1536);

        // The last level fits in one tile, and no further.
        let last = pyramid.levels.last().unwrap();
        assert!(last.width <= 512 && last.height <= 512);
        assert_eq!(last.tile_count(), 1);
    }

    #[test]
    fn tiling_costs_far_less_than_the_whole_image_at_low_zoom() {
        // The reason the pyramid exists: fully zoomed out, a viewer reads one
        // small level instead of the entire full-resolution image.
        let pyramid = Pyramid::describe(6144, 3456, 512);
        let full_res_tiles = pyramid.levels[0].tile_count();
        let zoomed_out = pyramid.level(pyramid.level_for_scale(8.0)).unwrap();

        assert!(
            zoomed_out.tile_count() * 8 < full_res_tiles,
            "level {} has {} tiles vs {full_res_tiles} at full res",
            zoomed_out.level,
            zoomed_out.tile_count(),
        );
    }

    #[test]
    fn edge_tiles_are_clipped_to_the_image() {
        // 3456 is not a multiple of 512 — the bottom row is 384px tall. A
        // viewer given a full 512 rect here would sample past the image.
        let pyramid = Pyramid::describe(6144, 3456, 512);
        let bottom_left = TileId {
            level: 0,
            col: 0,
            row: 6,
        };
        let (x, y, width, height) = pyramid.tile_rect(bottom_left).unwrap();

        assert_eq!((x, y), (0, 3072));
        assert_eq!(width, 512, "a full-width column should not be clipped");
        assert_eq!(height, 384, "3456 - 3072 = 384");
    }

    #[test]
    fn an_out_of_range_tile_has_no_rect() {
        let pyramid = Pyramid::describe(1024, 1024, 512);
        assert!(
            pyramid
                .tile_rect(TileId {
                    level: 0,
                    col: 2,
                    row: 0
                })
                .is_none()
        );
        assert!(
            pyramid
                .tile_rect(TileId {
                    level: 99,
                    col: 0,
                    row: 0
                })
                .is_none()
        );
    }

    #[test]
    fn level_selection_follows_zoom() {
        let pyramid = Pyramid::describe(6144, 3456, 512);

        // Zoomed in or 1:1 always wants full resolution.
        assert_eq!(pyramid.level_for_scale(0.5), 0);
        assert_eq!(pyramid.level_for_scale(1.0), 0);
        // Two map pixels per screen pixel -> half-size level.
        assert_eq!(pyramid.level_for_scale(2.0), 1);
        assert_eq!(pyramid.level_for_scale(4.0), 2);
        // Beyond the pyramid's depth, clamp to the smallest level rather than
        // requesting one that does not exist.
        assert_eq!(
            pyramid.level_for_scale(100_000.0),
            pyramid.levels.len() as u32 - 1,
        );
    }

    #[test]
    fn a_degenerate_image_still_produces_a_usable_pyramid() {
        // Long and thin: halving both axes would drive height to zero and,
        // without the clamp, never terminate.
        let pyramid = Pyramid::describe(8192, 1, 512);
        assert!(pyramid.levels.len() > 1);
        assert!(pyramid.levels.last().unwrap().tile_count() >= 1);

        // Zero is not a real image but must not hang or panic.
        let empty = Pyramid::describe(0, 0, 512);
        assert_eq!(empty.levels.len(), 1);
    }

    #[test]
    fn every_tile_in_a_level_has_a_rect_and_they_tile_the_level_exactly() {
        let pyramid = Pyramid::describe(1500, 1100, 512);
        let level = pyramid.level(0).unwrap();

        let mut covered = 0u64;
        for row in 0..level.rows {
            for col in 0..level.cols {
                let (_, _, width, height) =
                    pyramid.tile_rect(TileId { level: 0, col, row }).unwrap();
                covered += u64::from(width) * u64::from(height);
            }
        }

        // No gaps and no overlap: the tiles account for exactly the level's
        // area.
        assert_eq!(covered, u64::from(level.width) * u64::from(level.height));
    }
}
