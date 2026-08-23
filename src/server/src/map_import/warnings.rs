//! Import-result warning builders — disclose UVTT source-file field
//! categories that were parsed but not applied downstream (User Story 3,
//! research.md §5-6).

use super::types::{UvttEnvironment, UvttPoint, UvttPortal};

/// T021: a `freestanding: true` portal has no attaching wall/door
/// geometry of its own in this importer (`walls_from_portals` builds a
/// wall from every portal's `bounds` regardless of `freestanding`, so the
/// portal itself is never dropped) — but a freestanding portal is
/// conceptually "not attached to a wall" per the source format, which is
/// the gap this warning discloses (research.md §6).
pub(super) fn freestanding_portal_warning(portals: &[UvttPortal]) -> Option<String> {
    let count = portals.iter().filter(|p| p.freestanding).count();
    if count == 0 {
        return None;
    }
    Some(format!(
        "{count} freestanding portal{plural} present in the source file; freestanding portals are not attached to wall geometry and may not appear as expected",
        plural = if count == 1 { "" } else { "s" }
    ))
}

/// DungeonDraft's own exporter default (fully-opaque white, i.e. "no
/// ambient tint") — every real-world fixture surveyed for this feature
/// sets `ambient_light` to exactly this value except
/// `little-fish-academy.dd2vtt`'s deliberate non-default
/// `"fffff7e4"` (data-model.md). Warning on every file regardless of
/// value would violate FR-014's "no new noise for the common case", so
/// this only fires when the value differs from the exporter's default.
const DEFAULT_AMBIENT_LIGHT: &str = "ffffffff";

/// T022: a non-default `ambient_light` is parsed but not applied to
/// scene lighting today (research.md §5-6).
pub(super) fn ambient_light_warning(environment: &UvttEnvironment) -> Option<String> {
    environment
        .ambient_light
        .as_ref()
        .filter(|value| value.as_str() != DEFAULT_AMBIENT_LIGHT)
        .map(|value| {
            format!("ambient_light (\"{value}\") was present in the source file but is not yet applied to scene lighting")
        })
}

/// T023: `objects_line_of_sight` (occluders attached to placeable
/// objects, distinct from the static `line_of_sight` walls) has no
/// vision-blocking geometry created from it today — it's merged into
/// ordinary walls by `import_uvtt_impl` for backward-compatible
/// behavior, but that merge itself is the thing worth disclosing since
/// object-attached occluders are conceptually different from static
/// walls.
pub(super) fn objects_line_of_sight_warning(polygons: &[Vec<UvttPoint>]) -> Option<String> {
    if polygons.is_empty() {
        return None;
    }
    Some(format!(
        "{count} objects_line_of_sight occluder polygon{plural} present in the source file; object-attached vision-blocking geometry is imported as ordinary static walls, not as object-linked occluders",
        count = polygons.len(),
        plural = if polygons.len() == 1 { "" } else { "s" }
    ))
}
