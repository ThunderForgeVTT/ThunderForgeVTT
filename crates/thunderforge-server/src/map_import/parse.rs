//! UVTT JSON parsing (T023).

use super::types::{MapImportError, ParsedUvtt, SUPPORTED_FORMAT, UvttFile};

/// T023: parse and validate a raw UVTT JSON payload.
///
/// Rejects any `format` other than `0.3` outright. Skips (rather than
/// fails on) `line_of_sight`/`objects_line_of_sight` polygons with fewer
/// than 2 points, dropping them from the parsed document and reporting
/// how many were skipped.
pub fn parse_uvtt(raw: &[u8]) -> Result<ParsedUvtt, MapImportError> {
    let mut file: UvttFile = serde_json::from_slice(raw)?;

    if (file.format - SUPPORTED_FORMAT).abs() > f64::EPSILON {
        return Err(MapImportError::UnsupportedFormat { found: file.format });
    }

    let mut skipped = 0usize;

    file.line_of_sight.retain(|poly| {
        let keep = poly.len() >= 2;
        if !keep {
            skipped += 1;
        }
        keep
    });
    file.objects_line_of_sight.retain(|poly| {
        let keep = poly.len() >= 2;
        if !keep {
            skipped += 1;
        }
        keep
    });

    Ok(ParsedUvtt {
        file,
        skipped_degenerate_polygons: skipped,
    })
}
