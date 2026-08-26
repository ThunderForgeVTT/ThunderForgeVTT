//! Which recently-used images are worth keeping resident on the GPU.
//!
//! Uploading a map's texture is the single most expensive thing a scene
//! switch does — measured at roughly 350ms per megapixel on one frame, so a
//! 14MP map freezes the engine for about five seconds. The upload happens
//! once per texture, but only for as long as something holds the image:
//! when a scene switch despawns the old background sprite, the last handle
//! to its image drops, the texture is freed, and switching back pays the
//! full cost again. Measured: 5825ms to open a map, 5056ms to return to it
//! after one switch away, and 20ms when the texture was never released.
//!
//! Holding onto them is therefore worth real time, and costs real memory:
//! an uncompressed 21MP texture is about 85MB. So retention is bounded by
//! total pixels rather than a count, because "three maps" means 12MB or
//! 250MB depending entirely on which three.
//!
//! This module decides *what* to keep. The engine crate holds the actual
//! asset handles (`resources/background_cache.rs`).

/// Pixels worth of background textures to keep resident.
///
/// 48 megapixels is about 192MB as uncompressed RGBA — roughly two of the
/// largest maps in the example corpus, or many small ones. Chosen to make
/// flipping between two scenes free, which is the common case in play,
/// without holding a whole campaign's art on the GPU.
pub const DEFAULT_BUDGET_PIXELS: u64 = 48_000_000;

/// How many of `pixels_most_recent_first` to keep.
///
/// Entries are ordered most-recently-used first; the return value is a
/// count taken from the front, so the caller truncates. The most recent
/// entry is always kept even when it alone exceeds the budget: it is the
/// background currently on screen, and evicting it would free the texture
/// being drawn and re-upload it immediately.
pub fn retain_within_budget(pixels_most_recent_first: &[u64], budget_pixels: u64) -> usize {
    let mut total: u64 = 0;

    for (index, pixels) in pixels_most_recent_first.iter().enumerate() {
        total = total.saturating_add(*pixels);

        if total > budget_pixels {
            // Keep at least the current background regardless of size.
            return index.max(1).min(pixels_most_recent_first.len());
        }
    }

    pixels_most_recent_first.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fitting_the_budget_is_kept() {
        assert_eq!(retain_within_budget(&[10, 10, 10], 100), 3);
    }

    #[test]
    fn the_budget_boundary_is_inclusive() {
        // Exactly the budget fits; one pixel more does not.
        assert_eq!(retain_within_budget(&[50, 50], 100), 2);
        assert_eq!(retain_within_budget(&[50, 51], 100), 1);
    }

    #[test]
    fn the_oldest_are_dropped_first() {
        // Most-recent-first ordering means truncating the tail drops the
        // least recently used, which is the whole point of the ordering.
        assert_eq!(retain_within_budget(&[40, 40, 40, 40], 100), 2);
    }

    #[test]
    fn the_current_background_is_kept_even_when_it_alone_busts_the_budget() {
        // Evicting it would free the texture being drawn this frame and
        // immediately re-upload it — strictly worse than keeping it.
        assert_eq!(retain_within_budget(&[500], 100), 1);
        assert_eq!(retain_within_budget(&[500, 10], 100), 1);
    }

    #[test]
    fn an_empty_cache_retains_nothing() {
        assert_eq!(retain_within_budget(&[], 100), 0);
    }

    #[test]
    fn a_zero_budget_still_keeps_the_current_background() {
        assert_eq!(retain_within_budget(&[10, 10], 0), 1);
    }

    #[test]
    fn the_default_budget_holds_two_of_the_largest_example_maps() {
        // 6144x3456 is the largest in the example corpus, and flipping
        // between two such scenes is the case this budget exists to make
        // free.
        let largest = 6144 * 3456;
        assert_eq!(
            retain_within_budget(&[largest, largest], DEFAULT_BUDGET_PIXELS),
            2
        );
        assert_eq!(
            retain_within_budget(&[largest, largest, largest], DEFAULT_BUDGET_PIXELS),
            2
        );
    }

    #[test]
    fn totals_do_not_overflow_on_absurd_inputs() {
        assert_eq!(retain_within_budget(&[u64::MAX, u64::MAX], u64::MAX), 2);
    }
}
