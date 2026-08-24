//! Cypher System Server Package
//!
//! Backend implementation for Cypher System (research digest:
//! research/system_cypher_system.json). Mirrors packs/systems/dnd5e/server's
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
pub const SYSTEM_ID: &str = "cypher_system";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_constants() {
        assert_eq!(SYSTEM_ID, "cypher_system");
    }
}
