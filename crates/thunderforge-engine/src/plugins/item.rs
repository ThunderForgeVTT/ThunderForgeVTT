//! Taking something off the map, as a contributor to the interaction seam.
//!
//! Spec 031, US3 (FR-013..FR-017). The same shape as `lore_link.rs`, because
//! that is what a contributor is: a plugin with one system that reads
//! [`InteractionActivated`], handles the identifiers it declared, and ignores
//! everything else.
//!
//! Nothing in `plugins::interaction` knows this file exists, and nothing here
//! calls into it. Remove this plugin and the seam still works, offering one
//! fewer effect (ADR-054).
//!
//! # Why this emits rather than takes
//!
//! A pickup is a write to two things the engine does not own: what is on the
//! scene, and what is in somebody's bag. Both live on the server, and the
//! server is the only place the race in FR-016 can be settled — two players
//! clicking the same pouch in the same second must end with exactly one of
//! them holding it, and two clients each deciding for themselves would end
//! with two.
//!
//! So this recognises the effect, resolves which item is meant, and emits an
//! engine event. The application asks the server; the server answers; the
//! token leaves the map and the entry appears in an inventory because the
//! server said so, and a refusal puts the token back (FR-017).
//!
//! ADR-054 permits the engine to apply a visible change optimistically for
//! responsiveness. It does not permit the engine to be a second authority on
//! whether the change was allowed, and persisting anything from here would
//! make it one.

use bevy::prelude::*;

use thunderforge_canvas_core::item::{PICKUP, item_of};

use crate::emit_event;
use crate::plugins::interaction::{InteractionActivated, contribute};

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        // Registered here as well as in `InteractionPlugin`. `add_message` is
        // idempotent, and a contributor that could only be added after the
        // seam would not be independently addable (Principle II).
        app.add_message::<InteractionActivated>()
            .add_systems(Update, offer_requested_items);

        // The declaration and the handler are added by the same line of the
        // build, so a build that can perform this says it can, and one without
        // this plugin says nothing — which is what lets the seam report an
        // absent subsystem before dispatching into it.
        contribute(app, thunderforge_canvas_core::item::effects());
    }
}

/// Handle every `item.pickup` that was dispatched this frame.
fn offer_requested_items(mut activations: MessageReader<InteractionActivated>) {
    for activation in activations.read() {
        // Everything not ours belongs to somebody else, or to nobody. Either
        // way it is not this plugin's business, and skipping quietly is what
        // lets several contributors read the same stream.
        if activation.effect_id != PICKUP {
            continue;
        }

        let Some(item) = item_of(&activation.config) else {
            // Configuration that validated at authoring time and does not read
            // back here means the two disagree, which is worth saying out
            // loud: at the table it looks like a thing on the floor that
            // cannot be picked up and never says why.
            warn!(
                "item.pickup on {} carries no readable item",
                activation.interactive_id
            );
            continue;
        };

        crate::dispatched_effects_slot()
            .lock()
            .map(|mut log| {
                log.push(serde_json::json!({
                    "effectId": PICKUP,
                    "interactiveId": activation.interactive_id,
                    "item": item,
                }));
            })
            .ok();

        // `subjectRef` rides along because the application needs to know which
        // token to take off the map once the server agrees. Resolving it here
        // rather than there keeps that answer with the activation it belongs
        // to — by the time a round trip has come back, the interactive may
        // have been edited or the scene changed.
        emit_event(serde_json::json!({
            "type": "pickUpItem",
            "interactiveId": activation.interactive_id,
            "itemId": item,
            "subjectRef": activation.subject_ref,
        }));
    }
}
