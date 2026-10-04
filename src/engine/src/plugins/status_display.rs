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
//! # Why bars are drawn here and the pinned panel is not
//!
//! Constitution Principle I: the ECS owns what is spatial, React owns chrome.
//! A bar above a token tracks its position, scales with the camera and
//! reorders with other entities, so it belongs to the engine. The pinned
//! status panel (spec 029 FR-011a) is screen-space text and belongs in React,
//! where it keeps screen readers, text selection and browser zoom — all of
//! which would have to be reimplemented to draw it in WebGL. See ADR-053.
//!
//! # Sized from the token, not from a constant
//!
//! Playtest 2026-09-10 P6, spec 029 FR-010a. Bars used to be `TOKEN_SIZE`
//! wide (96) and started `TOKEN_SIZE.y / 2` above the token's centre — while
//! the token itself is sized to its footprint on the grid
//! (`systems::token_grid::size_tokens_to_grid`). On any grid that is not 96 a
//! cell, the bars sat inside a large token or floated off a small one. They
//! now take the same side length the token does, so they sit above it and span
//! it at any grid size. The bars are the token's children, so its own scale
//! carries them along as it does the sprite.

use bevy::prelude::*;

use crate::TOKEN_SIZE;
use crate::plugins::token_culling::{TokenCullSet, ViewportCull};
use crate::resources::{SceneGrid, TokenGridBehaviour};
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::resource_display::{
    Disclosed, DisplayAppearance, Precision, ResourceDefinition, Rgb, bar_fill, fill_for_precision,
};

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
#[derive(Resource, Debug, Clone, Deref, Default)]
pub struct Appearance(pub DisplayAppearance);

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
pub(crate) struct StatusGeometry;

pub struct StatusDisplayPlugin;

impl Plugin for StatusDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Appearance>()
            // After the culling decision, so a token that arrives off-screen
            // never has bars built for it at all.
            .add_systems(Update, redraw_changed_status.after(TokenCullSet));
    }
}

/// A token's side length in world units — the one its sprite is sized to.
///
/// The same calculation `size_tokens_to_grid` makes, so the bars and the token
/// cannot disagree about how big the token is. With no grid loaded yet, the
/// token is the default size and so are its bars.
fn token_side(grid: Option<&SceneGrid>, behaviour: Option<&TokenGridBehaviour>) -> f32 {
    let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
    grid.map_or(TOKEN_SIZE.x, |grid| footprint.world_size(grid.size))
}

/// Rebuild a token's bars whenever its status changes.
///
/// Despawn-and-rebuild rather than mutating in place: the number of bars
/// changes when a system's declarations change or a viewer's entitlement
/// does, and a diffing update would be more code to get the same picture.
///
/// This acts only on a changed `TokenStatus`, a changed footprint or grid, a
/// changed appearance, and a token entering or leaving the padded view, so it
/// is not a per-frame cost.
#[allow(clippy::type_complexity)]
fn redraw_changed_status(
    mut commands: Commands,
    tokens: Query<(
        Entity,
        Ref<TokenStatus>,
        Option<&Children>,
        Option<Ref<TokenGridBehaviour>>,
        Option<Ref<ViewportCull>>,
    )>,
    existing: Query<(), With<StatusGeometry>>,
    appearance: Res<Appearance>,
    grid: Option<Res<SceneGrid>>,
) {
    // A change to the appearance has to repaint bars that are already on
    // screen. Keying only on `Changed<TokenStatus>` would leave every
    // existing token wearing the old palette until something else happened
    // to it — so the new colours would appear to work when demonstrated on
    // a fresh scene and do nothing in a session already in progress.
    //
    // A new grid resizes every token, so it resizes every token's bars.
    let grid_changed = grid.as_ref().is_some_and(|grid| grid.is_changed());
    let repaint_everything = appearance.is_changed() || grid_changed;

    for (token_entity, status, children, behaviour, cull) in tokens.iter() {
        let footprint_changed = behaviour.as_ref().is_some_and(|b| b.is_changed());
        // FR-026: a token nowhere near the camera must not pay for bars nobody
        // can see. This is spawn-time culling rather than leaving it to the
        // renderer's frustum test, because the measured cost is not fill — it
        // is that the entities exist at all. With displays enabled a
        // 3,200-token board carried 16,003 sprites against 3,203 without, and
        // ran at 20fps against 59. Frustum culling would still walk all 16,003
        // every frame.
        //
        // Which tokens those are is decided once, for names and bars alike, by
        // `plugins::token_culling` — the view padded by a quarter of its size.
        // This plugin used to keep a view of its own with a fixed 192-unit
        // margin and repaint *every* token whenever the camera moved 8 units;
        // it now repaints exactly the tokens that crossed the edge. A token
        // with no `ViewportCull` (the culling plugin is absent) is always
        // drawn, which is what this plugin did before either existed.
        let cull_changed = cull.as_ref().is_some_and(|c| c.is_changed());
        if !repaint_everything && !status.is_changed() && !footprint_changed && !cull_changed {
            continue;
        }
        // Clear what this plugin drew last time, and nothing else.
        if let Some(children) = children {
            for child in children.iter().filter(|c| existing.contains(*c)) {
                commands.entity(child).despawn();
            }
        }

        // A token with nothing to show gets no furniture at all — not an
        // empty track, which would read as "a resource at zero".
        if status.resources.is_empty() {
            continue;
        }

        // Out of view: the old geometry is already cleared above, and nothing
        // replaces it. Coming back is a change to `ViewportCull`, which lands
        // here again.
        if cull.as_ref().is_some_and(|c| c.culled) {
            continue;
        }

        let mut ordered: Vec<&ResolvedResource> = status.resources.iter().collect();
        // The system's declared order, not ours.
        ordered.sort_by_key(|r| r.definition.order);

        // Matched to the token so the two read as one object.
        let side = token_side(grid.as_deref(), behaviour.as_deref());
        let bar_width = side;
        let first_bar_offset = side / 2.0 + appearance.first_bar_offset;
        let bar_height = appearance.bar_height;
        let track_color = rgb_to_color(appearance.track, appearance.track_alpha);

        for (row, resource) in ordered.iter().enumerate() {
            let y = first_bar_offset + row as f32 * (bar_height + appearance.bar_gap);
            let (fraction, precision) = bar_fill(&resource.disclosed);

            // The track.
            commands.entity(token_entity).with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(track_color, Vec2::new(bar_width, bar_height)),
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
                let width = bar_width * fraction;
                // Left-aligned inside the track: a bar that shrinks toward
                // its centre is unreadable at a glance.
                let x = -(bar_width - width) / 2.0;

                parent.spawn((
                    Sprite::from_color(fill_color, Vec2::new(width, bar_height)),
                    Transform::from_xyz(x, y, BAR_Z + 0.1),
                    StatusGeometry,
                ));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_canvas_core::resource_display::ResourceKind;

    fn one_resource() -> TokenStatus {
        TokenStatus {
            resources: vec![ResolvedResource {
                definition: ResourceDefinition {
                    id: "hp".to_string(),
                    label: "HP".to_string(),
                    kind: ResourceKind::Bar,
                    order: 0,
                    allow_stacking: false,
                },
                // Withheld, so only the track is drawn: one sprite to measure.
                disclosed: Disclosed::Greyed,
            }],
        }
    }

    /// The track drawn for `token`: its width and its height above the token.
    fn track_of(app: &mut App, token: Entity) -> (f32, f32) {
        let mut tracks = app
            .world_mut()
            .query_filtered::<(&Sprite, &Transform, &ChildOf), With<StatusGeometry>>();
        let (sprite, transform, _) = tracks
            .iter(app.world())
            .find(|(_, _, parent)| parent.parent() == token)
            .expect("a track above the token");
        (
            sprite.custom_size.expect("a sized track").x,
            transform.translation.y,
        )
    }

    /// Playtest 2026-09-10 P6, spec 029 FR-010a: on a 50-unit grid a
    /// one-cell token's bars are 50 wide and start just above it — not 96
    /// wide and starting inside it — and a two-cell token's span its 100.
    #[test]
    fn bars_are_sized_from_the_tokens_footprint_on_the_grid() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatusDisplayPlugin);
        app.insert_resource(SceneGrid::from_server("square", 50.0, Vec2::ZERO));

        let small = app
            .world_mut()
            .spawn((Transform::default(), one_resource()))
            .id();
        let large = app
            .world_mut()
            .spawn((
                Transform::default(),
                one_resource(),
                TokenGridBehaviour {
                    footprint: Footprint::new(2.0),
                    snap: true,
                },
            ))
            .id();
        app.update();
        app.update();

        let offset = Appearance::default().first_bar_offset;

        let (width, y) = track_of(&mut app, small);
        assert!((width - 50.0).abs() < 1e-3, "one cell of 50, got {width}");
        assert!(
            (y - (25.0 + offset)).abs() < 1e-3,
            "just above a 50-unit token, got {y}"
        );

        let (width, y) = track_of(&mut app, large);
        assert!((width - 100.0).abs() < 1e-3, "two cells of 50, got {width}");
        assert!(
            (y - (50.0 + offset)).abs() < 1e-3,
            "just above a 100-unit token, got {y}"
        );
    }
}
