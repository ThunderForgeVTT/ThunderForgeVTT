//! D&D 5e Server Package
//!
//! Backend implementation for D&D 5e ruleset for ThunderForgeVTT.
//! Provides data models, SRD reference data, validators, and GraphQL mutation registration.
//!
//! ## Architecture
//!
//! - **models.rs**: Base data structures (ability scores, skills, items)
//!   - Stored in PostgreSQL
//!   - Only BASE stats, never derived data
//!
//! - **srd.rs**: SRD reference data (skill definitions, classes, spell slots)
//!   - Used for derived data calculations on engine/web
//!   - Never stored in database
//!
//! - **validators.rs**: Validation for system-specific JSONB data (Phase 4.8.1)
//!   - Validates ability_data, resource_data, proficiency_data, trait_data, spell_data
//!   - Manifest-driven schema from system.json
//!   - Zero database migrations when adding new systems
//!
//! - **loader.rs**: GraphQL registration (Phase 4.6 integration)
//!   - Called on server startup
//!   - Injects D&D 5e mutations into core router

pub mod loader;
pub mod models;
pub mod srd;
pub mod validators;

pub use loader::register_dnd5e_mutations;
pub use models::{AbilityScores, DnD5eActorData, DnD5eItemData, Proficiencies};
pub use srd::{get_class, get_skill, get_spell_slots};
pub use validators::{
    validate_ability_data, validate_ability_data_for_registry, validate_proficiency_data,
    validate_proficiency_data_for_registry, validate_resource_data,
    validate_resource_data_for_registry, validate_spell_data, validate_spell_data_for_registry,
    validate_trait_data, validate_trait_data_for_registry, ValidationError,
};

/// D&D 5e System Version
pub const VERSION: &str = "0.1.0";

/// System ID for registration
pub const SYSTEM_ID: &str = "dnd5e";

// Spec 032 (FR-029): this pack declares what it contributes. Nothing in
// shared server code lists this system, names its id, or wires its
// validators — the server collects whatever contributions are linked in.
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        ability_data: Some(validators::validate_ability_data_for_registry),
        resource_data: Some(validators::validate_resource_data_for_registry),
        proficiency_data: Some(validators::validate_proficiency_data_for_registry),
        trait_data: Some(validators::validate_trait_data_for_registry),
        spell_data: Some(validators::validate_spell_data_for_registry),
        ..thunderforge_canvas_core::system_contribution::SystemContribution::new("dnd5e")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_constants() {
        assert_eq!(SYSTEM_ID, "dnd5e");
        assert_eq!(VERSION, "0.1.0");
    }
}
