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
        bright_radius: None,
    }
}

// # Placing a light attaches it to the cursor (owner decision 2026-09-15)
//
// The preview itself is gizmos, which leave nothing to assert on. What can be
// asserted — and what actually decides whether the preview tells the truth —
// is the one question it shares with the click it previews: would a click
// here place a light, or grab one?

#[test]
fn a_click_on_empty_floor_would_place_a_light() {
    let lamp = source("l1", 500.0, 500.0, 100.0, true);
    assert!(click_would_place_a_light(Vec2::ZERO, &[lamp]));
}

#[test]
fn a_click_on_an_existing_light_would_grab_it_instead() {
    let lamp = source("l1", 0.0, 0.0, 100.0, true);
    // Inside the grab radius: this click selects, so nothing is previewed.
    assert!(!click_would_place_a_light(Vec2::new(5.0, 0.0), &[lamp]));
}

/// A carried light belongs to its character's sheet and cannot be grabbed
/// (spec 045 T065), so standing on one does not stop a placement.
#[test]
fn a_carried_light_does_not_block_placing_one() {
    let torch = LightSource::carried("token-1", 20.0, 40.0).expect("carried light");
    assert!(click_would_place_a_light(torch.position(), &[torch]));
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
fn a_carried_light_with_no_token_yet_is_nowhere() {
    let torch = LightSource::carried("brom", 20.0, 40.0).expect("a torch");
    assert_eq!(live_light_position(&torch, &HashMap::new()), None);

    let positions = HashMap::from([("brom".to_string(), Vec2::new(9.0, 2.0))]);
    assert_eq!(
        live_light_position(&torch, &positions),
        Some(Vec2::new(9.0, 2.0))
    );
}

#[test]
fn a_carried_light_resolves_to_its_declared_bright_reach() {
    let torch = LightSource::carried("brom", 30.0, 40.0).expect("a torch");
    let positions = HashMap::from([("brom".to_string(), Vec2::ZERO)]);
    let resolved = resolve_light(&torch, &positions).expect("placed");
    assert_eq!((resolved.bright_radius, resolved.dim_radius), (30.0, 40.0));
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
            bright_radius: None,
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
            bright_radius: None,
        });

        app.update();

        assert_eq!(
            token_visibility(&mut app),
            Visibility::Inherited,
            "a non-shadow-casting light must light the token even behind a closed wall"
        );
    }
}

// ---------------------------------------------------------------------------
// FR-033: a Game Master's board marks what the table cannot see
// ---------------------------------------------------------------------------

mod marks_for_the_game_master {
    use super::*;
    use crate::TokenIdentity;
    use crate::resources::lighting::LightSet as EngineLightSet;
    use crate::resources::wall::WallSet as EngineWallSet;
    use thunderforge_canvas_core::wall::{DoorState as CoreDoorState, Wall as CoreWall};

    /// A bright scene with a vision-blocking wall at x = 50, a hero west of
    /// it, and a monster placed wherever the caller says.
    ///
    /// Bright on purpose: illumination is then `Bright` everywhere, so the old
    /// behaviour — which marked on illumination — could not mark anything
    /// here. Whatever these tests observe comes from line of sight.
    fn table(monster_at: Vec2) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let mut wall_set = EngineWallSet::default();
        wall_set.upsert(CoreWall {
            id: "w1".to_string(),
            x1: 50.0,
            y1: -500.0,
            x2: 50.0,
            y2: 500.0,
            blocks_vision: true,
            blocks_movement: true,
            door_state: CoreDoorState::None,
            locked: false,
            secret: false,
        });
        app.insert_resource(wall_set);
        app.init_resource::<EngineLightSet>();
        app.insert_resource(crate::resources::vision::SceneAmbient(
            thunderforge_canvas_core::vision::AmbientLight::daylight(),
        ));
        app.insert_resource(IsGameMaster(true));
        app.init_resource::<ViewerToken>();
        app.insert_resource(PartyEyes(vec!["hero".to_string()]));

        for (id, at) in [("hero", Vec2::new(-100.0, 0.0)), ("monster", monster_at)] {
            app.world_mut().spawn((
                Sprite::default(),
                Transform::from_translation(at.extend(0.0)),
                TokenIdentity(id.to_string()),
                Visibility::Inherited,
            ));
        }

        app.add_systems(Update, apply_light_illumination);
        app
    }

    fn marked(app: &mut App) -> Vec<String> {
        let mut query = app.world_mut().query::<(&TokenIdentity, &Sprite)>();
        let mut out: Vec<String> = query
            .iter(app.world())
            .filter(|(_, sprite)| (sprite.color.alpha() - DIM_ALPHA).abs() < 1e-4)
            .map(|(identity, _)| identity.0.clone())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_token_the_party_cannot_see_is_marked_though_it_stands_in_daylight() {
        // The case the old behaviour got wrong in the first direction: bright
        // light, so illumination says "plainly visible", but a wall stands
        // between the hero and it and nobody at the table can see it.
        let mut app = table(Vec2::new(200.0, 0.0));
        app.update();
        assert_eq!(marked(&mut app), vec!["monster".to_string()]);
    }

    #[test]
    fn a_token_the_party_can_see_is_not_marked() {
        // Same side of the wall as the hero: nothing to warn about.
        let mut app = table(Vec2::new(-200.0, 0.0));
        app.update();
        assert!(marked(&mut app).is_empty());
    }

    #[test]
    fn a_game_master_still_sees_a_token_the_party_cannot() {
        // Marked, never hidden: the Game Master runs the monster.
        let mut app = table(Vec2::new(200.0, 0.0));
        app.update();
        let mut query = app.world_mut().query::<(&TokenIdentity, &Visibility)>();
        for (identity, visibility) in query.iter(app.world()) {
            assert_eq!(
                *visibility,
                Visibility::Inherited,
                "{} must stay on a Game Master's board",
                identity.0
            );
        }
    }

    #[test]
    fn a_second_pair_of_eyes_still_marks_what_the_first_cannot_see() {
        // FR-033 is written as *at least one* player: a token any player is
        // missing gets the mark, even if another can see it plainly.
        //
        // A split party is where that reading bites, and this is that case —
        // a scout past the wall, the hero and the monster west of it. All
        // three are marked, because for each of them there is somebody who
        // cannot see it. That is the requirement as written and it is what
        // the engine does; whether a Game Master wants this much marking
        // when their party splits up is a question for play, recorded in
        // the spec rather than decided here.
        let mut app = table(Vec2::new(-200.0, 0.0));
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(Vec2::new(200.0, 0.0).extend(0.0)),
            TokenIdentity("scout".to_string()),
            Visibility::Inherited,
        ));
        app.insert_resource(PartyEyes(vec!["hero".to_string(), "scout".to_string()]));
        app.update();
        assert_eq!(
            marked(&mut app),
            vec![
                "hero".to_string(),
                "monster".to_string(),
                "scout".to_string()
            ],
        );
    }

    #[test]
    fn a_token_every_pair_of_eyes_can_see_is_never_marked() {
        // The other side of the same rule, and the one that keeps a quiet
        // board quiet: two heroes together, and a monster in front of them.
        let mut app = table(Vec2::new(-200.0, 0.0));
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(Vec2::new(-150.0, 0.0).extend(0.0)),
            TokenIdentity("scout".to_string()),
            Visibility::Inherited,
        ));
        app.insert_resource(PartyEyes(vec!["hero".to_string(), "scout".to_string()]));
        app.update();
        assert!(marked(&mut app).is_empty());
    }

    #[test]
    fn with_no_party_named_nothing_is_marked_by_line_of_sight() {
        // A Game Master whose players have no tokens yet. The board falls
        // back to illumination, and in daylight that marks nothing — rather
        // than marking everything on the grounds that an empty party can see
        // nothing, which is the reading that makes a fresh scene look wrong.
        let mut app = table(Vec2::new(200.0, 0.0));
        app.insert_resource(PartyEyes(Vec::new()));
        app.update();
        assert!(marked(&mut app).is_empty());
    }
}
