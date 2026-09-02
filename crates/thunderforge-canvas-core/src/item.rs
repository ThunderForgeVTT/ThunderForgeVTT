//! Taking something off the map and into a bag, as a contributor to the
//! interaction seam.
//!
//! Spec 031, US3 (FR-013..FR-017). A sword on the flagstones, a key on a
//! shelf, a pouch dropped by whatever used to be holding it: the Game Master
//! places it, a player clicks it, and it stops being scenery.
//!
//! # Why the item subsystem declares this and the interaction core does not
//!
//! ADR-054: the interaction plugin owns placement, hit-testing, triggers,
//! permission and `once` bookkeeping, and owns no effect at all. An item is a
//! thing the item subsystem knows about — what it is, whose bag it lands in,
//! what its page says — and none of that is the seam's business.
//! `scripts/check-interaction-seam.mjs` enforces the textual half of this.
//!
//! # Why the reference is an item id and not a description of one
//!
//! The same argument `lore_link` makes. A configuration that carried a name, a
//! weight and a picture would be a second, weaker copy of the item record,
//! free to drift from the real one; and the moment it drifts, the thing a
//! player picks up is not the thing the Game Master placed. A typed reference
//! cannot drift, because there is only ever one of it.
//!
//! It also settles what "inspect" means (FR-014) without a second field: the
//! reference is the item's page, so the offer to look before taking costs
//! nothing here.
//!
//! # Why this declares no "how many"
//!
//! A quantity field would make the effect a transaction — take three of five,
//! leave two — and a partial pickup is a stack-splitting feature nobody has
//! asked for. One placed item is one thing; two coins in a room are two
//! placements. That stays additive: a quantity may be added later, and an
//! interactive authored without one keeps working, because a field that is not
//! required is a field the reader may not find.
//!
//! # What this module deliberately does not decide
//!
//! Whether the pickup succeeds. Two players clicking the same pouch in the
//! same second must end with exactly one of them holding it (FR-016), and that
//! is settled where the write is serialised — at the server — not in a
//! declaration and not in the engine. The engine reports the intent; the
//! server answers, and a refusal puts the token back (FR-017).

use crate::interaction::{ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind};

/// The effect id this module owns.
pub const PICKUP: &str = "item.pickup";

/// What an item reference points at.
///
/// A string rather than an enum in the interaction core, so a subsystem
/// gaining a referenceable thing never edits that file.
pub const ITEM: &str = "item";

/// The configuration key carrying the item.
pub const ITEM_KEY: &str = "item";

/// What the item subsystem contributes to the registry.
pub fn effects() -> Vec<EffectDeclaration> {
    vec![EffectDeclaration {
        id: PICKUP.to_string(),
        label: String::from("Pick up an item"),
        description: String::from(
            "Offers the item to whoever activates it. Taken, it leaves the map and enters their inventory.",
        ),
        // A placed thing only. The other two subjects cannot be taken:
        // an area is not portable, and a piece of the map's own geometry
        // stops being the map if somebody puts it in a bag. Offering it
        // there would be an authoring form full of choices that could
        // only ever fail at the table.
        subject_kinds: vec![SubjectKind::Prop],
        config: vec![ConfigField {
            key: ITEM_KEY.to_string(),
            label: String::from("Item"),
            kind: ConfigFieldKind::Reference {
                of: ITEM.to_string(),
            },
            required: true,
        }],
    }]
}

/// The item a configured `item.pickup` points at, if it is well formed.
pub fn item_of(config: &serde_json::Value) -> Option<&str> {
    config.get(ITEM_KEY)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{EffectRegistry, validate_config};

    #[test]
    fn an_item_is_required_because_a_pickup_of_nothing_is_a_dead_prop() {
        // The failure this rules out is the quiet one: an interactive that
        // hit-tests, activates, is permitted, and then hands the server no
        // subject. At the table that reads as a broken product rather than as
        // a misconfiguration, because nothing anywhere says what was missing.
        let declaration = &effects()[0];
        assert!(!validate_config(declaration, &serde_json::json!({})).is_empty());
        assert!(validate_config(declaration, &serde_json::json!({ "item": "i-1" })).is_empty());
    }

    #[test]
    fn what_is_taken_is_a_reference_and_nothing_may_ride_alongside_it() {
        // Asserted structurally rather than by checking for particular field
        // names: the *only* field is a typed reference, so there is no place
        // to put a copy of the item's name, price or picture that could then
        // disagree with the record.
        let declaration = &effects()[0];
        assert_eq!(declaration.config.len(), 1);
        assert!(matches!(
            declaration.config[0].kind,
            ConfigFieldKind::Reference { .. }
        ));

        let errors = validate_config(
            declaration,
            &serde_json::json!({ "item": "i-1", "quantity": 3 }),
        );
        assert!(
            errors.iter().any(|e| matches!(
                e,
                crate::interaction::AuthoringError::UnknownConfigField { .. }
            )),
            "a field nothing declared must be refused, not stored and ignored"
        );
    }

    #[test]
    fn only_a_placed_thing_can_be_taken() {
        // FR-013 places an item token. The other two subjects are excluded at
        // the declaration, which is what keeps them out of the authoring form
        // — rather than at activation, where the refusal would arrive after a
        // Game Master had already built the scene around it.
        let kinds = &effects()[0].subject_kinds;
        assert!(kinds.contains(&SubjectKind::Prop));
        assert!(!kinds.contains(&SubjectKind::Door));
        assert!(!kinds.contains(&SubjectKind::Region));
    }

    #[test]
    fn the_declaration_is_namespaced_so_a_collision_is_a_prefix_concern() {
        let registry = EffectRegistry::assemble([effects()]).expect("one contributor");
        assert!(registry.contains(PICKUP));
        assert_eq!(registry.get(PICKUP).expect("present").namespace(), "item");
    }

    #[test]
    fn it_joins_the_real_registry_without_disturbing_anything_already_there() {
        // The cost of a new subsystem, measured rather than asserted: one more
        // set in the list, one more effect, and every other answer identical.
        use crate::{lighting, lore_link, navigation, wall};

        let before = EffectRegistry::assemble([
            lore_link::effects(),
            wall::interaction_effects(),
            lighting::interaction_effects(),
            navigation::effects(),
        ])
        .expect("the registry as it was");

        let after = EffectRegistry::assemble([
            lore_link::effects(),
            wall::interaction_effects(),
            lighting::interaction_effects(),
            navigation::effects(),
            effects(),
        ])
        .expect("no collision with anything already declared");

        assert_eq!(after.len(), before.len() + 1);
        assert!(after.contains(PICKUP));
        assert!(!before.contains(PICKUP));
        for declaration in before.all() {
            assert_eq!(
                after.get(&declaration.id),
                Some(declaration),
                "a contributor is not changed by which others are present"
            );
        }
    }

    #[test]
    fn a_configured_item_reads_back() {
        assert_eq!(item_of(&serde_json::json!({ "item": "i-1" })), Some("i-1"));
        assert_eq!(item_of(&serde_json::json!({})), None);
        // A number is not an id. Reading it back as one would send the server
        // a reference that resolves to nothing.
        assert_eq!(item_of(&serde_json::json!({ "item": 7 })), None);
    }
}
