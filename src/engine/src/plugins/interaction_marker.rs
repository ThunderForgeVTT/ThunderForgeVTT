//! Showing that something on the map can be interacted with.
//!
//! # Why anything is needed
//!
//! Interactives drew nothing. A placed lore page or item was invisible: you
//! could only activate one by already knowing where it was, which makes the
//! feature unusable for the player it exists for and hard to author for the
//! Game Master who placed it.
//!
//! # Why the engine draws it and not React
//!
//! A marker is a thing *in the world*. It sits at a world position, moves with
//! its subject, and has to survive a camera that pans and zooms. Chrome could
//! only place it by knowing the camera, which means either duplicating the
//! projection or having the engine publish screen positions every frame —
//! both of which make React a second source of truth for canvas state, the
//! failure Constitution Principle I exists to prevent.
//!
//! The text-placement bug is the cautionary case: a listener bound to the
//! React container never fired, because the real `<canvas>` is a `body`-level
//! element winit inserts itself. Anything positional belongs here.
//!
//! # Colour rather than iconography, for now
//!
//! A lucide glyph would have to become a texture and an asset-loading path.
//! Colour by *namespace* carries the distinction that matters at a glance —
//! this is a lore page, that is an item — and costs one sprite. An icon
//! atlas is a later refinement of the same system, not a different one.

use bevy::prelude::*;

use crate::TokenEntities;
use crate::plugins::interaction::Interactives;

/// Marks a badge so it can be found and removed without tracking ids.
///
/// Deliberately carries nothing. The badges are rebuilt from `Interactives`
/// each frame rather than diffed, so there is no id to match against — and a
/// field kept "in case" would be a second, quietly stale copy of what the
/// resource already knows.
#[derive(Component)]
struct InteractionMarker;

/// How far above the subject's centre the badge sits.
///
/// Above rather than centred: a badge over the middle of a token hides the art
/// the Game Master chose, and a prop is often small enough that the badge would
/// be most of what you see.
const MARKER_OFFSET_Y: f32 = 26.0;
const MARKER_SIZE: f32 = 14.0;

/// Drawn in front of tokens, behind the placement preview.
const MARKER_Z: f32 = 400.0;

/// Colour by effect namespace.
///
/// The namespace rather than the full id, so a subsystem contributing a second
/// effect does not need a new colour and a decision about it — `door.set_lock`
/// and `door.set_state` are both doors to a player glancing at the map.
fn marker_color(effect_id: &str) -> Color {
    match effect_id.split('.').next().unwrap_or_default() {
        "lore" => Color::srgb(0.42, 0.62, 0.95),
        "item" => Color::srgb(0.95, 0.78, 0.35),
        "door" => Color::srgb(0.72, 0.55, 0.38),
        "light" => Color::srgb(0.98, 0.90, 0.55),
        "nav" => Color::srgb(0.60, 0.85, 0.62),
        // An effect this build does not know how to colour still gets a badge.
        // Showing nothing would make an unrecognised interactive invisible,
        // which is the state this module exists to end.
        _ => Color::srgb(0.75, 0.75, 0.78),
    }
}

/// Keep one badge per interactive that has a visible subject.
///
/// Rebuilt from `Interactives` each frame rather than diffed. The set is small
/// — a scene's interactives, not its tokens — and a diff would need its own
/// bookkeeping to stay correct across scene switches, which is exactly the
/// kind of state that goes stale silently.
fn sync_interaction_markers(
    mut commands: Commands,
    interactives: Res<Interactives>,
    token_entities: Res<TokenEntities>,
    transforms: Query<&Transform, Without<InteractionMarker>>,
    existing: Query<Entity, With<InteractionMarker>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    for interactive in interactives.iter() {
        // Only interactives attached to something drawable. A region has
        // geometry rather than a subject and is deliberately invisible to
        // players (spec 030), so it gets no badge here.
        let Some(subject_ref) = interactive.subject_ref.as_ref() else {
            continue;
        };
        let Some(entity) = token_entities.0.get(subject_ref) else {
            continue;
        };
        let Ok(transform) = transforms.get(*entity) else {
            continue;
        };

        let effect_id = interactive.effect_id.as_deref().unwrap_or_default();

        commands.spawn((
            Sprite::from_color(marker_color(effect_id), Vec2::splat(MARKER_SIZE)),
            Transform::from_xyz(
                transform.translation.x,
                transform.translation.y + MARKER_OFFSET_Y,
                MARKER_Z,
            ),
            InteractionMarker,
        ));
    }
}

pub struct InteractionMarkerPlugin;

impl Plugin for InteractionMarkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_interaction_markers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_namespace_decides_the_colour() {
        // Both door effects are one colour: a player glancing at the map sees
        // "a door", not "a door whose lock state can be set".
        assert_eq!(marker_color("door.set_lock"), marker_color("door.set_state"));
        assert_ne!(marker_color("lore.open"), marker_color("item.pickup"));
    }

    #[test]
    fn an_unknown_effect_still_gets_a_badge() {
        // The alternative is an invisible interactive, which is the bug this
        // module exists to fix.
        let unknown = marker_color("something.new");
        assert_eq!(unknown, Color::srgb(0.75, 0.75, 0.78));
    }
}
