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
//! That was the whole job until spec 062. The Extras — the optional rules the
//! game leaves to the table — are *per-world* settings, and a setting that
//! lives nowhere cannot vary by world. So the pack now also owns one table and
//! the two root fields that read and write it (`settings`, ADR-063 and
//! ADR-108). The validators below still touch no database; only `settings`
//! does, and a roll still goes down the same path every other system's takes.
//!
//! A second table followed for the same reason (`table`): what the Game
//! Master says has to be beaten is said to the whole table, and a number
//! that lives on each player's own sheet is not said to anyone.

pub mod settings;
pub mod table;
pub mod validators;

pub use settings::graphql::{RollForShoesSettingsMutation, RollForShoesSettingsQuery};
pub use table::graphql::{RollForShoesTableMutation, RollForShoesTableQuery};

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
pub fn starting_skills() -> Vec<(String, i64)> {
    static DECLARED: std::sync::OnceLock<Vec<(String, i64)>> = std::sync::OnceLock::new();
    DECLARED
        .get_or_init(|| {
            let manifest =
                serde_json::from_str::<serde_json::Value>(include_str!("../../system.json")).ok();
            let declared = manifest
                .as_ref()
                .and_then(|m| m.get("startingSkills"))
                .and_then(serde_json::Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            let name = entry.get("name").and_then(|n| n.as_str())?;
                            let level = entry.get("level").and_then(serde_json::Value::as_i64)?;
                            Some((name.to_string(), level))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // A manifest that declares none still means the core rule, not a
            // character with no skills. The two are never the same thing here,
            // and this is the one place the distinction is made in Rust.
            if declared.is_empty() {
                vec![("Do Anything".to_string(), 1)]
            } else {
                declared
            }
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

    /// The manifest's starting skill must be one the trait validator accepts of
    /// a character's root skill, or every character created from it would be
    /// refused on first save.
    ///
    /// Spec 062 loosened what that means: T11 once demanded exactly level 1,
    /// and now demands only a level of at least 1, because a world may declare
    /// its own starting skills. This test is deliberately written against the
    /// validator rather than against the number, so it keeps holding whichever
    /// of the two rules is in force.
    #[test]
    fn the_declared_starting_skill_is_one_the_validator_would_accept() {
        let declared = starting_skills();
        assert!(
            !declared.is_empty(),
            "an empty declaration must still mean the core rule"
        );

        let seeded = serde_json::json!({
            "skills": declared
                .iter()
                .enumerate()
                .map(|(index, (name, level))| serde_json::json!({
                    "id": format!("s{index}"),
                    "name": name,
                    "level": level,
                    "parentId": null,
                }))
                .collect::<Vec<_>>()
        });
        for (name, level) in &declared {
            assert!(!name.trim().is_empty());
            assert!(*level >= 1);
        }
        assert!(
            validate_trait_data(&seeded).is_ok(),
            "the pack's own declared starting skills must produce a character \
             the validator accepts — otherwise a new character is refused on \
             first save"
        );
    }
}
