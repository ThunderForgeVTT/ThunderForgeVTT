//! Opening a lore page, as a contributor to the interaction seam.
//!
//! Spec 030, US1. This is the whole of what a contributing subsystem is: a
//! plugin with one system that reads [`InteractionActivated`] and handles the
//! identifiers it declared, ignoring everything else.
//!
//! Nothing in `plugins::interaction` knows this file exists, and nothing here
//! calls into it. Remove this plugin and the seam still works, offering one
//! fewer effect (FR-039, FR-040).
//!
//! # Why this emits rather than navigates
//!
//! Opening a browser tab needs the application's URL structure, and the engine
//! has no business holding it — a world's routes are chrome, and Constitution
//! Principle I puts chrome in React. So this recognises the effect, resolves
//! which entry it points at, and emits an engine event; the bridge opens the
//! tab.
//!
//! That split is also what keeps the canvas where it was. A plugin that
//! navigated would be one mistake away from taking the whole session with it.

use bevy::prelude::*;

use thunderforge_canvas_core::lore_link::{OPEN, entry_of};

use crate::emit_event;
use crate::plugins::interaction::InteractionActivated;

pub struct LoreLinkPlugin;

impl Plugin for LoreLinkPlugin {
    fn build(&self, app: &mut App) {
        // Registered here as well as in `InteractionPlugin`. `add_message` is
        // idempotent, and a contributor that could only be added after the
        // seam would not be independently addable (Principle II).
        app.add_message::<InteractionActivated>()
            .add_systems(Update, open_requested_entries);
    }
}

/// Handle every `lore.open` that was dispatched this frame.
fn open_requested_entries(mut activations: MessageReader<InteractionActivated>) {
    for activation in activations.read() {
        // Everything not ours belongs to somebody else, or to nobody. Either
        // way it is not this plugin's business, and skipping quietly is what
        // lets several contributors read the same stream.
        if activation.effect_id != OPEN {
            continue;
        }

        let Some(entry) = entry_of(&activation.config) else {
            // Configuration that validated at authoring time and does not read
            // back here means the two disagree, which is worth saying out
            // loud: at the table it looks like a book that does nothing.
            warn!(
                "lore.open on {} carries no readable entry",
                activation.interactive_id
            );
            continue;
        };

        crate::dispatched_effects_slot()
            .lock()
            .map(|mut log| {
                log.push(serde_json::json!({
                    "effectId": OPEN,
                    "interactiveId": activation.interactive_id,
                    "entry": entry,
                }));
            })
            .ok();

        emit_event(serde_json::json!({
            "type": "openLore",
            "interactiveId": activation.interactive_id,
            "entryId": entry,
        }));
    }
}
