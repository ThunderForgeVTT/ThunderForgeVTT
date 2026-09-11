//! Import-result warning builders — disclose UVTT source-file field
//! categories that were parsed but not applied downstream (User Story 3,
//! research.md §5-6).

use super::types::{UvttPoint, UvttPortal};

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

// T022's `ambient_light` warning is gone: the file's ambient light is now
// applied to the scene (playtest 2026-09-10 P9, `ambient.rs`), so there is
// nothing left to disclose about it.

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
