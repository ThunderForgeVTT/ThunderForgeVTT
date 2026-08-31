//! The engine half of the contributor that exists only to be added and
//! removed.
//!
//! Spec 030, US7. Twenty lines of behaviour and one system, which is what a
//! contributor costs. Compare it with `wall.rs` and `lighting.rs`: those are
//! larger because *doors and lights* are larger, not because contributing is.
//!
//! Deleting this file and its two registration lines removes the capability
//! and changes nothing else. That is the claim, and
//! `crates/thunderforge-canvas-core/src/interaction_tests.rs` asserts the
//! registry half of it where tests actually execute.

use bevy::prelude::*;

use thunderforge_canvas_core::seam_probe::{ECHO, note_of};

use crate::emit_event;
use crate::plugins::interaction::InteractionActivated;

pub struct SeamProbePlugin;

impl Plugin for SeamProbePlugin {
    fn build(&self, app: &mut App) {
        // Registered here as well as in `InteractionPlugin`, idempotently — a
        // contributor that could only be added after the seam would not be
        // independently addable (Principle II).
        app.add_message::<InteractionActivated>()
            .add_systems(Update, echo_notes);
    }
}

/// The one observable thing this contributor does.
fn echo_notes(mut activations: MessageReader<InteractionActivated>) {
    for activation in activations.read() {
        if activation.effect_id != ECHO {
            continue;
        }
        let note = note_of(&activation.config).unwrap_or("nothing");

        crate::dispatched_effects_slot()
            .lock()
            .map(|mut log| {
                log.push(serde_json::json!({
                    "effectId": ECHO,
                    "interactiveId": activation.interactive_id,
                    "note": note,
                }));
            })
            .ok();

        emit_event(serde_json::json!({
            "type": "seamProbeEcho",
            "interactiveId": activation.interactive_id,
            "note": note,
        }));
    }
}
