//! Genie Server Package
//!
//! Backend implementation for the Genie house system (spec 018-genie-house-system).
//! Mirrors packs/systems/dnd5e/server's architecture: models.rs (base data), validators.rs
//! (system-specific JSONB validation), loader.rs (GraphQL registration).

pub mod loader;
pub mod models;
pub mod rules;
pub mod validators;

pub use loader::register_genie_mutations;
pub use models::{GenieAbilityData, GenieProficiencyData, GenieResourceData, GenieTraitData};
pub use rules::{GenieRules, WISH_POINTS_FOR_LEVEL};
pub use validators::{
    validate_ability_data, validate_ability_data_for_registry, validate_proficiency_data,
    validate_proficiency_data_for_registry, validate_resource_data,
    validate_resource_data_for_registry, validate_trait_data, validate_trait_data_for_registry,
    ValidationError,
};

/// Genie System Version
pub const VERSION: &str = "0.1.0";

/// System ID for registration
pub const SYSTEM_ID: &str = "genie";

// Spec 032 (FR-029): this pack declares what it contributes. Nothing in
// shared server code lists this system, names its id, or wires its
// validators — the server collects whatever contributions are linked in.
//
// Genie has no `spell_data` slot (it has no spellcasting) and reuses the
// `trait_data` slot for conditions/Patron/size_category (spec 018
// data-model.md — see validators.rs's doc comments).
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        ability_data: Some(validators::validate_ability_data_for_registry),
        resource_data: Some(validators::validate_resource_data_for_registry),
        proficiency_data: Some(validators::validate_proficiency_data_for_registry),
        trait_data: Some(validators::validate_trait_data_for_registry),
        rules: Some(|manifest| Box::new(crate::rules::GenieRules::from_manifest(manifest))),
        ..thunderforge_canvas_core::system_contribution::SystemContribution::new(SYSTEM_ID)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_constants() {
        assert_eq!(SYSTEM_ID, "genie");
        assert_eq!(VERSION, "0.1.0");
    }
}
