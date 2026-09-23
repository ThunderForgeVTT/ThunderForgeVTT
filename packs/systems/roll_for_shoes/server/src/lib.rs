//! Roll for Shoes, server side.
//!
//! # Why this crate exists
//!
//! The pack contract calls a `server/` crate optional, and it is — for a pack
//! that only *declares*. Roll for Shoes stores two things, XP and a skill
//! lineage, and every write of an actor's system data is validated through the
//! registry, which refuses a system it does not hold. The registry is filled
//! only from `inventory`-submitted contributions. So without this crate the
//! sheet could draw the game but could never save it.
//!
//! That is the whole job. There is no table, no migration, no GraphQL, and
//! nothing here touches the database — a roll goes down the same path every
//! other system's roll takes.

pub mod validators;

#[cfg(test)]
mod validators_tests;

pub use validators::{
    validate_resource_data, validate_resource_data_for_registry, validate_trait_data,
    validate_trait_data_for_registry, ValidationError,
};

/// Roll for Shoes System Version
pub const VERSION: &str = "0.1.0";

/// System ID for registration. Matches the manifest's `id`, which is also the
/// pack's directory name.
pub const SYSTEM_ID: &str = "roll_for_shoes";

/// The skill every character begins with, as the manifest declares it.
///
/// Read from the `system.json` compiled in beside this crate rather than
/// written twice in Rust, so the manifest stays the authority on it. The
/// pack's web module states the same two values as its own constant, because
/// the host gives a sheet no way to read a manifest — that duplication is
/// recorded and accepted in the spec's research (D6), and the test below is
/// what keeps this half of it honest.
pub fn starting_skill() -> (String, i64) {
    static DECLARED: std::sync::OnceLock<(String, i64)> = std::sync::OnceLock::new();
    DECLARED
        .get_or_init(|| {
            let manifest =
                serde_json::from_str::<serde_json::Value>(include_str!("../../system.json")).ok();
            let declared = manifest.as_ref().and_then(|m| m.get("startingSkill"));
            let name = declared
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("Do Anything")
                .to_string();
            let level = declared
                .and_then(|s| s.get("level"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            (name, level)
        })
        .clone()
}

// This pack declares what it contributes; nothing in shared server code names
// it, lists it, or wires these validators (spec 032, FR-029).
//
// Every other field is absent, and each absence is the ruleset's answer rather
// than an omission: the game has no attributes, no proficiencies and no magic
// system, and it derives nothing — a skill's dice pool *is* its level, and XP
// is a stored count.
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        resource_data: Some(validators::validate_resource_data_for_registry),
        trait_data: Some(validators::validate_trait_data_for_registry),
        ..thunderforge_canvas_core::system_contribution::SystemContribution::new(SYSTEM_ID)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_the_directory_name() {
        assert_eq!(SYSTEM_ID, "roll_for_shoes");
        assert_eq!(VERSION, "0.1.0");
    }

    /// The manifest's starting skill must be at level 1, because that is what
    /// the trait validator enforces of a character's root skill (T11). If the
    /// manifest ever said otherwise, every character created from it would be
    /// refused on first save.
    #[test]
    fn the_declared_starting_skill_is_one_the_validator_would_accept() {
        let (name, level) = starting_skill();
        assert!(!name.trim().is_empty());
        assert_eq!(level, 1);

        let seeded = serde_json::json!({ "skills": [
            { "id": "s1", "name": name, "level": level, "parentId": null },
        ]});
        assert!(validate_trait_data(&seeded).is_ok());
    }
}
