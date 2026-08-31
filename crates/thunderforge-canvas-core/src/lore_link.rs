//! Opening a lore entry from something on the map.
//!
//! The first contributor to the interaction seam (spec 030, US1), and it is
//! deliberately the smallest one: a book on a table that opens the page about
//! it. If the seam cannot carry this, it cannot carry anything.
//!
//! # Why the reference is an entry id and not an address
//!
//! A Game Master cannot point an interactive at an arbitrary URL, because
//! there is no configuration field that would accept one — the vocabulary in
//! [`crate::interaction`] has no free-text kind at all.
//!
//! That is a deliberate structural answer to the spec's hostile-destination
//! edge case, and it beats the two alternatives. An allowlist is a moderation
//! surface nobody has agreed to own. A confirmation prompt puts the judgement
//! on the player, who has the least context about where the link came from and
//! the most reason to trust the table they are sitting at.
//!
//! It also leaves the constitution's content guardrail untouched: the
//! reference resolves inside the world it belongs to, so nothing here makes
//! one world's content reachable from another.
//!
//! Linking to a handout, image or journal is deferred rather than refused. The
//! reference is typed, so adding a kind later is additive.

use crate::interaction::{ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind};

/// The effect id this module owns.
pub const OPEN: &str = "lore.open";

/// What a lore entry reference points at.
///
/// A string rather than an enum in the interaction core, so a subsystem
/// gaining a referenceable thing never edits that file.
pub const LORE_ENTRY: &str = "loreEntry";

/// The configuration key carrying the entry.
pub const ENTRY_KEY: &str = "entry";

/// What lore contributes to the registry.
pub fn effects() -> Vec<EffectDeclaration> {
    vec![EffectDeclaration {
        id: OPEN.to_string(),
        label: String::from("Open a lore page"),
        description: String::from("Opens an entry from this world's lore in a new tab."),
        // Anything can be a page in a book: the tome on the lectern, the door
        // with the inscription, the alcove the party steps into.
        subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
        config: vec![ConfigField {
            key: ENTRY_KEY.to_string(),
            label: String::from("Lore entry"),
            kind: ConfigFieldKind::Reference {
                of: LORE_ENTRY.to_string(),
            },
            required: true,
        }],
    }]
}

/// The entry a configured `lore.open` points at, if it is well formed.
pub fn entry_of(config: &serde_json::Value) -> Option<&str> {
    config.get(ENTRY_KEY)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{EffectRegistry, validate_config};

    #[test]
    fn a_link_cannot_be_configured_with_an_address() {
        // The claim this whole design rests on, asserted structurally rather
        // than by checking a string for "http": the *only* field is a typed
        // reference, and a reference is validated as an id.
        //
        // A test that rejected addresses by pattern would pass against a
        // design that still accepted free text, and would then have to keep
        // growing to cover every scheme somebody thought of.
        let declaration = &effects()[0];
        assert_eq!(declaration.config.len(), 1);
        assert!(matches!(
            declaration.config[0].kind,
            ConfigFieldKind::Reference { .. }
        ));

        // And nothing else may be sent alongside it.
        let errors = validate_config(
            declaration,
            &serde_json::json!({ "entry": "abc", "url": "https://example.invalid" }),
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
    fn an_entry_is_required_because_a_link_to_nothing_is_a_dead_prop() {
        let declaration = &effects()[0];
        assert!(!validate_config(declaration, &serde_json::json!({})).is_empty());
        assert!(validate_config(declaration, &serde_json::json!({ "entry": "abc" })).is_empty());
    }

    #[test]
    fn the_declaration_is_namespaced_so_a_collision_is_a_prefix_concern() {
        let registry = EffectRegistry::assemble([effects()]).expect("one contributor");
        assert!(registry.contains(OPEN));
        assert_eq!(registry.get(OPEN).expect("present").namespace(), "lore");
    }

    #[test]
    fn a_configured_entry_reads_back() {
        assert_eq!(
            entry_of(&serde_json::json!({ "entry": "e-1" })),
            Some("e-1")
        );
        assert_eq!(entry_of(&serde_json::json!({})), None);
        assert_eq!(entry_of(&serde_json::json!({ "entry": 7 })), None);
    }
}
