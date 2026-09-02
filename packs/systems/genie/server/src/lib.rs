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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_constants() {
        assert_eq!(SYSTEM_ID, "genie");
        assert_eq!(VERSION, "0.1.0");
    }
}
