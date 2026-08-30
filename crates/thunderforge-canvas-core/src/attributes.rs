//! What a character's attributes are, without knowing what any of them mean.
//!
//! "Attribute" rather than "ability" deliberately. This codebase already uses
//! *ability* for something else entirely — `world_abilities` is a table of
//! character powers and features, with effects, permissions and lore links.
//! Reusing the word for ability *scores* would put two unrelated concepts
//! behind one name in a codebase where both are live.
//!
//! # Why this is a declared map and not six fields
//!
//! The engine used to carry `TokenAbilities { strength, dexterity,
//! constitution, intelligence, wisdom, charisma }`, which is one game
//! system's character sheet compiled into a renderer. It could hold D&D 5e
//! and Pathfinder 2e. It could not hold either of the other two systems that
//! already ship: Genie has might, cunning and spirit; Blades in the Dark has
//! insight, prowess and resolve. Three fields each, none of them a
//! dexterity.
//!
//! Those manifests already declared their own attribute sets, so the fixed
//! struct was not merely inflexible — it disagreed with data sitting in the
//! repository. What it stored for a Genie character was six `None`s.
//!
//! So the shape is the same one spec 029 arrived at for resources: the system
//! declares what exists and where to read it, the server resolves it, and
//! everything downstream carries `id -> value` pairs it does not interpret.
//! An engine that cannot name a single attribute cannot privilege one
//! system's, which is the property worth having.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One attribute a game system declares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct AttributeDeclaration {
    /// The system's own identifier — `might`, `strength`, `prowess`.
    pub id: String,
    /// What a person is shown.
    pub label: String,
    /// Short form for tight layouts, where the system offers one. Optional
    /// because not every system has one: Pathfinder 2e's manifest declares
    /// labels without abbreviations, and inventing "STR" for it would be this
    /// crate deciding how another ruleset abbreviates.
    pub abbreviation: Option<String>,
    /// Where to read the value inside the actor's stored attribute slot.
    ///
    /// Defaults to `id`, which is what every shipping manifest relies on. It
    /// exists so a system whose stored field names differ from its display
    /// identifiers does not have to rename its stored data to adopt this.
    pub source: String,
    /// Declaration order, so a sheet reads the way its book does.
    pub order: usize,
}

/// An attribute resolved for one actor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct ResolvedAttribute {
    pub id: String,
    pub label: String,
    pub abbreviation: Option<String>,
    pub value: i32,
}

/// Read declared attributes out of an actor's stored slot.
///
/// A declaration the actor stores nothing for is **omitted**, not defaulted.
/// A zero is a statement — it is a real score, and a crippling one in every
/// system here — whereas "this character sheet has not been filled in" is the
/// absence of a statement. Substituting the first for the second would put
/// numbers on screen that nobody entered, and spec 029 settled the same
/// question the same way for resources.
pub fn attributes_from(
    slot: &serde_json::Value,
    declarations: &[AttributeDeclaration],
) -> Vec<ResolvedAttribute> {
    let mut ordered: Vec<&AttributeDeclaration> = declarations.iter().collect();
    ordered.sort_by_key(|d| d.order);

    ordered
        .into_iter()
        .filter_map(|declaration| {
            let value = read_number(slot, &declaration.source)?;
            Some(ResolvedAttribute {
                id: declaration.id.clone(),
                label: declaration.label.clone(),
                abbreviation: declaration.abbreviation.clone(),
                value,
            })
        })
        .collect()
}

/// Read one number out of a stored slot.
///
/// Accepts an integer, or a float that is exactly an integer, or an object
/// carrying a `value` field — the last because a system storing
/// `{"strength": {"value": 14, "proficient": true}}` is describing the same
/// score, and refusing it would force a manifest to lie about its own data.
fn read_number(slot: &serde_json::Value, field: &str) -> Option<i32> {
    let raw = slot.get(field)?;
    let raw = if raw.is_object() {
        raw.get("value")?
    } else {
        raw
    };

    if let Some(number) = raw.as_i64() {
        return i32::try_from(number).ok();
    }
    // A float that is not a whole number is not an attribute score in any
    // system shipping here, and rounding it would invent precision the sheet
    // does not have.
    let float = raw.as_f64()?;
    if float.fract() == 0.0 {
        i32::try_from(float as i64).ok()
    } else {
        None
    }
}

/// Attributes as a lookup, for callers that want one by name.
pub fn by_id(attributes: &[ResolvedAttribute]) -> BTreeMap<String, i32> {
    attributes.iter().map(|a| (a.id.clone(), a.value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn declare(id: &str, order: usize) -> AttributeDeclaration {
        AttributeDeclaration {
            id: id.to_string(),
            label: id.to_string(),
            abbreviation: None,
            source: id.to_string(),
            order,
        }
    }

    /// The property the whole module exists for.
    ///
    /// Two systems with disjoint attribute sets, resolved by the same code,
    /// with nothing anywhere naming either set. A fixed struct passes for one
    /// of these and stores nothing but `None`s for the other.
    #[test]
    fn two_systems_with_nothing_in_common_both_resolve() {
        let five_e = vec![declare("strength", 0), declare("dexterity", 1)];
        let blades = vec![declare("insight", 0), declare("prowess", 1)];

        let sheet = json!({ "strength": 16, "dexterity": 12 });
        let crew = json!({ "insight": 2, "prowess": 3 });

        assert_eq!(
            by_id(&attributes_from(&sheet, &five_e)),
            BTreeMap::from([("strength".into(), 16), ("dexterity".into(), 12)])
        );
        assert_eq!(
            by_id(&attributes_from(&crew, &blades)),
            BTreeMap::from([("insight".into(), 2), ("prowess".into(), 3)])
        );
    }

    /// Declared order, not storage order and not alphabetical.
    #[test]
    fn attributes_come_back_in_the_order_the_system_declared() {
        let declarations = vec![
            declare("spirit", 2),
            declare("might", 0),
            declare("cunning", 1),
        ];
        let stored = json!({ "cunning": 3, "might": 5, "spirit": 4 });

        let ids: Vec<String> = attributes_from(&stored, &declarations)
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, vec!["might", "cunning", "spirit"]);
    }

    /// An unfilled sheet is not a sheet full of zeroes.
    #[test]
    fn an_attribute_with_nothing_stored_is_omitted_rather_than_zeroed() {
        let declarations = vec![declare("might", 0), declare("cunning", 1)];
        let stored = json!({ "might": 5 });

        let resolved = attributes_from(&stored, &declarations);
        assert_eq!(resolved.len(), 1, "cunning is unset, not zero");
        assert_eq!(resolved[0].id, "might");
    }

    /// Zero is a real score and must survive.
    ///
    /// The obvious implementation of the rule above — treat falsy as missing —
    /// deletes it. In Blades a zero action rating is the common case.
    #[test]
    fn a_stored_zero_is_a_value_and_not_an_absence() {
        let declarations = vec![declare("prowess", 0)];
        let resolved = attributes_from(&json!({ "prowess": 0 }), &declarations);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].value, 0);
    }

    /// A source distinct from the id, for a system whose storage disagrees
    /// with its display names.
    #[test]
    fn a_declaration_may_read_from_a_differently_named_field() {
        let declarations = vec![AttributeDeclaration {
            id: "might".into(),
            label: "Might".into(),
            abbreviation: Some("MGT".into()),
            source: "mgt_score".into(),
            order: 0,
        }];
        let resolved = attributes_from(&json!({ "mgt_score": 7, "might": 99 }), &declarations);
        assert_eq!(resolved[0].value, 7, "the source wins over the id");
    }

    /// Real sheets nest.
    #[test]
    fn a_score_stored_beside_other_facts_is_still_read() {
        let declarations = vec![declare("strength", 0)];
        let stored = json!({ "strength": { "value": 14, "proficient": true } });
        assert_eq!(attributes_from(&stored, &declarations)[0].value, 14);
    }

    /// Rubbish is omitted rather than guessed at.
    #[test]
    fn a_value_that_is_not_a_whole_number_is_not_invented_into_one() {
        let declarations = vec![declare("might", 0)];
        for rubbish in [
            json!({ "might": "strong" }),
            json!({ "might": 3.5 }),
            json!({ "might": null }),
            json!({ "might": [] }),
            json!({ "might": {} }),
        ] {
            assert!(
                attributes_from(&rubbish, &declarations).is_empty(),
                "{rubbish} must not resolve to a score"
            );
        }
    }

    /// A float that is exactly an integer is a number a JSON encoder produced.
    #[test]
    fn a_whole_float_is_accepted_because_encoders_emit_them() {
        let declarations = vec![declare("might", 0)];
        assert_eq!(
            attributes_from(&json!({ "might": 5.0 }), &declarations)[0].value,
            5
        );
    }

    /// A system that declares nothing gets nothing, not a default set.
    #[test]
    fn a_system_with_no_declared_attributes_resolves_none() {
        assert!(attributes_from(&json!({ "strength": 18 }), &[]).is_empty());
    }
}
