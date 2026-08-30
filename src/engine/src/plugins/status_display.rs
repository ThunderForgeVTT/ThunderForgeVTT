//! Bars and counters above tokens.
//!
//! Spec 029, User Stories 1 and 2. A token's resources — health, stamina,
//! mana, whatever the active game system declares — drawn on the map so a
//! crowded encounter can be read rather than interrogated.
//!
//! # What this plugin does not decide
//!
//! Which resources exist (the game system declares them), what their values
//! are (the server sends them), and how much of that a viewer is entitled to
//! know (the server resolves it). This draws what it is given.
//!
//! That last one is not a division of labour, it is a security boundary. A
//! bar is a disclosure channel — a player watching a boss's health learns
//! something whether or not anyone meant them to — so coarsening happens on
//! the server and a client is never sent a figure it may not display. Nothing
//! here can widen what a viewer sees, because nothing here has the value.
//!
//! # Why bars are drawn here and the corner panel is not
//!
//! Constitution Principle I: the ECS owns what is spatial, React owns chrome.
//! A bar above a token tracks its position, scales with the camera and
//! reorders with other entities, so it belongs to the engine. The
//! selected-token panel is screen-space text and belongs in React, where it
//! keeps screen readers, text selection and browser zoom — all of which would
//! have to be reimplemented to draw it in WebGL. See ADR-053.

use bevy::prelude::*;

use crate::TOKEN_SIZE;
use crate::TokenIdentity;
use thunderforge_canvas_core::resource_display::{Disclosed, ResourceDefinition};

/// Height of one bar, in world units.
const BAR_HEIGHT: f32 = 10.0;

/// Width of a bar, matched to the token so the two read as one object.
const BAR_WIDTH: f32 = TOKEN_SIZE.x;

/// Gap between stacked bars.
const BAR_GAP: f32 = 3.0;

/// How far above the token's centre the first bar sits.
const FIRST_BAR_OFFSET: f32 = TOKEN_SIZE.y / 2.0 + 8.0;

/// Drawn above the token sprite, below any selection furniture.
const BAR_Z: f32 = 5.0;

/// The track a bar's fill sits in. Dark and mostly opaque, so a nearly-empty
/// bar still reads as a bar rather than disappearing into the map.
const TRACK_COLOR: Color = Color::srgba(0.06, 0.07, 0.09, 0.78);

/// Fill for a resource whose value this viewer is not entitled to know.
///
/// Deliberately mid-grey and deliberately *not* empty: an empty bar says "at
/// zero", which is a different and much more actionable claim than "you have
/// not been told". Rendering the two alike would leak by implication — a
/// player would read a withheld boss as nearly dead.
const UNDISCLOSED_COLOR: Color = Color::srgba(0.42, 0.45, 0.50, 0.85);

/// Default fill when the application supplies no palette.
const DEFAULT_FILL: Color = Color::srgb(0.784, 0.208, 0.216);

/// What a token currently displays, as resolved by the server.
///
/// Attached to token entities. Empty means the token draws nothing at all —
/// not an empty container, per FR-007.
#[derive(Component, Debug, Clone, Default)]
pub struct TokenStatus {
    pub resources: Vec<ResolvedResource>,
}

/// One resource on one token, already reduced to what this viewer may see.
#[derive(Debug, Clone)]
pub struct ResolvedResource {
    pub definition: ResourceDefinition,
    pub disclosed: Disclosed,
}

/// Marks geometry this plugin owns, so it can be cleared without disturbing
/// anything else parented to a token.
#[derive(Component)]
struct StatusGeometry;

pub struct StatusDisplayPlugin;

impl Plugin for StatusDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, redraw_changed_status);
    }
}

/// How full a bar should be drawn, 0.0–1.0, and whether that is a real
/// reading or a stand-in for "not disclosed".
///
/// Separated from the drawing so the mapping is a value decision rather than
/// something buried in transform arithmetic — and so a reader can check the
/// one line that matters: a withheld resource never produces a fill derived
/// from its actual value, because this function is not given one.
fn fill_of(disclosed: &Disclosed) -> (f32, bool) {
    match disclosed {
        Disclosed::Visible { entries } => {
            let fraction =
                thunderforge_canvas_core::resource_display::proportion(entries).unwrap_or(0.0);
            (fraction, true)
        }
        Disclosed::Percentage { proportion } => (proportion.clamp(0.0, 1.0), true),
        // A quarter index is drawn at the *bottom* of its band, so a token in
        // the 1-4 band never looks half full. Reading a coarse bar as more
        // precise than it is would defeat the point of coarsening it.
        Disclosed::Chunked { quarter } => ((*quarter as f32 / 4.0).clamp(0.0, 1.0), true),
        // No value, and none is invented. The bar is drawn full in the
        // undisclosed colour so its presence is visible and its state is not.
        Disclosed::Greyed => (1.0, false),
    }
}

/// Rebuild a token's bars whenever its status changes.
///
/// Despawn-and-rebuild rather than mutating in place: the number of bars
/// changes when a system's declarations change or a viewer's entitlement
/// does, and a diffing update would be more code to get the same picture.
/// This runs only on `Changed<TokenStatus>`, so it is not a per-frame cost.
fn redraw_changed_status(
    mut commands: Commands,
    changed: Query<(Entity, &TokenStatus), Changed<TokenStatus>>,
    existing: Query<(Entity, &ChildOf), With<StatusGeometry>>,
) {
    for (token_entity, status) in changed.iter() {
        // Clear what this plugin drew last time, and nothing else.
        for (geometry, parent) in existing.iter() {
            if parent.parent() == token_entity {
                commands.entity(geometry).despawn();
            }
        }

        // A token with nothing to show gets no furniture at all — not an
        // empty track, which would read as "a resource at zero".
        if status.resources.is_empty() {
            continue;
        }

        let mut ordered: Vec<&ResolvedResource> = status.resources.iter().collect();
        // The system's declared order, not ours.
        ordered.sort_by_key(|r| r.definition.order);

        for (row, resource) in ordered.iter().enumerate() {
            let y = FIRST_BAR_OFFSET + row as f32 * (BAR_HEIGHT + BAR_GAP);
            let (fraction, disclosed) = fill_of(&resource.disclosed);

            // The track.
            commands.entity(token_entity).with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(TRACK_COLOR, Vec2::new(BAR_WIDTH, BAR_HEIGHT)),
                    Transform::from_xyz(0.0, y, BAR_Z),
                    StatusGeometry,
                ));

                if fraction <= 0.0 {
                    return;
                }

                let fill_color = if disclosed {
                    DEFAULT_FILL
                } else {
                    UNDISCLOSED_COLOR
                };
                let width = BAR_WIDTH * fraction;
                // Left-aligned inside the track: a bar that shrinks toward
                // its centre is unreadable at a glance.
                let x = -(BAR_WIDTH - width) / 2.0;

                parent.spawn((
                    Sprite::from_color(fill_color, Vec2::new(width, BAR_HEIGHT)),
                    Transform::from_xyz(x, y, BAR_Z + 0.1),
                    StatusGeometry,
                ));
            });
        }
    }
}

/// Look up a token entity by its server id.
///
/// The read surface (`getTokenStatus`) needs this, and so does the command
/// that sets status.
pub fn entity_for_token(
    tokens: &Query<(Entity, &TokenIdentity)>,
    token_id: &str,
) -> Option<Entity> {
    tokens
        .iter()
        .find(|(_, identity)| identity.0 == token_id)
        .map(|(entity, _)| entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_canvas_core::resource_display::ResourceEntry;

    // NOTE: these compile but do not execute — the engine crate has no test
    // runner for wasm32 (Constitution V). They are kept because they document
    // the intended mapping and will run the day a runner exists; the rules
    // they touch are covered by executing tests in
    // `thunderforge-canvas-core::resource_display`.

    fn entries(current: i32, max: i32) -> Vec<ResourceEntry> {
        vec![ResourceEntry {
            current,
            max: Some(max),
            label: None,
        }]
    }

    #[test]
    fn a_visible_resource_fills_by_its_real_proportion() {
        let (fraction, disclosed) = fill_of(&Disclosed::Visible {
            entries: entries(30, 100),
        });
        assert!((fraction - 0.3).abs() < 0.001);
        assert!(disclosed);
    }

    #[test]
    fn a_greyed_resource_is_drawn_full_and_marked_undisclosed() {
        let (fraction, disclosed) = fill_of(&Disclosed::Greyed);
        assert_eq!(fraction, 1.0, "presence is shown");
        assert!(!disclosed, "and its state is not");
    }

    #[test]
    fn a_chunked_resource_is_drawn_at_the_bottom_of_its_band() {
        let (fraction, _) = fill_of(&Disclosed::Chunked { quarter: 1 });
        assert!((fraction - 0.25).abs() < 0.001);
    }
}
