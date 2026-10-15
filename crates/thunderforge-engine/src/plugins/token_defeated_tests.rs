use super::*;
use crate::plugins::token_culling::ViewportCull;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(TokenDefeatedPlugin);
    app
}

const ART: Color = Color::WHITE;

fn spawn_token(app: &mut App, down: bool) -> Entity {
    let token = app
        .world_mut()
        .spawn((
            Transform::default(),
            TokenIdentity("t1".to_string()),
            Sprite::from_color(ART, Vec2::ONE),
            TokenDefeated(down),
        ))
        .id();
    app.update();
    token
}

fn set(app: &mut App, token: Entity, down: bool) {
    app.world_mut()
        .entity_mut(token)
        .insert(TokenDefeated(down));
    app.update();
}

fn mark_pieces(app: &mut App) -> usize {
    let mut query = app.world_mut().query_filtered::<(), With<DefeatMark>>();
    query.iter(app.world()).count()
}

fn art_color(app: &App, token: Entity) -> Color {
    app.world().get::<Sprite>(token).unwrap().color
}

/// The widest piece of the mark, for its size.
fn widest_piece(app: &mut App) -> f32 {
    let mut query = app
        .world_mut()
        .query_filtered::<&Sprite, With<DefeatMark>>();
    query
        .iter(app.world())
        .filter_map(|s| s.custom_size.map(|v| v.x.max(v.y)))
        .fold(0.0, f32::max)
}

#[test]
fn a_standing_token_draws_nothing_and_keeps_its_colour() {
    let mut app = app();
    let token = spawn_token(&mut app, false);
    assert_eq!(mark_pieces(&mut app), 0);
    assert_eq!(art_color(&app, token), ART);
}

#[test]
fn a_token_placed_down_is_drawn_down_at_once() {
    let mut app = app();
    let token = spawn_token(&mut app, true);
    assert!(mark_pieces(&mut app) > 0);
    assert_eq!(art_color(&app, token), dimmed(ART));
    assert_ne!(dimmed(ART), ART);
}

#[test]
fn a_down_token_stays_down_when_it_moves() {
    let mut app = app();
    let token = spawn_token(&mut app, true);
    let before = mark_pieces(&mut app);
    app.world_mut()
        .get_mut::<Transform>(token)
        .unwrap()
        .translation = Vec3::new(40.0, 10.0, 0.0);
    app.update();
    app.update();
    assert_eq!(mark_pieces(&mut app), before);
    assert_eq!(art_color(&app, token), dimmed(ART));
}

#[test]
fn getting_up_restores_the_colour_and_takes_the_mark_away() {
    let mut app = app();
    let token = spawn_token(&mut app, true);
    set(&mut app, token, false);
    assert_eq!(mark_pieces(&mut app), 0);
    assert_eq!(art_color(&app, token), ART);
    assert!(app.world().get::<DrawnDefeated>(token).is_none());
}

#[test]
fn new_art_arriving_while_down_is_dimmed_too() {
    let mut app = app();
    let token = spawn_token(&mut app, true);
    let red = Color::srgb(1.0, 0.0, 0.0);
    app.world_mut().get_mut::<Sprite>(token).unwrap().color = red;
    app.update();
    assert_eq!(art_color(&app, token), dimmed(red));
    set(&mut app, token, false);
    assert_eq!(art_color(&app, token), red);
}

#[test]
fn a_culled_token_builds_no_mark_and_builds_it_back_in_view() {
    let mut app = app();
    let token = spawn_token(&mut app, false);
    app.world_mut()
        .entity_mut(token)
        .insert(ViewportCull { culled: true });
    set(&mut app, token, true);
    assert_eq!(mark_pieces(&mut app), 0);
    app.world_mut()
        .entity_mut(token)
        .insert(ViewportCull { culled: false });
    app.update();
    assert!(mark_pieces(&mut app) > 0);
}

#[test]
fn the_mark_grows_with_the_token() {
    let mut app = app();
    app.insert_resource(SceneGrid::from_server("square", 100.0, Vec2::ZERO));
    let token = spawn_token(&mut app, true);
    let medium = widest_piece(&mut app);
    app.world_mut()
        .entity_mut(token)
        .insert(TokenGridBehaviour {
            footprint: thunderforge_canvas_core::grid::Footprint::new(2.0),
            ..Default::default()
        });
    app.update();
    let large = widest_piece(&mut app);
    assert!(large > medium * 1.9, "{medium} then {large}");
}

#[test]
fn the_report_lists_the_tokens_drawn_down() {
    let mut app = app();
    spawn_token(&mut app, true);
    let mut query = app.world_mut().query::<(&TokenIdentity, &TokenDefeated)>();
    let down: Vec<String> = query
        .iter(app.world())
        .filter(|(_, d)| d.0)
        .map(|(id, _)| id.0.clone())
        .collect();
    assert_eq!(report_of(down), r#"["t1"]"#);
}
