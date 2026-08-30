//! Asking to go somewhere else, as a contributor to the interaction seam.
//!
//! Spec 030, US6. A player steps onto the stairs and *asks*; the Game Master
//! decides.
//!
//! # This contributor is deliberately incomplete, and says so
//!
//! Multi-scene navigation does not exist in this project. So an approved
//! `nav.request_scene` raises a request, the GM approves or refuses it, the
//! requester is told — and nothing moves anybody, because there is nothing
//! yet to move them with.
//!
//! That is honest rather than dead. The request and the decision are the parts
//! this feature owns, and they work end to end today; the destination is
//! somebody else's future spec. The alternative — leaving it out until
//! navigation exists — would have meant building the approval flow with no
//! effect that uses it, and an approval flow with no user is a flow nobody has
//! tested against a real one.
//!
//! When multi-scene navigation lands, it performs this effect. Nothing here
//! changes.

use crate::interaction::{ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind};

/// The effect id this module owns.
pub const REQUEST_SCENE: &str = "nav.request_scene";

/// What a destination reference points at.
pub const SCENE: &str = "scene";

/// The configuration key carrying the destination.
pub const DESTINATION_KEY: &str = "destination";

/// What navigation contributes to the registry.
pub fn effects() -> Vec<EffectDeclaration> {
    vec![EffectDeclaration {
        id: REQUEST_SCENE.to_string(),
        label: String::from("Ask to travel to another scene"),
        description: String::from(
            "Raises a request for you to approve. Nothing moves until you do.",
        ),
        // A staircase, a doorway, a threshold at the edge of the map. Not a
        // wall segment: travelling by clicking the wall itself is not a thing
        // anyone has asked for, and offering it would be a footgun in the
        // authoring form.
        subject_kinds: vec![SubjectKind::Prop, SubjectKind::Region],
        config: vec![ConfigField {
            key: DESTINATION_KEY.to_string(),
            label: String::from("Where to"),
            kind: ConfigFieldKind::Reference {
                of: SCENE.to_string(),
            },
            required: true,
        }],
    }]
}

/// Where a configured request would go.
pub fn destination_of(config: &serde_json::Value) -> Option<&str> {
    config.get(DESTINATION_KEY)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{EffectRegistry, validate_config};

    #[test]
    fn the_declaration_is_namespaced_and_assembles() {
        let registry = EffectRegistry::assemble([effects()]).expect("one contributor");
        assert_eq!(
            registry.get(REQUEST_SCENE).expect("declared").namespace(),
            "nav"
        );
    }

    #[test]
    fn a_destination_is_a_scene_reference_and_not_an_address() {
        // Same structural guarantee as `lore.open`: there is no free-text
        // field in the vocabulary, so nowhere to type one.
        let declaration = &effects()[0];
        assert!(matches!(
            declaration.config[0].kind,
            ConfigFieldKind::Reference { .. }
        ));
        assert!(!validate_config(declaration, &serde_json::json!({})).is_empty());
        assert!(
            validate_config(declaration, &serde_json::json!({ "destination": "s-1" })).is_empty()
        );
    }

    #[test]
    fn travel_is_not_offered_on_a_wall_segment() {
        assert!(!effects()[0].subject_kinds.contains(&SubjectKind::Door));
    }
}
