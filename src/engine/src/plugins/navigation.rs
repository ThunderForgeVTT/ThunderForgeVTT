//! Asking to travel somewhere else, declared for the seam and performed
//! elsewhere.
//!
//! Spec 030, US6. This contributor has no system, and that is the honest
//! shape of it: `nav.request_scene` raises a request, the Game Master decides,
//! and the requester is told — all of which happens across the server and the
//! application, none of it on the canvas. Multi-scene navigation does not
//! exist yet, so there is nothing here to move anybody with.
//!
//! # Why it declares anyway
//!
//! Because the seam reports an identifier it does not recognise as
//! *unavailable* before dispatching it (ADR-054, decision 4). A build that can
//! perform an effect and does not declare it would have its own working
//! effects reported missing — which is the check firing on the one case it is
//! meant to leave alone.
//!
//! So the rule is: a declaration says "this build can perform this", not "this
//! plugin performs this". Travel can be requested in this build. That the
//! request is answered outside the engine does not make it absent.
//!
//! The day multi-scene navigation lands it adds a system here, and nothing
//! else in the seam changes.

use bevy::prelude::*;

use crate::plugins::interaction::{InteractionActivated, contribute};

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        // Idempotent, and registered here for the same reason every other
        // contributor registers it: so this plugin can be added before the
        // seam as well as after it (Principle II).
        app.add_message::<InteractionActivated>();

        contribute(app, thunderforge_canvas_core::navigation::effects());
    }
}
