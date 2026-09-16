//! "Look at this creature": the camera, moved to a token the viewer may see.
//!
//! # Why this is the engine's job
//!
//! The camera is the engine's (Constitution Principle I), and so is every
//! question this has to answer: where the token is, how many cells it fills,
//! how big the viewport is, whether this board draws the token at all. The
//! application asks by name — `focus_token` — and is told nothing it did not
//! already know.
//!
//! # Who may look at what
//!
//! A Game Master may look at anything. Anyone else may look at a token only
//! when **both**:
//!
//! - the board draws it — a token hidden by `systems::lighting`'s vision pass
//!   carries `Visibility::Hidden`, and a camera that flew to a blank patch of
//!   floor would tell a player exactly where the thing they cannot see is; and
//! - its name is one they may read — `TokenName::hidden_from_players`, which
//!   the server has already resolved to the effective rule for a player
//!   (`auth::npc_visibility::player_may_read_token_name`: the token's own
//!   switch **and** the creature it stands for being one players may see).
//!
//! Stated here, once, because this is the only place a refusal can actually
//! be enforced. The web hides the control for the same two reasons, but a
//! hidden control is chrome; this is the rule.
//!
//! # The motion
//!
//! `CameraManager` already eases toward a target (`advance`), so a focus sets
//! the target and lets the existing glide carry the camera there — roughly
//! 150ms, the same motion a cursor-anchored zoom uses. `immediate` lands it
//! in one frame instead, which is what `prefers-reduced-motion` asks for.
//!
//! A focus that arrives while someone is dragging the map is dropped rather
//! than queued: the hand on the mouse wins, and a camera that fought a drag
//! for 150ms would read as the map tearing itself out of the user's grip.

use bevy::prelude::*;

use crate::TokenIdentity;
use crate::plugins::nameplate::TokenName;
use crate::resources::{CameraManager, IsGameMaster, SceneGrid, TokenGridBehaviour};

/// A request to put a token on screen.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FocusRequest {
    pub token_id: String,
    /// How many cells of surroundings to frame around the creature, or `None`
    /// to keep the current zoom and only move.
    pub surround_cells: Option<f32>,
    /// Land in one frame rather than gliding.
    pub immediate: bool,
}

/// Whether a pan drag is in progress, so a focus can stand aside for it.
///
/// A resource rather than the `Local` the drag already keeps, because the
/// thing that needs to know is a different system.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanDragActive(pub bool);

static REQUESTED_FOCUS: std::sync::OnceLock<std::sync::Mutex<Option<FocusRequest>>> =
    std::sync::OnceLock::new();

/// Asks the camera to look at a token. Applied on the next frame.
///
/// A second request before the first is applied replaces it: the last thing
/// asked for is the thing wanted, and flying through an abandoned target on
/// the way would be motion nobody asked for.
pub(crate) fn request_focus(request: FocusRequest) {
    let slot = REQUESTED_FOCUS.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(request);
    }
}

/// What became of the last focus asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Outcome {
    #[default]
    None,
    /// The camera moved.
    Moved,
    /// No token by that id — it has not arrived, or never existed.
    Unknown,
    /// This board does not draw it.
    Unseen,
    /// This viewer may not read its name.
    Unnamed,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::None => "none",
            Outcome::Moved => "moved",
            Outcome::Unknown => "unknown",
            Outcome::Unseen => "unseen",
            Outcome::Unnamed => "unnamed",
        }
    }
}

/// The last `focus_token` and what became of it.
///
/// A resource as well as the mirror below, so a test reads this world's own
/// answer rather than a process-global one. The globals in this crate are
/// shared by every test in the binary, which is how two of them have already
/// come to fail at random under the default thread count.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LastFocus {
    pub token_id: String,
    pub outcome: Outcome,
}

static FOCUS_STATE: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();

fn record(last: &mut LastFocus, token_id: &str, outcome: Outcome) {
    *last = LastFocus {
        token_id: token_id.to_string(),
        outcome,
    };
    let json = format!(
        "{{\"tokenId\":{},\"outcome\":\"{}\"}}",
        serde_json::Value::from(token_id),
        outcome.as_str(),
    );
    let slot = FOCUS_STATE.get_or_init(|| std::sync::Mutex::new(String::from("{}")));
    if let Ok(mut held) = slot.lock() {
        *held = json;
    }
}

/// What became of the last `focus_token`, as
/// `{"tokenId":…,"outcome":"moved"|"unknown"|"unseen"|"unnamed"}`.
///
/// Read-only, and here for the same reason `hidden_tokens` is: a camera that
/// did not move has several possible causes, and a test should be able to ask
/// which one rather than infer it from pixels. A refusal in particular is
/// invisible by design — nothing is drawn, which is the point — so without
/// this there would be no way to tell a refusal from a bug.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn focus_state() -> String {
    FOCUS_STATE
        .get()
        .and_then(|slot| slot.lock().ok().map(|held| held.clone()))
        .unwrap_or_else(|| String::from("{}"))
}

/// The smallest square, in world units, that frames a creature of
/// `footprint_cells` together with `surround_cells` of its surroundings.
///
/// The surroundings are counted **around** the creature, not through it: a
/// Large token asked for six cells of context gets six cells past its own
/// two, not four cells and a body filling the rest of the frame. That is the
/// whole reason the engine computes this and the web does not — the web does
/// not know the footprint.
pub(crate) fn framed_extent(footprint_cells: f32, surround_cells: f32, cell_size: f32) -> f32 {
    (footprint_cells + surround_cells.max(0.0) * 2.0) * cell_size.max(f32::EPSILON)
}

/// Applies a pending `focus_token`.
pub(crate) fn apply_requested_focus(
    mut camera_mgr: ResMut<CameraManager>,
    mut last: ResMut<LastFocus>,
    grid: Option<Res<SceneGrid>>,
    is_game_master: Option<Res<IsGameMaster>>,
    dragging: Option<Res<PanDragActive>>,
    tokens: Query<(
        &Transform,
        &TokenIdentity,
        &Visibility,
        Option<&TokenName>,
        Option<&TokenGridBehaviour>,
    )>,
    viewport: Query<&Camera, With<Camera2d>>,
) {
    let Some(slot) = REQUESTED_FOCUS.get() else {
        return;
    };
    let Ok(mut pending) = slot.lock() else {
        return;
    };
    // Taken whether or not it is acted on. A request held back would arrive
    // one frame after the drag ended, which is a camera moving for no reason
    // the user can see.
    let Some(request) = pending.take() else {
        return;
    };
    drop(pending);

    if dragging.is_some_and(|drag| drag.0) {
        return;
    }

    let game_master = is_game_master.is_some_and(|gm| gm.0);

    let Some((transform, _, visibility, name, behaviour)) = tokens
        .iter()
        .find(|(_, identity, _, _, _)| identity.0 == request.token_id)
    else {
        warn!(target: "camera", "focus_token: no token {}", request.token_id);
        record(&mut last, &request.token_id, Outcome::Unknown);
        return;
    };

    if !game_master {
        if *visibility == Visibility::Hidden {
            record(&mut last, &request.token_id, Outcome::Unseen);
            return;
        }
        if name.is_some_and(|name| name.hidden_from_players) {
            record(&mut last, &request.token_id, Outcome::Unnamed);
            return;
        }
    }

    let centre = transform.translation.truncate();

    match request.surround_cells {
        Some(surround) => {
            let cell_size = grid.as_ref().map_or(32.0, |g| g.size);
            let footprint = behaviour
                .copied()
                .unwrap_or_default()
                .footprint
                .cells()
                .max(1.0);
            let extent = framed_extent(footprint, surround, cell_size);
            // World units at 1:1 are pixels, so the viewport's logical size is
            // its size in world units — the same reasoning `fit_camera_to`
            // uses. The fallback matters only headlessly, where there is no
            // camera to ask.
            let size = viewport
                .single()
                .ok()
                .and_then(|camera| camera.logical_viewport_size())
                .unwrap_or(Vec2::new(1280.0, 720.0));
            camera_mgr.fit_to(centre, Vec2::splat(extent), size);
        }
        None => {
            // Only the target: the current zoom is the viewer's own choice and
            // a plain "look at this" has no business overruling it.
            camera_mgr.target_translation = centre;
        }
    }

    if request.immediate {
        camera_mgr.snap_to_target();
    }

    record(&mut last, &request.token_id, Outcome::Moved);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::camera::CameraPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::input::InputPlugin);
        app.init_resource::<crate::resources::SelectedLight>();
        app.add_plugins(CameraPlugin);
        app.insert_resource(SceneGrid::from_server("square", 32.0, Vec2::ZERO));
        app
    }

    /// A token at `at`, drawn or not, named readably or not.
    fn spawn_token(app: &mut App, id: &str, at: Vec2, drawn: bool, name_hidden: bool) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_xyz(at.x, at.y, 0.0),
                TokenIdentity(id.to_string()),
                if drawn {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
                TokenName {
                    text: Some(id.to_string()),
                    hidden_from_players: name_hidden,
                },
            ))
            .id()
    }

    fn focus(app: &mut App, id: &str, surround: Option<f32>, immediate: bool) {
        request_focus(FocusRequest {
            token_id: id.to_string(),
            surround_cells: surround,
            immediate,
        });
        app.update();
    }

    fn outcome(app: &App) -> Outcome {
        app.world().resource::<LastFocus>().outcome
    }

    #[test]
    fn looking_at_a_token_aims_the_camera_at_it_and_glides() {
        let mut app = app();
        spawn_token(&mut app, "boblin", Vec2::new(400.0, -250.0), true, false);

        focus(&mut app, "boblin", None, false);

        let camera = app.world().resource::<CameraManager>();
        assert_eq!(camera.target_translation, Vec2::new(400.0, -250.0));
        assert_ne!(
            camera.translation,
            Vec2::new(400.0, -250.0),
            "a look is a considered motion, not a jump",
        );
        assert_eq!(outcome(&app), Outcome::Moved);
    }

    #[test]
    fn reduced_motion_lands_the_camera_in_one_frame() {
        let mut app = app();
        spawn_token(&mut app, "boblin", Vec2::new(400.0, -250.0), true, false);

        focus(&mut app, "boblin", None, true);

        let camera = app.world().resource::<CameraManager>();
        assert_eq!(camera.translation, Vec2::new(400.0, -250.0));
    }

    /// The zoom-out that follows a turn has to show the creature *and* what is
    /// around it — a Large one included, which is two cells a side.
    #[test]
    fn framing_a_large_creature_leaves_room_for_its_surroundings() {
        let mut app = app();
        let token = spawn_token(&mut app, "ogre", Vec2::ZERO, true, false);
        app.world_mut()
            .entity_mut(token)
            .insert(TokenGridBehaviour {
                footprint: thunderforge_canvas_core::grid::Footprint::new(2.0),
                snap: true,
            });

        focus(&mut app, "ogre", Some(6.0), true);

        let cell = app.world().resource::<SceneGrid>().size;
        let camera = app.world().resource::<CameraManager>();
        // 2 cells of ogre plus 6 either side is 14 cells across; at the
        // headless viewport's 720 short side that is the scale needed to fit.
        let framed = framed_extent(2.0, 6.0, cell);
        assert!(
            camera.scale >= framed / 720.0 - 1e-3,
            "the frame has to hold {framed} world units, got scale {}",
            camera.scale,
        );
        // And the ogre's own body is a fraction of it, not most of it.
        assert!(
            2.0 * cell < framed * 0.25,
            "a Large creature should not fill the frame",
        );
    }

    #[test]
    fn a_player_cannot_look_at_a_token_their_board_does_not_draw() {
        let mut app = app();
        app.insert_resource(IsGameMaster(false));
        spawn_token(&mut app, "lurker", Vec2::new(900.0, 0.0), false, false);

        focus(&mut app, "lurker", None, true);

        assert_eq!(outcome(&app), Outcome::Unseen);
        assert_eq!(
            app.world().resource::<CameraManager>().translation,
            Vec2::ZERO
        );
    }

    #[test]
    fn a_player_cannot_look_at_a_creature_whose_name_they_cannot_read() {
        let mut app = app();
        app.insert_resource(IsGameMaster(false));
        spawn_token(&mut app, "hidden-npc", Vec2::new(900.0, 0.0), true, true);

        focus(&mut app, "hidden-npc", None, true);

        assert_eq!(outcome(&app), Outcome::Unnamed);
        assert_eq!(
            app.world().resource::<CameraManager>().translation,
            Vec2::ZERO
        );
    }

    /// The same two tokens, from the other chair.
    #[test]
    fn a_game_master_may_look_at_anything() {
        let mut app = app();
        app.insert_resource(IsGameMaster(true));
        spawn_token(&mut app, "hidden-npc", Vec2::new(900.0, 0.0), false, true);

        focus(&mut app, "hidden-npc", None, true);

        assert_eq!(outcome(&app), Outcome::Moved);
        assert_eq!(
            app.world().resource::<CameraManager>().translation,
            Vec2::new(900.0, 0.0),
        );
    }

    #[test]
    fn looking_at_a_token_that_has_not_arrived_moves_nothing() {
        let mut app = app();

        focus(&mut app, "not-here", None, true);

        assert_eq!(outcome(&app), Outcome::Unknown);
        assert_eq!(
            app.world().resource::<CameraManager>().translation,
            Vec2::ZERO
        );
    }

    /// The hand on the mouse wins.
    ///
    /// Driven through a real middle-button drag rather than by setting
    /// `PanDragActive` directly: the resource is written by `handle_drag_pan`,
    /// which runs first in the same chain, so a test that set it by hand would
    /// have it cleared before the focus system ever read it — and would pass
    /// for the wrong reason if the ordering were ever reversed.
    #[test]
    fn a_look_does_not_fight_a_drag_in_flight() {
        use bevy::input::ButtonState;
        use bevy::input::mouse::MouseButtonInput;
        use bevy::window::PrimaryWindow;

        let mut app = app();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        spawn_token(&mut app, "boblin", Vec2::new(400.0, 0.0), true, false);
        app.update();

        let cursor_to = |app: &mut App, to: Vec2| {
            app.world_mut()
                .get_mut::<Window>(window)
                .expect("window")
                .set_cursor_position(Some(to));
        };
        cursor_to(&mut app, Vec2::new(400.0, 300.0));
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Middle,
            state: ButtonState::Pressed,
            window,
        });
        app.update();
        // Past the drag threshold, so the drag is live.
        cursor_to(&mut app, Vec2::new(600.0, 300.0));
        app.update();

        let held = app.world().resource::<CameraManager>().target_translation;
        assert!(
            app.world().resource::<PanDragActive>().0,
            "the drag should be live for this test to mean anything",
        );

        focus(&mut app, "boblin", None, true);

        assert_eq!(
            app.world().resource::<CameraManager>().target_translation,
            held,
            "a drag in progress keeps the camera",
        );
        assert_eq!(outcome(&app), Outcome::None, "and the look is dropped");
    }

    /// Framing counts the surroundings around the creature, not through it.
    #[test]
    fn surroundings_are_counted_beyond_the_creatures_own_body() {
        assert_eq!(framed_extent(1.0, 6.0, 32.0), 13.0 * 32.0);
        assert_eq!(framed_extent(2.0, 6.0, 32.0), 14.0 * 32.0);
    }
}
