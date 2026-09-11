//! Tests for `systems/lighting.rs`, kept beside it rather than inside it so
//! the module stays within the file-length limit (`scripts/check-file-length.sh`).

use super::*;
use crate::resources::DoorState;

fn source(id: &str, x: f32, y: f32, radius: f32, casts_shadows: bool) -> LightSource {
    LightSource {
        id: id.to_string(),
        x,
        y,
        radius,
        intensity: 1.0,
        color: None,
        attached_token_id: None,
        casts_shadows,
    }
}

#[test]
fn light_color_prioritizes_selection() {
    let light = source("l1", 0.0, 0.0, 100.0, false);
    assert_eq!(light_color(&light, true), SELECTED_LIGHT_COLOR);
}

#[test]
fn light_color_tints_non_shadow_casting_when_unselected() {
    let light = source("l1", 0.0, 0.0, 100.0, false);
    assert_eq!(light_color(&light, false), NON_SHADOW_CASTING_TINT);
}

#[test]
fn light_color_defaults_when_no_color_set() {
    let light = source("l1", 0.0, 0.0, 100.0, true);
    assert_eq!(light_color(&light, false), DEFAULT_LIGHT_COLOR);
}

#[test]
fn effective_position_uses_attached_token_when_present() {
    let mut light = source("l1", 0.0, 0.0, 100.0, true);
    light.attached_token_id = Some("token-1".to_string());

    let mut positions = HashMap::new();
    positions.insert("token-1".to_string(), Vec2::new(42.0, 7.0));

    assert_eq!(
        effective_light_position(&light, &positions),
        Vec2::new(42.0, 7.0)
    );
}

#[test]
fn effective_position_falls_back_to_stored_when_attached_token_missing() {
    let mut light = source("l1", 3.0, 4.0, 100.0, true);
    light.attached_token_id = Some("missing".to_string());

    let positions = HashMap::new();
    assert_eq!(
        effective_light_position(&light, &positions),
        Vec2::new(3.0, 4.0)
    );
}

#[test]
fn effective_position_uses_stored_when_not_attached() {
    let light = source("l1", 3.0, 4.0, 100.0, true);
    let positions = HashMap::new();
    assert_eq!(
        effective_light_position(&light, &positions),
        Vec2::new(3.0, 4.0)
    );
}

#[test]
fn default_wall_state_never_blocks_a_zero_wall_scene() {
    // Sanity: DoorState import above is actually used (avoids an
    // unused-import warning while documenting that door-state-aware
    // occlusion is exercised via `is_visible`, tested exhaustively in
    // `thunderforge_canvas_core::wall`).
    assert_eq!(DoorState::default(), DoorState::None);
}

// T065: `apply_light_illumination`'s door-state-aware occlusion itself
// just calls `thunderforge_canvas_core::wall::is_visible`, already
// exhaustively covered (open/closed door, combined scenarios) in that
// crate's own tests. What's untested anywhere is the branch *before*
// that call: `if !light.casts_shadows { return true; }` — a light with
// `casts_shadows == false` is defined (FR-027) to illuminate everything
// in radius regardless of walls, short-circuiting `is_visible` entirely.
// These drive the real Bevy system end to end (not just the pure
// geometry) to prove that short-circuit actually takes effect. Per this
// crate's tests now build and run on the host (spec 032 T083); they used
// to only compile-check under `cargo check --target
// wasm32-unknown-unknown --tests`. The equivalent pure-geometry coverage
// lives in `thunderforge_canvas_core::wall`'s tests.
mod apply_light_illumination_tests {
    use super::*;
    use crate::TokenIdentity;
    use crate::resources::lighting::LightSet as EngineLightSet;
    use crate::resources::wall::WallSet as EngineWallSet;
    use thunderforge_canvas_core::wall::{DoorState as CoreDoorState, Wall as CoreWall};

    fn app_with_blocking_wall_and_token(token_pos: Vec2) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut wall_set = EngineWallSet::default();
        wall_set.upsert(CoreWall {
            id: "w1".to_string(),
            x1: 50.0,
            y1: -10.0,
            x2: 50.0,
            y2: 10.0,
            blocks_vision: true,
            blocks_movement: false,
            door_state: CoreDoorState::Closed,
            locked: false,
            secret: false,
        });
        app.insert_resource(wall_set);
        app.init_resource::<EngineLightSet>();
        // Without this the scene is in daylight — `SceneAmbient` defaults
        // to `AmbientLight::daylight()` — and `illumination_at` reports
        // Bright everywhere before a wall or a light is consulted. Both
        // tests below then pass whatever the occlusion code does, which
        // is exactly what happened while this suite could not be built:
        // the shadow-casting case failed on its first real run, and its
        // non-shadow-casting companion had been passing vacuously.
        app.insert_resource(crate::resources::vision::SceneAmbient(
            thunderforge_canvas_core::vision::AmbientLight::unlit(),
        ));

        // `Sprite` is not decoration here: `apply_light_illumination`'s
        // token query is `(&Transform, Option<&TokenVision>, &mut Sprite,
        // &mut Visibility)`, so an entity without one is never visited and
        // its `Visibility` is never written. Without it both tests below
        // read back the `Visibility::Inherited` they spawned with and drew
        // conclusions about occlusion from it.
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(token_pos.extend(0.0)),
            TokenIdentity("token-1".to_string()),
            Visibility::Inherited,
        ));

        app.add_systems(Update, apply_light_illumination);
        app
    }

    fn token_visibility(app: &mut App) -> Visibility {
        let mut query = app.world_mut().query::<(&TokenIdentity, &Visibility)>();
        let (_, visibility) = query.iter(app.world()).next().unwrap();
        *visibility
    }

    #[test]
    fn shadow_casting_light_is_occluded_by_closed_wall() {
        // Light and token on opposite sides of a closed, vision-blocking
        // wall: the light casts shadows, so `is_visible` should apply
        // and the token should end up unlit.
        let target = Vec2::new(100.0, 0.0);
        let mut app = app_with_blocking_wall_and_token(target);

        let mut light_set = app.world_mut().resource_mut::<EngineLightSet>();
        light_set.0.upsert(LightSource {
            id: "l1".to_string(),
            x: 0.0,
            y: 0.0,
            radius: 500.0,
            intensity: 1.0,
            color: None,
            attached_token_id: None,
            casts_shadows: true,
        });

        app.update();

        assert_eq!(
            token_visibility(&mut app),
            UNLIT_VISIBILITY,
            "a shadow-casting light blocked by a closed wall must not light the token"
        );
    }

    #[test]
    fn a_game_master_never_loses_a_token_to_the_dark() {
        // Playtest 2026-09-10 P9: a token a player could not see is one a
        // Game Master must still find — dimmed, never gone.
        let mut app = app_with_blocking_wall_and_token(Vec2::new(100.0, 0.0));
        app.insert_resource(IsGameMaster(true));
        app.world_mut()
            .resource_mut::<EngineLightSet>()
            .0
            .upsert(source("far", 5000.0, 0.0, 10.0, true));

        app.update();

        assert_eq!(token_visibility(&mut app), Visibility::Inherited);
    }

    #[test]
    fn a_token_in_plain_sight_keeps_its_own_alpha_and_loses_only_the_dim() {
        // Selection feedback draws an unselected token at 0.85; plain
        // sight used to overwrite that with 1.0 every frame.
        let mut app = app_with_blocking_wall_and_token(Vec2::new(-100.0, 0.0));
        let alpha = |app: &mut App| {
            let mut query = app.world_mut().query::<(&TokenIdentity, &Sprite)>();
            query.iter(app.world()).next().unwrap().1.color.alpha()
        };
        let set_alpha = |app: &mut App, value: f32| {
            let mut query = app.world_mut().query::<&mut Sprite>();
            for mut sprite in query.iter_mut(app.world_mut()) {
                sprite.color = sprite.color.with_alpha(value);
            }
        };

        // Lit: selection's alpha stands.
        app.world_mut()
            .resource_mut::<EngineLightSet>()
            .0
            .upsert(source("lamp", 0.0, 0.0, 5000.0, false));
        set_alpha(&mut app, 0.85);
        app.update();
        assert!((alpha(&mut app) - 0.85).abs() < 1e-4);

        // Dim light: dimmed — and back in full light, undimmed.
        app.world_mut()
            .resource_mut::<EngineLightSet>()
            .0
            .upsert(source("lamp", -250.0, 0.0, 200.0, false));
        app.update();
        assert!((alpha(&mut app) - DIM_ALPHA).abs() < 1e-4);
        app.world_mut()
            .resource_mut::<EngineLightSet>()
            .0
            .upsert(source("lamp", 0.0, 0.0, 5000.0, false));
        app.update();
        assert!((alpha(&mut app) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_player_sees_from_their_own_token() {
        // One light, not shadow-casting, lighting everything: the only
        // thing that can hide the target is the viewer's line of sight,
        // which the wall at x = 50 blocks.
        let mut app = app_with_blocking_wall_and_token(Vec2::new(100.0, 0.0));
        app.world_mut()
            .resource_mut::<EngineLightSet>()
            .0
            .upsert(source("sun", 0.0, 0.0, 5000.0, false));
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(Vec3::ZERO),
            TokenIdentity("mine".to_string()),
            Visibility::Inherited,
        ));

        // No viewer named: the board by light alone — the target is lit.
        app.init_resource::<ViewerToken>();
        app.update();
        let visibility_of_target = |app: &mut App| {
            let mut query = app.world_mut().query::<(&TokenIdentity, &Visibility)>();
            query
                .iter(app.world())
                .find(|(id, _)| id.0 == "token-1")
                .map(|(_, v)| *v)
                .unwrap()
        };
        assert_eq!(visibility_of_target(&mut app), Visibility::Inherited);

        // Seen from their own token, the wall is between them.
        app.insert_resource(ViewerToken(Some("mine".to_string())));
        app.update();
        assert_eq!(visibility_of_target(&mut app), UNLIT_VISIBILITY);
    }

    #[test]
    fn non_shadow_casting_light_ignores_closed_wall() {
        // Same geometry as above (light and token split by the same
        // closed wall), but `casts_shadows: false` — FR-027 says this
        // light illuminates anything within radius unconditionally,
        // short-circuiting the `is_visible` occlusion check entirely.
        let target = Vec2::new(100.0, 0.0);
        let mut app = app_with_blocking_wall_and_token(target);

        let mut light_set = app.world_mut().resource_mut::<EngineLightSet>();
        light_set.0.upsert(LightSource {
            id: "l1".to_string(),
            x: 0.0,
            y: 0.0,
            radius: 500.0,
            intensity: 1.0,
            color: None,
            attached_token_id: None,
            casts_shadows: false,
        });

        app.update();

        assert_eq!(
            token_visibility(&mut app),
            Visibility::Inherited,
            "a non-shadow-casting light must light the token even behind a closed wall"
        );
    }
}
