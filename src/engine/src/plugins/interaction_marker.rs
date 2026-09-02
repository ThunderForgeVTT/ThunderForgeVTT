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
//! # Colour, and a shape where the shape is worth the sprites
//!
//! This module used to say that a lucide glyph would have to become a texture
//! and an asset-loading path, and that an icon atlas was a later refinement of
//! the same system. This is that refinement, taken as far as it is honest to
//! take it without the asset path.
//!
//! Spec 031 FR-012 asks that a placed lore page read as *a book you can open*.
//! Colour alone does not say that: at the table a blue square is a blue square,
//! and a Game Master who placed it is the only person who knows what it means.
//! So a badge is now a small composition of sprites rather than one, and the
//! `lore` badge composes into an open book — two pale pages either side of a
//! spine, which is the same silhouette `lucide-react`'s `BookOpen` draws in
//! the authoring panel that placed it.
//!
//! # Why not the actual lucide texture
//!
//! Because it is not a texture yet, and making it one is bigger than this.
//! `lib.rs` documents an invariant — every asset this engine loads is a
//! same-origin, server-authorized URL under `/api/canvas-assets/...`, and
//! `systems/background.rs` holds the only two `asset_server.load` call sites.
//! A bundled glyph atlas would be a third kind of asset with a different
//! provenance, a new served path, and a decode-time failure mode on a badge
//! that exists precisely so an interactive is never invisible. Rectangles
//! cannot fail to load.
//!
//! The composition stays deliberately coarse — a badge is 14 world units, and
//! detail below a couple of units is a smear at any zoom a person plays at.
//! It carries the one distinction a glance needs: this is a book, that is not.

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

/// How far apart the parts of one badge are stacked in depth.
///
/// Small enough that no other layer can slip between two parts of the same
/// badge, which is the only ordering this needs to guarantee.
const PART_Z_STEP: f32 = 0.1;

/// A pale colour for whatever the namespace colour is *behind*.
///
/// One shared page/highlight colour rather than one derived per namespace: it
/// has to read as paper against every badge colour, and a derived tint would
/// be five different judgements about contrast instead of one.
const MARKER_HIGHLIGHT: Color = Color::srgb(0.96, 0.95, 0.90);

/// One rectangle of a badge, positioned relative to the badge's centre.
///
/// Relative rather than absolute so a composition can be written once and read
/// as a shape. Absolute positions would each have to repeat the subject's
/// transform and the vertical offset, which is three chances per part to put a
/// page somewhere its badge is not.
struct MarkerPart {
    offset: Vec2,
    size: Vec2,
    color: Color,
}

/// The sprites that make up one badge, in draw order, back to front.
///
/// The first part is always the whole badge in its namespace colour, so the
/// colour distinction this module started with survives every shape added
/// after it — a badge that gains a silhouette does not lose its colour, and a
/// namespace with no silhouette is still exactly the square it always was.
fn marker_parts(effect_id: &str) -> Vec<MarkerPart> {
    let color = marker_color(effect_id);
    let base = MarkerPart {
        offset: Vec2::ZERO,
        size: Vec2::splat(MARKER_SIZE),
        color,
    };

    match effect_id.split('.').next().unwrap_or_default() {
        // An open book: two pages with the board showing between them as the
        // spine. Drawn by *omission* — the gap between the pages is the base
        // part, so the spine can never disagree with the badge colour.
        "lore" => vec![
            base,
            MarkerPart {
                offset: Vec2::new(-MARKER_SIZE * 0.24, MARKER_SIZE * 0.04),
                size: Vec2::new(MARKER_SIZE * 0.36, MARKER_SIZE * 0.62),
                color: MARKER_HIGHLIGHT,
            },
            MarkerPart {
                offset: Vec2::new(MARKER_SIZE * 0.24, MARKER_SIZE * 0.04),
                size: Vec2::new(MARKER_SIZE * 0.36, MARKER_SIZE * 0.62),
                color: MARKER_HIGHLIGHT,
            },
        ],
        // Every other namespace keeps the plain square it had. Inventing a
        // silhouette for each one now would be five guesses at what a player
        // reads at a glance, made without a playtest to correct them; colour
        // already carries the distinction, and this is the badge that a
        // playtest actually said was unreadable.
        _ => vec![base],
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

        for (index, part) in marker_parts(effect_id).into_iter().enumerate() {
            commands.spawn((
                Sprite::from_color(part.color, part.size),
                Transform::from_xyz(
                    transform.translation.x + part.offset.x,
                    transform.translation.y + MARKER_OFFSET_Y + part.offset.y,
                    // Later parts sit in front of earlier ones. A fixed step
                    // rather than a per-part z: the parts of one badge are
                    // authored in draw order, and giving each its own depth
                    // would be a second place to get that order wrong.
                    MARKER_Z + index as f32 * PART_Z_STEP,
                ),
                InteractionMarker,
            ));
        }
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

    #[test]
    fn every_badge_starts_with_the_namespace_colour() {
        // The property that must survive any silhouette added later: shape is
        // drawn on top of the colour, never instead of it.
        for effect_id in ["lore.open", "item.pickup", "door.set_state", "???"] {
            let parts = marker_parts(effect_id);
            assert_eq!(parts[0].color, marker_color(effect_id));
            assert_eq!(parts[0].size, Vec2::splat(MARKER_SIZE));
        }
    }

    #[test]
    fn a_lore_badge_is_a_book_and_the_others_are_not() {
        // Two pages either side of a spine. Asserted as "more than the base
        // part, symmetric about the centre" rather than by exact geometry,
        // because the sizes are a look and will be tuned; the symmetry is the
        // thing that makes it read as a book rather than as a smudge.
        let book = marker_parts("lore.open");
        assert_eq!(book.len(), 3);
        assert_eq!(book[1].offset.x, -book[2].offset.x);
        assert_eq!(book[1].size, book[2].size);
        assert_eq!(book[1].color, MARKER_HIGHLIGHT);

        assert_eq!(marker_parts("item.pickup").len(), 1);
    }

    #[test]
    fn no_part_escapes_the_badge() {
        // A part wider than the badge would overlap the token art the offset
        // exists to keep clear, and would do it silently.
        for effect_id in ["lore.open", "item.pickup", "nav.request_scene"] {
            for part in marker_parts(effect_id) {
                let half = MARKER_SIZE / 2.0 + f32::EPSILON;
                assert!(part.offset.x.abs() + part.size.x / 2.0 <= half);
                assert!(part.offset.y.abs() + part.size.y / 2.0 <= half);
            }
        }
    }
}
