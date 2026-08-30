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
use thunderforge_canvas_core::resource_display::{
    Disclosed, DisplayAppearance, Precision, ResourceDefinition, Rgb, bar_fill, fill_for_precision,
};

/// Width of a bar, matched to the token so the two read as one object.
///
/// Not part of `DisplayAppearance` on purpose: it is derived from the token,
/// so an application overriding it could only ever make bars that no longer
/// line up with what they describe.
const BAR_WIDTH: f32 = TOKEN_SIZE.x;

/// Drawn above the token sprite, below any selection furniture.
const BAR_Z: f32 = 5.0;

/// The appearance every status display is drawn with.
///
/// FR-022: these values are supplied by the application rather than compiled
/// in here, and FR-023: the documented default set lives in exactly one
/// place, which is `DisplayAppearance::default()` in canvas-core. This
/// resource is that set until the application replaces it.
///
/// A Bevy resource rather than a static, so `setDisplayAppearance` can change
/// it at runtime and the next redraw picks it up without a restart.
#[derive(Resource, Debug, Clone, Deref)]
pub struct Appearance(pub DisplayAppearance);

impl Default for Appearance {
    fn default() -> Self {
        Self(DisplayAppearance::default())
    }
}

/// Turn a canvas-core colour into a Bevy one.
///
/// The two crates deliberately do not share a colour type: canvas-core is
/// compiled by the server as well, and it has no business depending on a
/// rendering engine to describe a shade of red.
fn rgb_to_color((r, g, b): Rgb, alpha: f32) -> Color {
    Color::srgba(r, g, b, alpha)
}

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
        app.init_resource::<Appearance>()
            .add_systems(Update, redraw_changed_status);
    }
}

/// The world-space rectangle the camera can see, widened enough to cover a
/// token's bars.
///
/// `None` when there is no orthographic camera to ask, which is treated as
/// "draw everything" — a missing camera must not silently blank every display
/// in the scene, because that failure looks exactly like the feature being
/// broken.
fn visible_region(
    cameras: &Query<(&Transform, &Projection), (With<Camera2d>, Without<TokenStatus>)>,
) -> Option<Rect> {
    let (transform, projection) = cameras.iter().next()?;
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };

    // Bars sit above the token, so a token whose centre is just below the
    // bottom edge still has geometry on screen. The margin covers a token and
    // a generous stack of bars rather than being tuned to the current
    // appearance, which the application can change at any time.
    const MARGIN: f32 = TOKEN_SIZE.y * 2.0;
    let centre = transform.translation.truncate();
    Some(Rect {
        min: centre + ortho.area.min - Vec2::splat(MARGIN),
        max: centre + ortho.area.max + Vec2::splat(MARGIN),
    })
}

/// Rebuild a token's bars whenever its status changes.
///
/// Despawn-and-rebuild rather than mutating in place: the number of bars
/// changes when a system's declarations change or a viewer's entitlement
/// does, and a diffing update would be more code to get the same picture.
///
/// This runs only on `Changed<TokenStatus>` and camera movement, so it is not
/// a per-frame cost.
fn redraw_changed_status(
    mut commands: Commands,
    tokens: Query<(Entity, Ref<TokenStatus>, &Transform)>,
    existing: Query<(Entity, &ChildOf), With<StatusGeometry>>,
    appearance: Res<Appearance>,
    cameras: Query<(&Transform, &Projection), (With<Camera2d>, Without<TokenStatus>)>,
    mut last_view: Local<Option<Rect>>,
) {
    // A change to the appearance has to repaint bars that are already on
    // screen. Keying only on `Changed<TokenStatus>` would leave every
    // existing token wearing the old palette until something else happened
    // to it — so the new colours would appear to work when demonstrated on
    // a fresh scene and do nothing in a session already in progress.
    // FR-026: a token nowhere near the camera must not pay for bars nobody can
    // see. This is spawn-time culling rather than leaving it to the renderer's
    // frustum test, because the measured cost is not fill — it is that the
    // entities exist at all. With displays enabled a 3,200-token board carried
    // 16,003 sprites against 3,203 without, and ran at 20fps against 59.
    // Frustum culling would still walk all 16,003 every frame.
    let view = visible_region(&cameras);

    // Panning must bring bars back. The redraw is otherwise change-driven, so
    // without this a token scrolled into view would stay bare until something
    // else happened to it — which, for a token standing still, is never.
    //
    // Compared with a tolerance rather than exactly: a camera at rest still
    // jitters in the low bits, and float-equality would call that a move and
    // repaint every on-screen token every frame, turning an optimisation into
    // a per-frame cost. The tolerance is well under a token, so a real pan is
    // still picked up before anything reaches the edge.
    let view_moved = match (*last_view, view) {
        (Some(previous), Some(current)) => {
            const TOLERANCE: f32 = 8.0;
            (previous.min - current.min).abs().max_element() > TOLERANCE
                || (previous.max - current.max).abs().max_element() > TOLERANCE
        }
        (previous, current) => previous.is_some() != current.is_some(),
    };
    if view_moved {
        *last_view = view;
    }

    let repaint_everything = appearance.is_changed() || view_moved;

    for (token_entity, status, transform) in tokens.iter() {
        if !repaint_everything && !status.is_changed() {
            continue;
        }
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

        // Off-screen: the old geometry is already cleared above, and nothing
        // replaces it. Coming back into view is handled by `view_moved`.
        if let Some(region) = view
            && !region.contains(transform.translation.truncate())
        {
            continue;
        }

        let mut ordered: Vec<&ResolvedResource> = status.resources.iter().collect();
        // The system's declared order, not ours.
        ordered.sort_by_key(|r| r.definition.order);

        let first_bar_offset = TOKEN_SIZE.y / 2.0 + appearance.first_bar_offset;
        let bar_height = appearance.bar_height;
        let track_color = rgb_to_color(appearance.track, appearance.track_alpha);

        for (row, resource) in ordered.iter().enumerate() {
            let y = first_bar_offset + row as f32 * (bar_height + appearance.bar_gap);
            let (fraction, precision) = bar_fill(&resource.disclosed);

            // The track.
            commands.entity(token_entity).with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(track_color, Vec2::new(BAR_WIDTH, bar_height)),
                    Transform::from_xyz(0.0, y, BAR_Z),
                    StatusGeometry,
                ));

                if fraction <= 0.0 {
                    return;
                }

                // Indexed by the row this resource occupies, which is the
                // system's declared order — the engine still knows nothing
                // about what any of these resources mean.
                let fill_color = rgb_to_color(
                    fill_for_precision(appearance.fill_for(row), appearance.undisclosed, precision),
                    if precision == Precision::Withheld {
                        0.85
                    } else {
                        1.0
                    },
                );
                let width = BAR_WIDTH * fraction;
                // Left-aligned inside the track: a bar that shrinks toward
                // its centre is unreadable at a glance.
                let x = -(BAR_WIDTH - width) / 2.0;

                parent.spawn((
                    Sprite::from_color(fill_color, Vec2::new(width, bar_height)),
                    Transform::from_xyz(x, y, BAR_Z + 0.1),
                    StatusGeometry,
                ));
            });
        }
    }
}
