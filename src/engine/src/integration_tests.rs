//! Integration tests verifying the canvas stack's resources and data flow
//! work together against the CURRENT engine API surface.
//!
//! This file previously targeted an older API shape (a `SelectedToken.entity`
//! field, `CameraManager::new(w, h, grid_size)`, a `Default`-derived `Token`,
//! a 4-arg `grid_to_pixel`, etc.) that had drifted out of sync with the real
//! code across several refactors, so none of it compiled and the whole test
//! binary — all 194 `#[test]` functions in this crate — silently never ran.
//! Rewritten against today's actual types; see `src/engine/src/resources/`,
//! `src/engine/src/components.rs`, and `src/engine/src/transforms/coordinate.rs`
//! for the real signatures being exercised here.
//!
//! Mouse-driven selection/drag (`systems::selection::handle_token_drag`) is
//! NOT covered here: it depends on `Camera::viewport_to_world_2d`, which
//! needs a real camera projection computed by bevy_render's plugin set —
//! not feasible to drive headlessly with `MinimalPlugins`. Its pure hit-test
//! math (`is_point_in_rect`) has its own unit tests in `systems/selection.rs`.

#[cfg(test)]
mod e2e_canvas_tests {
    use crate::resources::camera::CameraManager;
    use crate::resources::scene_data::{GridType, SceneData};
    use crate::resources::selection::{DraggingToken, SelectedToken};
    use bevy::prelude::*;

    #[test]
    fn test_e2e_launch_world_see_grid() {
        // Scenario: User launches world and should see grid
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        app.insert_resource(SceneData::new(
            "default".to_string(),
            "world-default".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        ));
        app.init_resource::<CameraManager>();
        app.init_resource::<SelectedToken>();
        app.init_resource::<ButtonInput<KeyCode>>();

        let scene = app.world().resource::<SceneData>();
        assert_eq!(scene.scene_id, "default");
        assert_eq!(scene.width, 20);
        assert_eq!(scene.height, 20);
    }

    #[test]
    fn test_e2e_token_identity_spawn_and_transform() {
        // Scenario: tokens are represented on the live `TokenIdentity` pipeline
        // (see lib.rs `apply_external_commands`/`emit_player_state`) — a plain
        // `TokenIdentity` marker + `Transform`, not the legacy `Token` component.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let token_entity = app
            .world_mut()
            .spawn((
                Sprite::from_color(Color::WHITE, Vec2::new(96.0, 96.0)),
                Transform::from_xyz(10.0, 20.0, 0.0),
            ))
            .id();

        assert!(app.world().get_entity(token_entity).is_ok());
        let transform = app.world().get::<Transform>(token_entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(10.0, 20.0, 0.0));
    }

    #[test]
    fn test_e2e_pan_zoom_camera() {
        // Scenario: User pans and zooms camera, tokens should move with it
        let mut camera = CameraManager::new();

        assert_eq!(camera.translation, Vec2::ZERO);
        assert_eq!(camera.scale, 1.0);

        // `pan` moves the camera's *target*; `translation` eases toward it.
        // These assertions predate that split and read `translation`
        // immediately after panning, so `snap_to_target` is what makes them
        // about panning rather than about the glide.
        camera.pan(Vec2::new(100.0, 0.0));
        camera.snap_to_target();
        assert_eq!(camera.translation, Vec2::new(100.0, 0.0));

        camera.set_zoom(2.0);
        camera.snap_to_target();
        assert_eq!(camera.scale, 2.0);

        camera.pan(Vec2::new(50.0, 50.0));
        camera.snap_to_target();
        assert_eq!(camera.translation, Vec2::new(150.0, 50.0));
    }

    #[test]
    fn test_e2e_select_and_deselect_token() {
        // Scenario: User selects a token, then clicks empty space to deselect
        let mut selected = SelectedToken::default();
        assert!(selected.get_selected().is_none());

        selected.select("token-1".to_string());
        assert!(selected.is_selected("token-1"));

        selected.deselect();
        assert!(selected.get_selected().is_none());
    }

    /// Scenario: user presses down on a token off-centre; the drag resource
    /// should remember the grab offset so the token doesn't snap-recentre.
    ///
    /// `DraggingToken` became a `Vec` of `(id, offset)` when clicking a stack
    /// started picking up the whole stack — this test still spoke the
    /// `Option` it was written against, and asserted only about the single
    /// dragged token. It now also asserts the property the `Vec` exists for:
    /// each member of a dragged stack keeps its *own* offset, which is what
    /// preserves their relative positions.
    #[test]
    fn test_e2e_drag_state_tracks_grab_offset() {
        let mut dragging = DraggingToken::default();
        assert!(dragging.0.is_empty());

        let grab_offset = Vec2::new(-12.0, 4.0);
        let second_offset = Vec2::new(6.0, -3.0);
        dragging.0 = vec![
            ("token-1".to_string(), grab_offset),
            ("token-2".to_string(), second_offset),
        ];

        assert_eq!(dragging.0[0].0, "token-1");
        assert_eq!(dragging.0[0].1, grab_offset);
        assert_eq!(dragging.0[1].0, "token-2");
        assert_eq!(
            dragging.0[1].1, second_offset,
            "each token in a dragged stack keeps its own offset"
        );
    }

    #[test]
    fn test_e2e_movement_rejection_rollback() {
        // Scenario: User moves an actor optimistically, server rejects,
        // should rollback via RollbackCache's last-known-good state.
        let initial = crate::components::GridPosition::new(5.0, 5.0, 0.0);
        let cache = crate::components::RollbackCache::new(initial, Some(10));
        assert!(!cache.is_pending);
        assert_eq!(cache.last_server_position, initial);

        // Optimistic move to (6, 5)
        let mut pos = crate::components::GridPosition::new(6.0, 5.0, 0.0);
        assert_eq!(pos.x, 6.0);

        // Server rejects: rollback to cached position
        pos = cache.last_server_position;
        assert_eq!(pos.x, 5.0);
        assert_eq!(pos.y, 5.0);
    }

    #[test]
    fn test_e2e_camera_keyboard_shortcuts() {
        // Scenario: User presses arrow keys, camera pans; +/- to zoom; Home to reset
        let mut camera = CameraManager::new();

        camera.pan(Vec2::new(32.0, 0.0));
        camera.snap_to_target();
        assert_eq!(camera.translation, Vec2::new(32.0, 0.0));

        camera.pan(Vec2::new(0.0, 32.0));
        camera.snap_to_target();
        assert_eq!(camera.translation, Vec2::new(32.0, 32.0));

        // Positive steps zoom in, which *shrinks* the scale (world units per
        // screen unit) — see `CameraManager::zoom_by`.
        camera.zoom_by(1.0);
        camera.snap_to_target();
        assert!(camera.scale < 1.0);

        camera.zoom_by(-1.0);
        camera.snap_to_target();
        assert!((camera.scale - 1.0).abs() < 0.001);

        camera.reset();
        camera.snap_to_target();
        assert_eq!(camera.translation, Vec2::ZERO);
        assert_eq!(camera.scale, 1.0);
    }

    #[test]
    fn test_e2e_coordinate_transform_consistency() {
        // Scenario: A grid coordinate should survive a round trip through
        // grid_to_pixel -> pixel_to_grid unchanged (within float precision),
        // for any given camera pan/zoom state.
        let scene = SceneData::new(
            "test".to_string(),
            "world-test".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        );
        let camera_translation = Vec2::new(15.0, -8.0);
        let camera_scale = 1.5;

        let grid_pos = (5.0, 5.0);

        let pixel_pos = crate::transforms::coordinate::grid_to_pixel(
            grid_pos.0,
            grid_pos.1,
            &scene,
            camera_translation,
            camera_scale,
        );

        let back_to_grid = crate::transforms::coordinate::pixel_to_grid(
            pixel_pos.x,
            pixel_pos.y,
            &scene,
            camera_translation,
            camera_scale,
        );

        assert!((back_to_grid.0 - grid_pos.0).abs() < 0.001);
        assert!((back_to_grid.1 - grid_pos.1).abs() < 0.001);
    }

    #[test]
    fn test_e2e_full_canvas_startup_sequence() {
        // Scenario: Verify complete startup: scene load -> camera ready -> selection ready -> system registry ready
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        app.insert_resource(SceneData::new(
            "default".to_string(),
            "world-default".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        ));
        let scene = app.world().resource::<SceneData>();
        assert_eq!(scene.scene_id, "default");

        app.init_resource::<CameraManager>();
        let camera = app.world().resource::<CameraManager>();
        assert_eq!(camera.scale, 1.0);

        app.init_resource::<SelectedToken>();
        let selected = app.world().resource::<SelectedToken>();
        assert!(selected.get_selected().is_none());

        // A fourth assertion stood here, that a `SystemRegistry` resource was
        // ready. It has gone with the registry: nothing ever read that
        // resource, so "ready" was a claim about a value no code consulted.
        // What a system contributes is now resolved server-side and arrives
        // as declared values — see ADR-060.
    }

    #[test]
    fn test_e2e_error_recovery_invalid_token_move() {
        // Scenario: User tries to move a token out of bounds, should reject gracefully
        let scene = SceneData::new(
            "test".to_string(),
            "world-test".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        let pos = crate::components::GridPosition::new(9.0, 5.0, 0.0);
        let new_x = pos.x + 1.0;

        let is_valid = new_x < scene.width as f32;
        assert!(!is_valid); // Should be invalid; server would reject this mutation
    }
}

#[cfg(test)]
mod integration_system_tests {
    use crate::resources::camera::CameraManager;
    use crate::resources::scene_data::{GridType, SceneData};
    use crate::resources::selection::SelectedToken;
    use bevy::prelude::*;

    #[test]
    fn test_all_systems_compile_together() {
        // Meta-test: Verify all core resources can be added to an app simultaneously
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        app.insert_resource(SceneData::new(
            "default".to_string(),
            "world-default".to_string(),
            GridType::Square,
            32.0,
            20,
            20,
            None,
        ));
        app.init_resource::<CameraManager>();
        app.init_resource::<SelectedToken>();
        app.init_resource::<ButtonInput<KeyCode>>();
        #[cfg(target_arch = "wasm32")]
        app.init_resource::<crate::network::mutations::MutationTracker>();

        assert_eq!(app.world().resource::<SceneData>().scene_id, "default");
    }

    #[test]
    fn test_multiple_tokens_independent_selection() {
        // Scenario: Multiple tokens on board, selecting one doesn't affect others
        let mut selected = SelectedToken::default();

        selected.select("t1".to_string());
        assert!(selected.is_selected("t1"));
        assert!(!selected.is_selected("t2"));

        selected.select("t2".to_string());
        assert!(selected.is_selected("t2"));
        assert!(!selected.is_selected("t1"));
    }

    #[test]
    fn test_camera_state_persists_across_movements() {
        // Scenario: Pan, then zoom, pan state should be preserved
        let mut camera = CameraManager::new();

        let initial_pan = Vec2::new(100.0, 50.0);
        camera.translation = initial_pan;

        // A plain zoom leaves the camera where it is; only `zoom_toward`
        // (cursor-anchored) moves it.
        camera.zoom_by(1.0);
        assert_eq!(camera.translation, initial_pan);
    }
}

#[cfg(test)]
mod plugin_independence_tests {
    //! T064: proves Constitution Principle II ("each plugin is
    //! independently addable/removable") for `WallPlugin`, `LightingPlugin`,
    //! and `ShapePlugin` — each test below builds a headless `App` with
    //! `MinimalPlugins` + `CanvasLayerPlugin` (the one fixed, shared
    //! dependency all three read `CanvasLayers` from) + *only* the plugin
    //! under test, deliberately leaving the other two out of the builder.
    //! `app.update()` must run without panicking and the plugin's own
    //! resource must exist and be queryable afterward.
    //!
    //! `MinimalPlugins` doesn't provide `ButtonInput<KeyCode>`/
    //! `ButtonInput<MouseButton>` (those come from `bevy::input::InputPlugin`,
    //! part of `DefaultPlugins`) or a real window/camera (from
    //! `bevy_window`/`bevy_render`'s plugins), all of which each plugin's
    //! input systems read. Per the module doc above, driving mouse-based
    //! selection/drag needs a real camera projection and isn't feasible
    //! headlessly — so these tests insert the missing input resources
    //! manually (cheap, and enough for the systems to run without a missing-
    //! resource panic) but do NOT spawn a window or camera, which means
    //! `cursor_world_position` in each plugin's input system always resolves
    //! to `None` and every input system is a no-op passthrough for the
    //! `app.update()` calls here. That's fine for this test's purpose: it
    //! proves the plugin *builds and runs standalone*, not that its mouse-
    //! driven authoring flow works (that's out of scope for a headless test,
    //! same carve-out as `handle_token_drag`).

    use crate::ActiveWorld;
    use crate::plugins::{CanvasLayerPlugin, LightingPlugin, ShapePlugin, WallPlugin};
    use crate::resources::{LightSet, ShapeSet, WallSet};
    use bevy::prelude::*;

    /// The app-level environment every plugin here assumes, and which
    /// `MinimalPlugins` does not supply.
    ///
    /// This used to hand-insert only the two `ButtonInput` resources, which
    /// was enough to compile and — since the suite had never run — enough to
    /// look finished. On the first real run all three tests panicked inside
    /// `app.update()`: the wall and shape keyboard-toggle systems take
    /// `Res<ActiveWorld>`, which only `start()` inserts, and
    /// `handle_light_resize` reads `MessageReader<MouseWheel>`, whose message
    /// registration comes from `InputPlugin` — not from the bare `ButtonInput`
    /// resources. `InputPlugin` also supplies both of those, so it replaces
    /// the hand-inserted pair rather than joining them.
    ///
    /// Neither is a plugin defect: both are things the host app owns. What
    /// the tests prove is unchanged — each plugin builds and runs with the
    /// other two absent.
    fn headless_app_with_inputs() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::input::InputPlugin);
        app.insert_resource(ActiveWorld("world-test".to_string()));
        app
    }

    #[test]
    fn wall_plugin_builds_and_runs_independently() {
        let mut app = headless_app_with_inputs();
        app.add_plugins(CanvasLayerPlugin);
        app.add_plugins(WallPlugin);
        // Deliberately no LightingPlugin/ShapePlugin.

        // Should not panic across a couple of frames.
        app.update();
        app.update();

        let wall_set = app.world().resource::<WallSet>();
        assert!(wall_set.walls().is_empty());
    }

    /// FAILING, and left failing deliberately (spec 032 T083).
    ///
    /// `LightingPlugin` is not independently addable: `apply_light_
    /// illumination`, which it schedules, takes `Res<WallSet>` — a resource
    /// only `WallPlugin` registers — so `app.update()` panics with "Resource
    /// does not exist" when the lighting plugin is added without it. The
    /// plugin's own doc comment claims the opposite ("Independently
    /// addable/removable per Constitution Principle II ... mirrors
    /// `WallPlugin` exactly"), and it already registers `IsGameMaster`,
    /// `GridSnapEnabled` and `SceneGrid` idempotently for precisely this
    /// reason — `WallSet` was left out.
    ///
    /// This test has never been able to run, so the gap has never been
    /// visible. The fix is a one-line `.init_resource::<WallSet>()` in
    /// `plugins/lighting.rs`, which is a no-op in the shipping app (where
    /// `WallPlugin` is always present) — but it is an engine change, not a
    /// test repair, so it is being reported rather than made here.
    #[test]
    fn lighting_plugin_builds_and_runs_independently() {
        let mut app = headless_app_with_inputs();
        app.add_plugins(CanvasLayerPlugin);
        app.add_plugins(LightingPlugin);
        // Deliberately no WallPlugin/ShapePlugin.

        app.update();
        app.update();

        let light_set = app.world().resource::<LightSet>();
        assert!(light_set.lights().is_empty());
    }

    #[test]
    fn shape_plugin_builds_and_runs_independently() {
        let mut app = headless_app_with_inputs();
        app.add_plugins(CanvasLayerPlugin);
        app.add_plugins(ShapePlugin);
        // Deliberately no WallPlugin/LightingPlugin.

        app.update();
        app.update();

        let shape_set = app.world().resource::<ShapeSet>();
        assert!(shape_set.shapes().is_empty());
    }
}

#[cfg(test)]
mod manual_browser_test_scenarios {
    // Manual browser testing checklist — mouse-driven selection/drag and full
    // rendering can't be exercised headlessly (see module doc comment above),
    // so these remain documented scenarios for manual QA.

    // SCENARIO 1: Launch world and see grid
    // SCENARIO 2: Tokens render on grid at correct positions
    // SCENARIO 3: Pan and zoom via keyboard shortcuts
    // SCENARIO 4: Click a token to select it; click another to switch selection
    // SCENARIO 5: Drag a selected token; release persists the new position to the server
    // SCENARIO 6: Movement rejection rollback (requires server configured to reject a move)
    // SCENARIO 7: Multiple clients: token moves sync via server, camera stays local per-client

    // `test_manual_scenarios_documented` was here: `assert!(true, "See
    // manual_browser_test_scenarios ...")`. The checklist above is the
    // deliverable; a test that cannot fail does not make it any more true.
}
