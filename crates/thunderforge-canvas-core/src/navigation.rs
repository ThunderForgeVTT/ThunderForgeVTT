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
//!
//! # Travel between levels is the complete one
//!
//! `nav.travel` moves a token between the levels of *one* scene, and it does
//! move it. A transition is a pair of interactives — a stairwell region on
//! each floor, a trapdoor and the ladder under it — each naming the other as
//! its partner. A token that walks into one, or whose player clicks one, is
//! put down at the other.
//!
//! The declaration is all that lives here. Who may travel, where exactly they
//! land and who is told are the server's to decide, because a level is also a
//! privacy boundary and a client cannot be the one to say which side of it a
//! token is on.

use crate::interaction::{ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind};

/// The effect id this module owns.
pub const REQUEST_SCENE: &str = "nav.request_scene";

/// What a destination reference points at.
pub const SCENE: &str = "scene";

/// The configuration key carrying the destination.
pub const DESTINATION_KEY: &str = "destination";

/// The effect id for travelling between the levels of a scene.
pub const TRAVEL: &str = "nav.travel";

/// What a travel partner reference points at.
pub const INTERACTIVE: &str = "interactive";

/// The configuration key carrying the partner: the interactive, usually on
/// another level, that a traveller arrives at.
pub const PARTNER_KEY: &str = "partner";

/// What navigation contributes to the registry.
///
/// `nav.request_scene` stays first: it is the older of the two and its tests
/// address it by position.
pub fn effects() -> Vec<EffectDeclaration> {
    vec![request_scene(), travel()]
}

fn travel() -> EffectDeclaration {
    EffectDeclaration {
        id: TRAVEL.to_string(),
        label: String::from("Travel to another level"),
        description: String::from(
            "Moves whoever walks in, or clicks, to the partner you choose — stairs, a ladder, a trapdoor. Give the partner the same effect pointing back for a way that works in both directions.",
        ),
        // A door is offered here where `nav.request_scene` withholds it: a
        // trapdoor and a hatch are doors, and they are exactly how a table
        // expects to change floors.
        subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
        config: vec![ConfigField {
            key: PARTNER_KEY.to_string(),
            label: String::from("Arrives at"),
            kind: ConfigFieldKind::Reference {
                of: INTERACTIVE.to_string(),
            },
            required: true,
        }],
    }
}

fn request_scene() -> EffectDeclaration {
    EffectDeclaration {
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
    }
}

/// The partner a configured travel arrives at.
pub fn partner_of(config: &serde_json::Value) -> Option<&str> {
    config.get(PARTNER_KEY)?.as_str()
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
    fn level_travel_names_a_partner_and_may_sit_on_any_subject() {
        let declaration = effects()
            .into_iter()
            .find(|d| d.id == TRAVEL)
            .expect("declared");
        assert_eq!(declaration.namespace(), "nav");
        for kind in [SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region] {
            assert!(declaration.subject_kinds.contains(&kind));
        }
        assert!(!validate_config(&declaration, &serde_json::json!({})).is_empty());
        let config = serde_json::json!({ "partner": "i-2" });
        assert!(validate_config(&declaration, &config).is_empty());
        assert_eq!(partner_of(&config), Some("i-2"));
    }

    #[test]
    fn travel_is_not_offered_on_a_wall_segment() {
        assert!(!effects()[0].subject_kinds.contains(&SubjectKind::Door));
    }
}
