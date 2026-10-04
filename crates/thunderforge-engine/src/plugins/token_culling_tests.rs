//! Viewport culling, run against a real `App` with the plugins that draw a
//! token's furniture, so the claim tested is "the name and bars are gone" and
//! not merely "a flag was set".
//!
//! Every test uses a 1600 × 900 view. With no grid loaded a token is 96 wide,
//! so it and its furniture reach 48 + 192 = 240 from its centre, and along x:
//!
//! - the screen ends at 800;
//! - the promised padding (25% of 1600) ends at 1200;
//! - a token becomes present inside 800 + 480 + 240 = 1520;
//! - a token is culled outside 800 + 640 + 240 = 1680.

use super::*;
use crate::plugins::nameplate::{Nameplate, NameplatePlugin, TokenName};
use crate::plugins::status_display::{
    Appearance, ResolvedResource, StatusDisplayPlugin, StatusGeometry, TokenStatus,
};
use crate::resources::DraggedToken;
use thunderforge_canvas_core::resource_display::{Disclosed, ResourceDefinition, ResourceKind};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(TokenCullingPlugin)
        .add_plugins(StatusDisplayPlugin)
        .add_plugins(NameplatePlugin)
        .init_resource::<Appearance>()
        .init_resource::<SelectedToken>()
        .init_resource::<DraggingToken>()
        .init_resource::<ViewerToken>()
        .init_resource::<ControlledToken>();
    app
}

/// A camera showing 1600 × 900 of the world, centred on `centre`.
fn spawn_camera(app: &mut App, centre: Vec2) -> Entity {
    let mut projection = OrthographicProjection::default_2d();
    projection.area = Rect::from_center_size(Vec2::ZERO, Vec2::new(1600.0, 900.0));
    app.world_mut()
        .spawn((
            Camera2d,
            Transform::from_translation(centre.extend(0.0)),
            Projection::Orthographic(projection),
        ))
        .id()
}

fn status() -> TokenStatus {
    TokenStatus {
        resources: vec![ResolvedResource {
            definition: ResourceDefinition {
                id: "hp".to_string(),
                label: "HP".to_string(),
                kind: ResourceKind::Bar,
                order: 0,
                allow_stacking: false,
            },
            // Withheld: a track and a full grey fill, two sprites a bar.
            disclosed: Disclosed::Greyed,
        }],
    }
}

/// A named token with one bar, the way `upsert_token` leaves one.
fn spawn_token(app: &mut App, id: &str, at: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            Sprite::default(),
            Transform::from_translation(at.extend(0.0)),
            TokenIdentity(id.to_string()),
            TokenName {
                text: Some(id.to_string()),
                hidden_from_players: false,
            },
            status(),
        ))
        .id()
}

fn culled(app: &App, token: Entity) -> bool {
    app.world()
        .get::<ViewportCull>(token)
        .expect("every token carries a ViewportCull")
        .culled
}

/// How many name entities and bar entities are parented to `token`.
fn furniture(app: &mut App, token: Entity) -> (usize, usize) {
    let mut plates = app
        .world_mut()
        .query_filtered::<&ChildOf, With<Nameplate>>();
    let names = plates
        .iter(app.world())
        .filter(|parent| parent.parent() == token)
        .count();
    let mut geometry = app
        .world_mut()
        .query_filtered::<&ChildOf, With<StatusGeometry>>();
    let bars = geometry
        .iter(app.world())
        .filter(|parent| parent.parent() == token)
        .count();
    (names, bars)
}

/// A name and its shadow, and one bar's track and fill.
const FULL: (usize, usize) = (2, 2);
const NONE: (usize, usize) = (0, 0);

fn move_to(app: &mut App, entity: Entity, x: f32, y: f32) {
    let mut transform = app
        .world_mut()
        .get_mut::<Transform>(entity)
        .expect("a transform");
    transform.translation.x = x;
    transform.translation.y = y;
}

/// Two frames: one to decide and queue, one for anything queued to be seen.
fn settle(app: &mut App) {
    app.update();
    app.update();
}

#[test]
fn a_token_far_outside_the_view_exists_but_carries_no_furniture() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    let far = spawn_token(&mut app, "far", Vec2::new(5000.0, 0.0));
    let near = spawn_token(&mut app, "near", Vec2::new(100.0, 50.0));
    settle(&mut app);

    assert!(culled(&app, far));
    assert_eq!(furniture(&mut app, far), NONE);
    assert!(!culled(&app, near));
    assert_eq!(furniture(&mut app, near), FULL);

    // Still a token to everything that asks about tokens: same identity,
    // same position, same sprite, and its visibility untouched — culling is
    // not vision, and must not look like it to camera focus or hit-testing.
    let world = app.world();
    assert_eq!(
        world.get::<TokenIdentity>(far).map(|t| t.0.as_str()),
        Some("far")
    );
    assert_eq!(
        world.get::<Transform>(far).map(|t| t.translation.x),
        Some(5000.0)
    );
    assert!(world.get::<Sprite>(far).is_some());
    assert_eq!(world.get::<Visibility>(far), Some(&Visibility::Inherited));
    assert!(world.get::<TokenName>(far).is_some());
    assert!(world.get::<TokenStatus>(far).is_some());
}

#[test]
fn a_token_arriving_off_screen_never_has_furniture_built_for_it() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    app.update();
    let far = spawn_token(&mut app, "far", Vec2::new(5000.0, 0.0));
    // One frame, not two: the decision is made before the name and the bars
    // are drawn, so there is nothing to tear down afterwards.
    app.update();
    assert!(culled(&app, far));
    assert_eq!(furniture(&mut app, far), NONE);
}

#[test]
fn a_token_off_screen_but_inside_the_padding_is_fully_present() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    // Past the right edge (800) and the top edge (450), inside the 25%.
    let right = spawn_token(&mut app, "right", Vec2::new(1190.0, 0.0));
    let above = spawn_token(&mut app, "above", Vec2::new(0.0, 670.0));
    let corner = spawn_token(&mut app, "corner", Vec2::new(-1190.0, -670.0));
    settle(&mut app);

    for token in [right, above, corner] {
        assert!(!culled(&app, token));
        assert_eq!(furniture(&mut app, token), FULL);
    }
}

#[test]
fn panning_brings_a_token_in_before_the_screen_edge_reaches_it() {
    let mut app = app();
    let camera = spawn_camera(&mut app, Vec2::ZERO);
    let token = spawn_token(&mut app, "ahead", Vec2::new(5000.0, 0.0));
    settle(&mut app);
    assert_eq!(furniture(&mut app, token), NONE);

    // Still a long way off: the screen's right edge is at 3800.
    move_to(&mut app, camera, 3000.0, 0.0);
    settle(&mut app);
    assert!(culled(&app, token));

    // The screen's right edge is at 4600 — the token is 400 beyond it, a
    // quarter of a screen away and not yet visible — and it is already whole.
    move_to(&mut app, camera, 3800.0, 0.0);
    settle(&mut app);
    assert!(!culled(&app, token));
    assert_eq!(furniture(&mut app, token), FULL);

    // On screen, and nothing about it changes as it gets there.
    move_to(&mut app, camera, 4800.0, 0.0);
    settle(&mut app);
    assert_eq!(furniture(&mut app, token), FULL);

    // And leaving takes the furniture away again.
    move_to(&mut app, camera, 0.0, 0.0);
    settle(&mut app);
    assert!(culled(&app, token));
    assert_eq!(furniture(&mut app, token), NONE);
}

#[test]
fn zooming_out_brings_tokens_in_and_zooming_in_lets_them_go() {
    let mut app = app();
    let camera = spawn_camera(&mut app, Vec2::ZERO);
    let token = spawn_token(&mut app, "wide", Vec2::new(3000.0, 0.0));
    settle(&mut app);
    assert!(culled(&app, token));

    let set_view = |app: &mut App, size: Vec2| {
        let mut projection = app
            .world_mut()
            .get_mut::<Projection>(camera)
            .expect("a projection");
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.area = Rect::from_center_size(Vec2::ZERO, size);
        }
    };

    // Four times the world on screen: the right edge is at 3200.
    set_view(&mut app, Vec2::new(6400.0, 3600.0));
    settle(&mut app);
    assert!(!culled(&app, token));
    assert_eq!(furniture(&mut app, token), FULL);

    set_view(&mut app, Vec2::new(1600.0, 900.0));
    settle(&mut app);
    assert!(culled(&app, token));
    assert_eq!(furniture(&mut app, token), NONE);
}

#[test]
fn a_token_walking_in_from_outside_appears_as_it_crosses_the_padded_edge() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    let token = spawn_token(&mut app, "walker", Vec2::new(4000.0, 0.0));
    settle(&mut app);
    assert!(culled(&app, token));

    // The camera has not moved, so this is found by the token's own change.
    move_to(&mut app, token, 1500.0, 0.0);
    settle(&mut app);
    assert!(!culled(&app, token));
    assert_eq!(furniture(&mut app, token), FULL);
}

#[test]
fn between_the_two_edges_a_token_keeps_its_state_so_nothing_flickers() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    // 1600 is past "becomes present" (1520) and short of "is culled" (1680).
    let came_from_outside = spawn_token(&mut app, "out", Vec2::new(4000.0, 0.0));
    let came_from_inside = spawn_token(&mut app, "in", Vec2::new(0.0, 0.0));
    settle(&mut app);

    move_to(&mut app, came_from_outside, 1600.0, 0.0);
    move_to(&mut app, came_from_inside, 1600.0, 100.0);
    settle(&mut app);
    assert!(culled(&app, came_from_outside), "stays culled");
    assert!(!culled(&app, came_from_inside), "stays present");
    assert_eq!(furniture(&mut app, came_from_inside), FULL);

    // Nudged back and forth inside the band: neither changes its mind.
    for x in [1610.0, 1590.0, 1650.0, 1540.0] {
        move_to(&mut app, came_from_outside, x, 0.0);
        move_to(&mut app, came_from_inside, x, 100.0);
        settle(&mut app);
        assert!(culled(&app, came_from_outside));
        assert!(!culled(&app, came_from_inside));
    }
}

#[test]
fn the_selected_dragged_viewed_and_controlled_tokens_are_never_culled() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    let far = Vec2::new(9000.0, 9000.0);
    let selected = spawn_token(&mut app, "selected", far);
    let dragged = spawn_token(&mut app, "dragged", far);
    let viewer = spawn_token(&mut app, "viewer", far);
    let controlled = spawn_token(&mut app, "controlled", far);
    let nobody = spawn_token(&mut app, "nobody", far);

    app.world_mut()
        .resource_mut::<SelectedToken>()
        .select("selected".to_string());
    app.world_mut().resource_mut::<DraggingToken>().0 = vec![DraggedToken {
        id: "dragged".to_string(),
        offset: Vec2::ZERO,
        origin: Vec2::ZERO,
    }];
    app.world_mut().resource_mut::<ViewerToken>().0 = Some("viewer".to_string());
    app.world_mut().resource_mut::<ControlledToken>().0 = Some("controlled".to_string());
    settle(&mut app);

    for token in [selected, dragged, viewer, controlled] {
        assert!(!culled(&app, token));
        assert_eq!(furniture(&mut app, token), FULL);
    }
    assert!(culled(&app, nobody));

    // Selecting a culled token brings its furniture back with nothing else
    // having moved; letting go of everything culls them all.
    app.world_mut()
        .resource_mut::<SelectedToken>()
        .select("nobody".to_string());
    settle(&mut app);
    assert!(!culled(&app, nobody));
    assert_eq!(furniture(&mut app, nobody), FULL);
    assert!(culled(&app, selected));

    app.world_mut().resource_mut::<SelectedToken>().0.clear();
    app.world_mut().resource_mut::<DraggingToken>().0.clear();
    app.world_mut().resource_mut::<ViewerToken>().0 = None;
    app.world_mut().resource_mut::<ControlledToken>().0 = None;
    settle(&mut app);
    for token in [selected, dragged, viewer, controlled, nobody] {
        assert!(culled(&app, token));
        assert_eq!(furniture(&mut app, token), NONE);
    }
}

#[test]
fn a_token_hidden_by_vision_stays_hidden_in_view_and_out_of_it() {
    let mut app = app();
    let camera = spawn_camera(&mut app, Vec2::ZERO);
    let in_view = spawn_token(&mut app, "unseen-near", Vec2::new(100.0, 0.0));
    let out_of_view = spawn_token(&mut app, "unseen-far", Vec2::new(5000.0, 0.0));
    for token in [in_view, out_of_view] {
        app.world_mut().entity_mut(token).insert(Visibility::Hidden);
    }
    settle(&mut app);

    // In view it is not culled — and it is still hidden. Culling never
    // writes `Visibility`, so it has no way to reveal what vision withheld.
    assert!(!culled(&app, in_view));
    assert_eq!(
        app.world().get::<Visibility>(in_view),
        Some(&Visibility::Hidden)
    );
    assert!(culled(&app, out_of_view));
    assert_eq!(
        app.world().get::<Visibility>(out_of_view),
        Some(&Visibility::Hidden)
    );

    // Panning to the far one un-culls it, and it is still hidden.
    move_to(&mut app, camera, 5000.0, 0.0);
    settle(&mut app);
    assert!(!culled(&app, out_of_view));
    assert_eq!(
        app.world().get::<Visibility>(out_of_view),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Visibility>(in_view),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn a_camera_at_rest_over_a_still_board_judges_no_tokens() {
    let mut app = app();
    let camera = spawn_camera(&mut app, Vec2::ZERO);
    let mut tokens = Vec::new();
    for i in 0..200 {
        let at = Vec2::new((i % 20) as f32 * 400.0, (i / 20) as f32 * 400.0);
        tokens.push(spawn_token(&mut app, &format!("t{i}"), at));
    }
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 200);

    // At rest: nothing.
    for _ in 0..3 {
        app.update();
        assert_eq!(app.world().resource::<TokenCullWork>().judged, 0);
    }

    // One token moves: one token is judged.
    move_to(&mut app, tokens[7], 123.0, 456.0);
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 1);
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 0);

    // The camera trembles by less than 2% of the view: still nothing.
    move_to(&mut app, camera, 6.0, -4.0);
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 0);

    // The camera really moves: everything, once.
    move_to(&mut app, camera, 400.0, 0.0);
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 200);
    app.update();
    assert_eq!(app.world().resource::<TokenCullWork>().judged, 0);
}

#[test]
fn switching_culling_off_restores_every_token_and_on_culls_again() {
    let mut app = app();
    spawn_camera(&mut app, Vec2::ZERO);
    let far = spawn_token(&mut app, "far", Vec2::new(5000.0, 0.0));
    settle(&mut app);
    assert_eq!(furniture(&mut app, far), NONE);

    app.world_mut().resource_mut::<TokenCulling>().enabled = false;
    settle(&mut app);
    assert!(!culled(&app, far));
    assert_eq!(furniture(&mut app, far), FULL);

    app.world_mut().resource_mut::<TokenCulling>().enabled = true;
    settle(&mut app);
    assert!(culled(&app, far));
    assert_eq!(furniture(&mut app, far), NONE);
}

#[test]
fn with_no_camera_to_ask_nothing_is_culled() {
    let mut app = app();
    let far = spawn_token(&mut app, "far", Vec2::new(50_000.0, 0.0));
    settle(&mut app);
    assert!(!culled(&app, far));
    assert_eq!(furniture(&mut app, far), FULL);
}

#[test]
fn a_token_renamed_and_hurt_while_culled_returns_with_what_is_true_now() {
    let mut app = app();
    let camera = spawn_camera(&mut app, Vec2::ZERO);
    let token = spawn_token(&mut app, "Grom", Vec2::new(5000.0, 0.0));
    settle(&mut app);

    // Changes that would each have redrawn it, arriving while it is away.
    app.world_mut().entity_mut(token).insert(TokenName {
        text: Some("Grom the Bold".to_string()),
        hidden_from_players: false,
    });
    let mut two = status();
    two.resources.push(two.resources[0].clone());
    app.world_mut().entity_mut(token).insert(two);
    settle(&mut app);
    assert_eq!(furniture(&mut app, token), NONE, "still nothing drawn");

    move_to(&mut app, camera, 5000.0, 0.0);
    settle(&mut app);
    assert_eq!(furniture(&mut app, token), (2, 4));
    let mut names = app.world_mut().query::<(&Text2d, &Nameplate)>();
    for (text, _) in names.iter(app.world()) {
        assert_eq!(text.0, "Grom the Bold");
    }
}
