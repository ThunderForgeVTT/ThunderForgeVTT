//! A contributor that exists only to be added and removed.
//!
//! Spec 030, US7. Everything else in this feature tests what an effect *does*.
//! This tests that contributing one is a self-contained act — that the seam is
//! a seam, rather than a shape that happens to have four users.
//!
//! # Why a deliberately useless effect
//!
//! Every real contributor is entangled with the subsystem it drives, so
//! removing one to see what breaks also removes doors, or lighting. This does
//! exactly one observable thing and touches nothing, which makes it the only
//! contributor whose absence proves something about the *seam* rather than
//! about the subsystem.
//!
//! It is also the honest way to answer "what does it take to add a new one?".
//! The answer is: this file, one line in the server's contribution list, and
//! one plugin with one system. Nothing in the interaction core, which is what
//! `scripts/check-interaction-seam.mjs` checks textually.
//!
//! # Why a feature flag rather than a test-only module
//!
//! A `#[cfg(test)]` contributor would only ever exist inside a test binary, so
//! its presence would prove nothing about a real build. This compiles into the
//! shipped one, is authorable by a Game Master, and runs — and turning it off
//! is one line in `Cargo.toml`, which is the removal US7 is about.
//!
//! The flag is on by default today. That is deliberate: an end-to-end test can
//! only observe a contributor that is actually there. What matters is that
//! turning it off changes nothing else, and `interaction_tests.rs` asserts
//! that by assembling a registry without it.

use crate::interaction::{
    ChoiceOption, ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind,
};

/// The effect id this module owns.
pub const ECHO: &str = "probe.echo";

/// The configuration key carrying what to echo.
pub const NOTE_KEY: &str = "note";

/// What the probe contributes to the registry.
pub fn effects() -> Vec<EffectDeclaration> {
    vec![EffectDeclaration {
        id: ECHO.to_string(),
        label: String::from("Echo a note"),
        description: String::from(
            "Does one visible thing and nothing else. Here to prove a new capability can be added without touching anything.",
        ),
        subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
        config: vec![ConfigField {
            key: NOTE_KEY.to_string(),
            label: String::from("Which note"),
            // A choice rather than free text, for the same reason nothing else
            // in this vocabulary takes free text: the kind does not exist.
            kind: ConfigFieldKind::Choice {
                options: vec![
                    ChoiceOption {
                        value: String::from("first"),
                        label: String::from("First"),
                    },
                    ChoiceOption {
                        value: String::from("second"),
                        label: String::from("Second"),
                    },
                ],
            },
            required: true,
        }],
    }]
}

/// Which note a configured `probe.echo` names.
pub fn note_of(config: &serde_json::Value) -> Option<&str> {
    config.get(NOTE_KEY)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::EffectRegistry;

    #[test]
    fn the_probe_is_an_ordinary_contributor_with_no_special_treatment() {
        // If it needed anything the others do not, it would prove nothing
        // about what adding a real one costs.
        let registry = EffectRegistry::assemble([effects()]).expect("one contributor");
        assert!(registry.contains(ECHO));
        assert_eq!(registry.get(ECHO).expect("present").namespace(), "probe");
    }

    #[test]
    fn a_note_reads_back() {
        assert_eq!(
            note_of(&serde_json::json!({ "note": "first" })),
            Some("first")
        );
        assert_eq!(note_of(&serde_json::json!({})), None);
    }
}
