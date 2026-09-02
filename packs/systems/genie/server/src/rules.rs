//! What Genie computes, as opposed to what it stores.
//!
//! Genie's manifest already declares what a character *has* — three
//! abilities, Health, Wish Points — and the server already reads those. This
//! file is only for the half a manifest cannot express: a number that exists
//! because another number does.
//!
//! # Why the ladder is read from the manifest rather than written here
//!
//! `system.json` carries the by-level Wish Points table, and it is the
//! authority on it. Copying those ten numbers into Rust would create a second
//! table that has to be kept in step by hand, which is the failure
//! `thunderforge_canvas_core::attributes` was written to end: two sources of
//! truth for one list, and the manifest is the one that ships with the pack.
//!
//! So the rules are *constructed from* the manifest and hold no numbers of
//! their own.

use std::collections::BTreeMap;

use thunderforge_canvas_core::attributes::AttributeDeclaration;
use thunderforge_canvas_core::system_rules::{
    DeclaredValue, DeclaredValueKind, DeclaredValues, Origin, SystemRules,
};

/// The identifier Genie publishes its by-level Wish Points total under.
///
/// Deliberately **not** `max_wish_points`, which the actor already stores.
/// Spec 018 US6 wants the total to follow the level table "without manual
/// entry", and the stored field is the manual entry — but reconciling those
/// two is spec 018's decision to make, not something to change quietly while
/// building a contract. Publishing them side by side under different names
/// makes any disagreement visible instead of picking a winner in the dark.
pub const WISH_POINTS_FOR_LEVEL: &str = "wishPointsForLevel";

/// Where a Genie character's level is stored, inside `trait_data`.
const LEVEL: &str = "level";

pub struct GenieRules {
    /// Character level to Wish Points, read from the manifest's table.
    ladder: BTreeMap<i32, i32>,
}

impl GenieRules {
    /// Build the rules from the pack's own manifest.
    ///
    /// A manifest with no usable table yields an empty ladder, and an empty
    /// ladder derives nothing. That is correct rather than defensive: a
    /// system that declares no progression has none, and inventing one would
    /// put a number on a sheet that no book supports.
    pub fn from_manifest(manifest: &serde_json::Value) -> Self {
        let mut ladder = BTreeMap::new();

        if let Some(table) = manifest.get("wishPoints").and_then(|w| w.as_object()) {
            for (level, entry) in table {
                let Ok(level) = level.parse::<i32>() else {
                    continue;
                };
                // The table is an array per level, matching the `spellSlots`
                // shape spec 018 FR-004 asked it to be structurally compatible
                // with. Genie has one value per level; the shape allows more.
                let total = entry
                    .as_array()
                    .and_then(|values| values.first())
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok());
                if let Some(total) = total {
                    ladder.insert(level, total);
                }
            }
        }

        Self { ladder }
    }

    /// The Wish Points a character of this level has, when the table says.
    ///
    /// A level the table does not cover derives nothing. Clamping to the
    /// nearest rung would be inventing a rule the system did not write down.
    fn wish_points_at(&self, level: i32) -> Option<i32> {
        self.ladder.get(&level).copied()
    }
}

impl SystemRules for GenieRules {
    fn id(&self) -> &str {
        "genie"
    }

    fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
        vec![AttributeDeclaration {
            id: WISH_POINTS_FOR_LEVEL.to_string(),
            label: "Wish Points (by level)".to_string(),
            abbreviation: Some("WP/L".to_string()),
            source: WISH_POINTS_FOR_LEVEL.to_string(),
            order: 0,
        }]
    }

    fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue> {
        let Some(level) = stored.integer(LEVEL) else {
            // No level recorded: nothing to look up. Omitted rather than
            // defaulted to level one, because a sheet nobody has filled in is
            // not a first-level character.
            return Vec::new();
        };
        let Some(total) = self.wish_points_at(level) else {
            return Vec::new();
        };

        vec![DeclaredValue {
            id: WISH_POINTS_FOR_LEVEL.to_string(),
            label: "Wish Points (by level)".to_string(),
            abbreviation: Some("WP/L".to_string()),
            value: DeclaredValueKind::Integer(total),
            group: None,
            group_label: None,
            headline: false,
            origin: Origin::Derived,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real manifest, not a fixture — so this test fails if the pack's
    /// table changes and nobody thought about the rule that reads it.
    fn rules() -> GenieRules {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../system.json");
        let text = std::fs::read_to_string(path).expect("genie's manifest should be readable");
        GenieRules::from_manifest(&serde_json::from_str(&text).expect("valid manifest json"))
    }

    fn at_level(level: i32) -> DeclaredValues {
        DeclaredValues::new([DeclaredValue {
            id: LEVEL.to_string(),
            label: "Level".to_string(),
            abbreviation: None,
            value: DeclaredValueKind::Integer(level),
            group: None,
            group_label: None,
            headline: false,
            origin: Origin::Stored,
        }])
    }

    fn derived_total(level: i32) -> Option<i32> {
        rules()
            .derive(&at_level(level))
            .first()
            .and_then(|v| v.value.as_integer())
    }

    #[test]
    fn the_ladder_comes_from_the_manifest_and_covers_every_level() {
        let rules = rules();
        for level in 1..=10 {
            assert!(
                rules.wish_points_at(level).is_some(),
                "level {level} should be on the manifest's table"
            );
        }
    }

    #[test]
    fn wish_points_rise_with_level_and_never_fall() {
        let rules = rules();
        let mut previous = 0;
        for level in 1..=10 {
            let total = rules.wish_points_at(level).expect("covered level");
            assert!(
                total >= previous,
                "level {level} gives {total}, below level {} at {previous}",
                level - 1
            );
            previous = total;
        }
    }

    #[test]
    fn a_level_the_table_does_not_cover_derives_nothing() {
        assert_eq!(derived_total(0), None, "there is no level zero");
        assert_eq!(
            derived_total(11),
            None,
            "past the table's end, clamping would invent a rule"
        );
    }

    #[test]
    fn a_character_with_no_level_recorded_derives_nothing() {
        assert!(
            rules().derive(&DeclaredValues::default()).is_empty(),
            "an unfilled sheet is not a first-level character"
        );
    }

    /// The property the contract requires of every implementation.
    #[test]
    fn deriving_twice_from_the_same_input_gives_the_same_answer() {
        let rules = rules();
        let first = rules.derive(&at_level(4));
        for _ in 0..32 {
            assert_eq!(rules.derive(&at_level(4)), first);
        }
    }

    /// Everything `derive` returns must be something `derived_declarations`
    /// promised, or the resolver drops it and the sheet silently loses a row.
    #[test]
    fn everything_derived_was_declared() {
        let rules = rules();
        let declared: Vec<String> = rules
            .derived_declarations()
            .into_iter()
            .map(|d| d.id)
            .collect();
        for level in 1..=10 {
            for value in rules.derive(&at_level(level)) {
                assert!(
                    declared.contains(&value.id),
                    "{} was derived but never declared",
                    value.id
                );
            }
        }
    }
}
