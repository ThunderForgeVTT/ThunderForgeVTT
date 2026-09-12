//! Resolving each token's sight from its system's declaration (spec 045 US6).
//!
//! The server owns this rather than the web, for the reason every other
//! declared value is resolved here: the alternative is shipping each client
//! the whole manifest and every actor's sheet, and asking it to agree with
//! everybody else about what they mean.
//!
//! Split `_for_system` from `_from_manifest` in the same way `attributes.rs`
//! does, so the resolution can be tested without a filesystem.

use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::vision_declaration::{
    ActorSlotData, ResolvedVision, VisionDeclaration, vision_from,
};

use crate::declared_values::ActorSlots;

/// Read a system's `vision` block off disk. Absent, unreadable or malformed
/// all mean the same thing: the system declares no vision, and its tokens see
/// by the default rules (FR-066).
pub fn vision_declaration_for_system(systems_dir: &str, system_id: &str) -> VisionDeclaration {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return VisionDeclaration::default();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return VisionDeclaration::default();
    };
    vision_from_manifest(&manifest)
}

/// The same, from a manifest already in hand.
///
/// A `vision` block that fails to parse is treated as absent rather than as an
/// error. Install-time validation is where a malformed block is reported to
/// the person who wrote it (`pack_system_spec`); by the time a scene is being
/// drawn, refusing to show anybody anything would be the worse answer.
pub fn vision_from_manifest(manifest: &serde_json::Value) -> VisionDeclaration {
    manifest
        .get("vision")
        .and_then(|block| serde_json::from_value::<VisionDeclaration>(block.clone()).ok())
        .unwrap_or_default()
}

/// An actor's stored slots, in the shape the shared resolver reads.
///
/// The names match the manifest's own vocabulary (`traitData`, not
/// `trait_data`), because a declaration written by a pack author names the
/// slot and the author is looking at their own JSON.
pub fn slot_data(slots: &ActorSlots) -> ActorSlotData {
    let mut out = ActorSlotData::new();
    for (name, value) in [
        ("abilityData", &slots.ability_data),
        ("resourceData", &slots.resource_data),
        ("proficiencyData", &slots.proficiency_data),
        ("traitData", &slots.trait_data),
    ] {
        if let Some(value) = value {
            out.insert(name.to_string(), value.clone());
        }
    }
    out
}

/// One token's resolved sight, in cells.
pub fn resolve(slots: &ActorSlots, declaration: &VisionDeclaration) -> ResolvedVision {
    vision_from(&slot_data(slots), declaration, &declaration.grid_units())
}

/// A distance in cells, as the engine wants it: world units.
///
/// The scene's `grid_size` is pixels per cell — which is the *only* thing it
/// has ever been, despite reading like a measurement — so this is the one
/// place cells become something drawable.
pub fn cells_to_world(cells: f32, grid_size: i32) -> f32 {
    if cells <= 0.0 {
        return 0.0;
    }
    cells * grid_size.max(1) as f32
}

/// The scale a system's distances are quoted in, for anything that shows one.
pub fn units_of(declaration: &VisionDeclaration) -> GridUnits {
    declaration.grid_units()
}

#[cfg(test)]
#[path = "vision_profiles_tests.rs"]
mod tests;
