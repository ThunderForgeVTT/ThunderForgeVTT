//! Speech bubbles, run against a real `App`, so the claim tested is "a bubble
//! hangs over the token" and not merely "a command was parsed".
//!
//! Every test uses a 1600 × 900 view centred on the origin, so x runs from
//! -800 to 800 and y from -450 to 450. With no grid loaded a token is 96
//! wide, so its top edge — where a bubble anchors — is 48 above its centre.

use std::time::Duration;

use bevy::time::TimeUpdateStrategy;

use super::*;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(SpeechPlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
    let mut projection = OrthographicProjection::default_2d();
    projection.area = Rect::from_center_size(Vec2::ZERO, Vec2::new(1600.0, 900.0));
    app.world_mut().spawn((
        Camera2d,
        Transform::default(),
        Projection::Orthographic(projection),
    ));
    app
}

fn spawn_token(app: &mut App, id: &str, at: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            Transform::from_translation(at.extend(0.0)),
            GlobalTransform::from_translation(at.extend(0.0)),
            Visibility::Visible,
            TokenIdentity(id.to_string()),
        ))
        .id()
}

fn say(app: &mut App, id: &str, token: &str, text: &str, ms: u32) {
    app.world_mut().resource_mut::<Speech>().says.push(Say {
        id: id.to_string(),
        token_id: token.to_string(),
        name: "Brakka".to_string(),
        text: text.to_string(),
        kind: SpeechKind::Line,
        ms,
    });
}

/// The bubbles alive, as (token entity, line id, text).
fn bubbles(app: &mut App) -> Vec<(Entity, String, String)> {
    let mut query = app.world_mut().query::<(&ChildOf, &SpeechBubble)>();
    query
        .iter(app.world())
        .map(|(parent, b)| (parent.parent(), b.id.clone(), b.text.clone()))
        .collect()
}

fn placed(app: &App) -> Vec<(String, Placement)> {
    app.world().resource::<Speech>().placed.clone()
}

#[test]
fn a_line_said_as_a_visible_token_in_view_hangs_over_it() {
    let mut app = app();
    let token = spawn_token(&mut app, "brakka", Vec2::ZERO);
    say(&mut app, "chat:1", "brakka", "Hold the door!", 6000);
    app.update();
    assert_eq!(
        bubbles(&mut app),
        [(token, "chat:1".to_string(), "Hold the door!".to_string())]
    );
    assert_eq!(placed(&app), [("chat:1".to_string(), Placement::Over)]);
}

#[test]
fn a_second_line_replaces_the_first() {
    let mut app = app();
    spawn_token(&mut app, "brakka", Vec2::ZERO);
    say(&mut app, "chat:1", "brakka", "Hold the door!", 6000);
    app.update();
    say(&mut app, "chat:2", "brakka", "Run!", 6000);
    app.update();
    let ids: Vec<String> = bubbles(&mut app).into_iter().map(|(_, id, _)| id).collect();
    assert_eq!(ids, ["chat:2"]);
}

#[test]
fn a_hidden_token_wears_nothing_and_goes_to_the_edge() {
    let mut app = app();
    let token = spawn_token(&mut app, "lurker", Vec2::ZERO);
    app.world_mut().entity_mut(token).insert(Visibility::Hidden);
    say(&mut app, "chat:1", "lurker", "Boo", 6000);
    app.update();
    assert!(bubbles(&mut app).is_empty());
    assert_eq!(placed(&app), [("chat:1".to_string(), Placement::Edge)]);
}

#[test]
fn a_token_under_the_page_chrome_goes_to_the_edge() {
    let mut app = app();
    // Top edge at y = -400 + 48 = -352: in the view, under a 180 px drawer.
    spawn_token(&mut app, "low", Vec2::new(0.0, -400.0));
    // And one off the right of the screen.
    spawn_token(&mut app, "far", Vec2::new(900.0, 0.0));
    app.world_mut().resource_mut::<Speech>().insets = ViewInsets {
        bottom: 180.0,
        ..default()
    };
    say(&mut app, "chat:1", "low", "Down here", 6000);
    say(&mut app, "chat:2", "far", "Over here", 6000);
    app.update();
    assert!(bubbles(&mut app).is_empty());
    assert_eq!(
        placed(&app),
        [
            ("chat:1".to_string(), Placement::Edge),
            ("chat:2".to_string(), Placement::Edge)
        ]
    );
}

#[test]
fn a_line_for_a_token_not_on_the_board_goes_to_the_edge() {
    let mut app = app();
    say(&mut app, "chat:1", "nobody", "Hello?", 6000);
    app.update();
    assert_eq!(placed(&app), [("chat:1".to_string(), Placement::Edge)]);
}

#[test]
fn a_bubble_goes_after_its_time() {
    let mut app = app();
    spawn_token(&mut app, "brakka", Vec2::ZERO);
    say(&mut app, "chat:1", "brakka", "Quick", 300);
    app.update();
    assert_eq!(bubbles(&mut app).len(), 1);
    for _ in 0..5 {
        app.update();
    }
    assert!(bubbles(&mut app).is_empty());
}

#[test]
fn clear_speech_takes_every_bubble_down() {
    let mut app = app();
    spawn_token(&mut app, "a", Vec2::new(-100.0, 0.0));
    spawn_token(&mut app, "b", Vec2::new(100.0, 0.0));
    say(&mut app, "chat:1", "a", "One", 6000);
    say(&mut app, "chat:2", "b", "Two", 6000);
    app.update();
    assert_eq!(bubbles(&mut app).len(), 2);
    app.world_mut().resource_mut::<Speech>().clear = true;
    app.update();
    assert!(bubbles(&mut app).is_empty());
}

#[test]
fn a_token_that_turns_hidden_loses_its_bubble() {
    let mut app = app();
    let token = spawn_token(&mut app, "brakka", Vec2::ZERO);
    say(&mut app, "chat:1", "brakka", "Hold the door!", 6000);
    app.update();
    assert_eq!(bubbles(&mut app).len(), 1);
    // A viewer change: this token is now out of the viewer's sight.
    app.world_mut().entity_mut(token).insert(Visibility::Hidden);
    app.update();
    assert!(bubbles(&mut app).is_empty());
}

#[test]
fn a_long_line_ends_in_an_ellipsis() {
    let long = "a".repeat(SPEECH_MAX_CHARS + 30);
    let clipped = clip(&long);
    assert_eq!(clipped.chars().count(), SPEECH_MAX_CHARS);
    assert!(clipped.ends_with('…'));
    assert_eq!(clip("  short  "), "short");
}
