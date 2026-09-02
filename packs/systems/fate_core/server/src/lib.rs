//! Fate Core Server Package
//!
//! Backend implementation for Fate Core (research digest:
//! research/system_fate_core.json). Mirrors packs/systems/dnd5e/server's
//! architecture: models.rs (base data), validators.rs (JSONB validation),
//! loader.rs (GraphQL registration).

pub mod loader;
pub mod models;
pub mod validators;

pub use loader::register_mutations;
pub use models::{AbilityData, ProficiencyData, ResourceData, TraitData};
pub use validators::{
    validate_ability_data, validate_ability_data_for_registry, validate_proficiency_data,
    validate_proficiency_data_for_registry, validate_resource_data,
    validate_resource_data_for_registry, validate_trait_data, validate_trait_data_for_registry,
    ValidationError,
};

pub const VERSION: &str = "0.1.0";
pub const SYSTEM_ID: &str = "fate_core";

// Spec 032 (FR-029): this pack declares what it contributes. Nothing in
// shared server code lists this system, names its id, or wires its
// validators — the server collects whatever contributions are linked in.
//
// Fate has no fixed ability scores (research.md); `ability_data` still
// validates — validators.rs accepts any object — so an empty ability_data
// block is always valid. There is no `spell_data` slot.
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        ability_data: Some(validators::validate_ability_data_for_registry),
        resource_data: Some(validators::validate_resource_data_for_registry),
        proficiency_data: Some(validators::validate_proficiency_data_for_registry),
        trait_data: Some(validators::validate_trait_data_for_registry),
        ..thunderforge_canvas_core::system_contribution::SystemContribution::new("fate_core")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_constants() {
        assert_eq!(SYSTEM_ID, "fate_core");
    }
}
