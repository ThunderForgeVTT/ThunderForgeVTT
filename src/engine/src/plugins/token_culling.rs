//! Tokens the camera cannot see carry nothing.
//!
//! A token outside the view — padded by a quarter of the view's size on every
//! side, so a fast pan or zoom never shows anything arriving — has its name
//! and its bars removed until it comes back. The padding, the two edges and
//! the arithmetic behind them live in `thunderforge_canvas_core::viewport`;
//! this plugin applies them to the board.
//!
//! # What is culled, and what is not
//!
//! Only the furniture *around* a token: the nameplate (`plugins::nameplate`)
//! and the status bars (`plugins::status_display`). Those are the entities
//! that multiply — a named token with two bars is seven entities, six of them
//! furniture — and the measured cost of a crowded board is that they exist,
//! not that they are filled (see `status_display`: 16,003 sprites at 20fps
//! against 3,203 at 59, on the same 3,200 tokens).
//!
//! The token itself is left alone, deliberately:
//!
//! - **Its sprite is already not drawn.** Bevy's frustum test drops an
//!   off-screen sprite before extraction, exactly, every frame. Nothing here
//!   could make that cheaper.
//! - **Its `Visibility` is not ours to write.** Lighting sets it every frame
//!   to say "this viewer cannot see this token"
//!   (`systems::lighting::apply_light_illumination`), and camera focus and the
//!   context menu's hit test both *read* `Visibility::Hidden` as exactly that.
//!   A second writer would either un-hide a token vision had hidden, or make a
//!   merely off-screen token refuse to be focused — and "look at my token" is
//!   asked precisely when the token is off-screen.
//!
//! So everything that reasons about tokens — selection by id, vision, carried
//! lights, movement sync, hit-testing, focus, the counts the host page reads —
//! sees the same tokens it always did. [`ViewportCull`] is a statement about
//! presentation and nothing else.
//!
//! # Change-driven
//!
//! Every token is judged when the view has drifted by more than 2% of its
//! size, when the grid changes (tokens resize), when the set of tokens that
//! may never be culled changes, or when culling is switched. Otherwise only a
//! token that moved, resized or just arrived is judged — so a camera at rest
//! over a still board does no per-token work at all, and a token walking in
//! from outside is picked up on the frame it crosses the edge.

use bevy::prelude::*;

use crate::TOKEN_SIZE;
use crate::TokenIdentity;
use crate::resources::{DraggingToken, SceneGrid, SelectedToken, TokenGridBehaviour};
use crate::systems::lighting_vision::ViewerToken;
use crate::systems::token_move::ControlledToken;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::viewport::{CullBand, VIEW_DRIFT, ViewRect};

/// How far a token's furniture reaches beyond its body, in world units.
///
/// A token and a generous stack of bars with a name above them, rather than a
/// figure tuned to the current appearance, which the application can change at
/// any time. The same allowance `status_display` made when it culled on its
/// own.
const FURNITURE_REACH: f32 = TOKEN_SIZE.y * 2.0;

/// Whether a token's furniture is currently withheld because nobody can see
/// it.
///
/// On every token (`TokenIdentity` requires it), present by default. Written
/// only when the answer changes, so `Changed<ViewportCull>` is a transition
/// and the plugins that draw furniture can react to it without watching the
/// camera themselves.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewportCull {
    pub culled: bool,
}

/// Whether culling is in force. On unless switched off.
///
/// Switchable with the `set_token_culling` world command so its effect can be
/// measured on one board in one session — the same reason `set_render_probe`
/// is a command. Off, every token is present, as before this plugin existed.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCulling {
    pub enabled: bool,
}

impl Default for TokenCulling {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// How many tokens the last frame judged.
///
/// Zero on a frame where nothing moved, which is the claim this plugin makes
/// about a camera at rest — and a claim is only worth making if something can
/// check it.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenCullWork {
    pub judged: usize,
}

/// The system that decides. Anything that draws token furniture runs after
/// it, so a token created off-screen is culled before its furniture is ever
/// built rather than a frame after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenCullSet;

pub struct TokenCullingPlugin;

impl Plugin for TokenCullingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TokenCulling>()
            .init_resource::<TokenCullWork>()
            .add_systems(Update, cull_tokens_to_view.in_set(TokenCullSet));
    }
}

/// What the last full pass was made against.
#[derive(Default)]
struct Evaluated {
    /// The view every token was last judged against, and the band built from
    /// it. `None` while culling is off or there is no camera to ask.
    view: Option<(ViewRect, CullBand)>,
    /// The tokens that may not be culled, sorted.
    exempt: Vec<String>,
}

/// The world-space rectangle the camera shows.
///
/// `None` when there is no orthographic camera to ask, which is treated as
/// "cull nothing" — a missing camera must not silently strip every name and
/// bar in the scene, because that failure looks exactly like the feature being
/// broken.
fn camera_view(cameras: &Query<(&Transform, &Projection), With<Camera2d>>) -> Option<ViewRect> {
    let (transform, projection) = cameras.iter().next()?;
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };
    let centre = transform.translation.truncate();
    let view = ViewRect::from_corners(centre + ortho.area.min, centre + ortho.area.max);
    // Before the first resize the projection's area is empty, and an empty
    // view would cull the whole board for a frame.
    let size = view.size();
    (size.x > 0.0 && size.y > 0.0 && size.is_finite()).then_some(view)
}

/// How far a token and its furniture extend from its centre.
fn token_reach(
    grid: Option<&SceneGrid>,
    behaviour: Option<&TokenGridBehaviour>,
    transform: &Transform,
) -> f32 {
    let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
    let side = grid.map_or(TOKEN_SIZE.x, |grid| footprint.world_size(grid.size));
    // A token someone has enlarged reaches further. Overestimating only keeps
    // a token present a little longer; underestimating would cut one off.
    let scale = transform.scale.truncate().abs().max_element().max(1.0);
    side * 0.5 * scale + FURNITURE_REACH
}

/// The tokens that are never culled: whatever the player is holding, has
/// selected, sees through or steers.
///
/// Those are the tokens a player is about to look at or is already acting on,
/// and none of them is ever numerous — so exempting them costs nothing and
/// removes any chance of a name vanishing from the token in someone's hand.
fn exempt_ids<'a>(
    selected: Option<&'a SelectedToken>,
    dragging: Option<&'a DraggingToken>,
    viewer: Option<&'a ViewerToken>,
    controlled: Option<&'a ControlledToken>,
) -> Vec<&'a str> {
    let mut ids: Vec<&str> = Vec::new();
    if let Some(selected) = selected {
        ids.extend(selected.selected_ids().iter().map(String::as_str));
    }
    if let Some(dragging) = dragging {
        ids.extend(dragging.0.iter().map(|d| d.id.as_str()));
    }
    if let Some(id) = viewer.and_then(|v| v.0.as_deref()) {
        ids.push(id);
    }
    if let Some(id) = controlled.and_then(|c| c.0.as_deref()) {
        ids.push(id);
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn cull_tokens_to_view(
    culling: Res<TokenCulling>,
    mut work: ResMut<TokenCullWork>,
    grid: Option<Res<SceneGrid>>,
    selected: Option<Res<SelectedToken>>,
    dragging: Option<Res<DraggingToken>>,
    viewer: Option<Res<ViewerToken>>,
    controlled: Option<Res<ControlledToken>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    // One query walked with change ticks in hand, rather than a filtered
    // query for "what moved" beside a mutable one for "everything": a filter
    // on `ViewportCull` beside `&mut ViewportCull` is a conflict Bevy refuses
    // at startup.
    mut tokens: Query<(
        Ref<Transform>,
        &TokenIdentity,
        Option<Ref<TokenGridBehaviour>>,
        &mut ViewportCull,
    )>,
    mut evaluated: Local<Evaluated>,
) {
    let view = culling.enabled.then(|| camera_view(&cameras)).flatten();

    // Has the view moved far enough that tokens judged against the old one
    // might now be wrong? Switching culling, or gaining or losing the camera,
    // counts: it changes the answer for everything.
    let view_moved = match (&evaluated.view, &view) {
        (Some((earlier, _)), Some(current)) => current.drifted_from(earlier, VIEW_DRIFT),
        (earlier, current) => earlier.is_some() != current.is_some(),
    };
    if view_moved {
        evaluated.view = view.map(|view| (view, CullBand::around(view)));
    }

    let exempt = exempt_ids(
        selected.as_deref(),
        dragging.as_deref(),
        viewer.as_deref(),
        controlled.as_deref(),
    );
    let exempt_changed = !exempt
        .iter()
        .copied()
        .eq(evaluated.exempt.iter().map(String::as_str));
    if exempt_changed {
        evaluated.exempt = exempt.iter().map(|id| (*id).to_string()).collect();
    }

    // A new grid resizes every token, so it changes every token's reach.
    let grid_changed = grid.as_ref().is_some_and(|grid| grid.is_changed());
    let judge_everything = view_moved || exempt_changed || grid_changed;

    // The band from the last full pass, not from this frame's view: tokens
    // judged one at a time between passes must agree with their neighbours,
    // and `SHOW_PAD` is sized so the older band still covers the promise.
    let band = evaluated.view.map(|(_, band)| band);

    let mut judged = 0;
    for (transform, identity, behaviour, mut cull) in tokens.iter_mut() {
        let moved = transform.is_changed()
            || behaviour.as_ref().is_some_and(|b| b.is_changed())
            || cull.is_added();
        if !judge_everything && !moved {
            continue;
        }
        judged += 1;

        let culled = match band {
            None => false,
            Some(_) if exempt.binary_search(&identity.0.as_str()).is_ok() => false,
            Some(band) => band.culls(
                transform.translation.truncate(),
                token_reach(grid.as_deref(), behaviour.as_deref(), &transform),
                cull.culled,
            ),
        };
        // Compared before writing, so `Changed<ViewportCull>` means the
        // answer changed and the furniture plugins rebuild only then.
        if cull.culled != culled {
            cull.culled = culled;
        }
    }
    if work.judged != judged {
        work.judged = judged;
    }
}

#[cfg(test)]
#[path = "token_culling_tests.rs"]
mod tests;
