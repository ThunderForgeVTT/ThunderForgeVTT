//! A system's `sheetImport` declaration, read from its manifest.

use thunderforge_sheet_import::{MappingError, SheetMapping};

/// The declaration of `system_id`, checked against its own manifest.
///
/// `Ok(None)` when the system declares none, or has no readable manifest:
/// it takes no sheets (`SYSTEM_HAS_NO_MAPPING`). A block that does not
/// check is an error, never a partial mapping.
pub fn mapping_for_system(
    systems_dir: &str,
    system_id: &str,
) -> Result<Option<SheetMapping>, MappingError> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(None);
    };
    SheetMapping::from_manifest(&manifest)
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
