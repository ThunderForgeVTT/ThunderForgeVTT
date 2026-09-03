//! Whether a scene's grid still matches the background under it.
//!
//! # Why this exists
//!
//! A map background wider than the GPU texture cap is stored smaller than it
//! arrived. The importer used to keep the source file's `pixels_per_grid` as
//! the scene's `grid_size` anyway, so a 6144x3456 map became a 4096x2304
//! background under a 128px grid: thirty-two cells drawn across a map with
//! forty-eight, and every wall, portal and light out by the same 1.5x. It was
//! reported as "the grid is off", which is exactly what it looked like.
//!
//! The fix keeps the two in step. This module is the *check* — so that if they
//! ever come apart again, the product says so rather than leaving someone to
//! notice their squares look wrong.
//!
//! # Why the source map size is recorded at import
//!
//! Because without it the question is unanswerable. The stored width, height
//! and grid size of the worst affected scenes are **self-consistent**:
//! 4096/128 is exactly 32 and 2304/128 is exactly 18, so the row looks
//! perfectly healthy while being uniformly wrong. Only the source file knows
//! the map is 48 cells across, and the source file is not kept — the
//! transcoded WebP is.
//!
//! So [`record_source_map`] writes the file's own `map_size` into
//! `scenes.metadata` at import, and [`grid_mismatch`] compares against it.
//! That makes the check exact for anything imported from now on, and it is a
//! standing regression detector rather than a one-off migration aid.

use serde_json::{Value, json};

/// The metadata key this owns. Namespaced because `scenes.metadata` is shared.
const MAP_IMPORT_KEY: &str = "mapImport";

/// A scene's background is stored resized whenever the source exceeded the
/// texture cap, so a stored dimension landing *exactly* on the cap is the
/// fingerprint of the pre-fix code path.
///
/// A heuristic, and labelled as one wherever its answer is shown: a map whose
/// corrected size happens to land on the cap exactly would be a false
/// positive. It is only consulted for scenes imported before the source map
/// size was recorded, where the alternative is no answer at all.
const LEGACY_CAP: i32 = crate::storage::transcode::MAX_CANVAS_TEXTURE_DIMENSION as i32;

/// Merge the source file's grid facts into a scene's existing metadata.
///
/// Merged rather than replaced: `scenes.metadata` is a shared bag, and an
/// import must not throw away whatever else a scene is carrying.
pub fn record_source_map(
    existing: Option<Value>,
    map_cells_x: f64,
    map_cells_y: f64,
    pixels_per_grid: f64,
) -> Value {
    let mut root = match existing {
        Some(Value::Object(map)) => Value::Object(map),
        // A non-object metadata value is not something this can merge into
        // without destroying it, so it is replaced. Nothing in this product
        // writes one.
        _ => json!({}),
    };

    root[MAP_IMPORT_KEY] = json!({
        "sourceMapCellsX": map_cells_x,
        "sourceMapCellsY": map_cells_y,
        "sourcePixelsPerGrid": pixels_per_grid,
    });
    root
}

/// The map's size in cells, as the imported file stated it.
fn source_map_cells(metadata: Option<&Value>) -> Option<(f64, f64)> {
    let block = metadata?.get(MAP_IMPORT_KEY)?;
    let x = block.get("sourceMapCellsX")?.as_f64()?;
    let y = block.get("sourceMapCellsY")?.as_f64()?;
    (x > 0.0 && y > 0.0).then_some((x, y))
}

/// Why this scene's grid and background disagree, or `None` when they agree.
///
/// A sentence rather than a flag, because the useful thing to show someone is
/// the two numbers that differ. The rule lives here rather than in the client
/// so there is one of it.
pub fn grid_mismatch(
    has_background: bool,
    width: i32,
    height: i32,
    grid_size: i32,
    metadata: Option<&Value>,
) -> Option<String> {
    if !has_background {
        return None;
    }
    if grid_size <= 0 || width <= 0 || height <= 0 {
        return Some(
            "This scene's background is stored with an unusable grid size. \
             Re-import the map to correct it."
                .to_string(),
        );
    }

    if let Some((expected_x, expected_y)) = source_map_cells(metadata) {
        let actual_x = f64::from(width) / f64::from(grid_size);
        let actual_y = f64::from(height) / f64::from(grid_size);

        // Half a cell: tighter than any rounding this path can introduce, and
        // far looser than the 1.5x the bug produced.
        if (actual_x - expected_x).abs() >= 0.5 || (actual_y - expected_y).abs() >= 0.5 {
            return Some(format!(
                "This scene's grid does not match its background: the map is \
                 {expected_x:.0} x {expected_y:.0} squares, but the stored image \
                 covers {actual_x:.1} x {actual_y:.1}. Re-import the map to correct it."
            ));
        }
        return None;
    }

    // No recorded source size: imported before this was written down. The best
    // available answer is the fingerprint of the code path that got it wrong.
    if width == LEGACY_CAP || height == LEGACY_CAP {
        return Some(
            "This scene's background was resized when it was imported, and its \
             grid may not have been adjusted to match. Re-import the map to be sure."
                .to_string(),
        );
    }

    None
}

#[cfg(test)]
#[path = "alignment_tests.rs"]
mod tests;
