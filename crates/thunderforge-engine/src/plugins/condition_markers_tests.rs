use super::*;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(ConditionMarkersPlugin);
    app
}

fn payload(id: &str, glyph: &str, color: &str) -> ConditionPayload {
    ConditionPayload {
        id: id.to_string(),
        glyph: glyph.to_string(),
        color: color.to_string(),
    }
}

fn spawn_token(app: &mut App, conditions: &[ConditionPayload]) -> Entity {
    app.world_mut()
        .spawn((
            Transform::default(),
            TokenIdentity("t1".to_string()),
            TokenConditions::from_payload(conditions),
        ))
        .id()
}

fn set(app: &mut App, token: Entity, conditions: &[ConditionPayload]) {
    app.world_mut()
        .entity_mut(token)
        .insert(TokenConditions::from_payload(conditions));
    app.update();
}

/// Every badge piece drawn, as (entity, colour).
fn pieces(app: &mut App) -> Vec<(Entity, Color)> {
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &Sprite), With<ConditionBadge>>();
    query
        .iter(app.world())
        .map(|(entity, sprite)| (entity, sprite.color))
        .collect()
}

/// What the engine holds as drawn, the way `mirror_conditions` gathers it.
/// Read from this world rather than the process-wide mirror, which every
/// test in the binary writes to.
fn drawn(app: &mut App) -> DrawnList {
    let mut query = app
        .world_mut()
        .query::<(&TokenIdentity, &DrawnConditions)>();
    query
        .iter(app.world())
        .map(|(id, drawn)| (id.0.clone(), drawn.markers.clone()))
        .collect()
}

/// How many pieces are drawn in one colour.
fn count_of(app: &mut App, color: Color) -> usize {
    pieces(app).iter().filter(|(_, c)| *c == color).count()
}

#[test]
fn a_token_under_no_condition_draws_nothing() {
    let mut app = app();
    spawn_token(&mut app, &[]);
    app.update();
    assert!(pieces(&mut app).is_empty());
}

#[test]
fn each_condition_draws_a_backing_and_its_glyph_in_its_colour() {
    let mut app = app();
    spawn_token(
        &mut app,
        &[
            payload("prone", "bar", "warning"),
            payload("stunned", "cross", "danger"),
        ],
    );
    app.update();

    assert_eq!(count_of(&mut app, backing_color()), 2);
    // A bar is one rectangle; a cross is two.
    assert_eq!(count_of(&mut app, Tone::Warning.color()), 1);
    assert_eq!(count_of(&mut app, Tone::Danger.color()), 2);
}

#[test]
fn it_is_drawn_from_identifier_and_marker_alone() {
    // Two conditions no ruleset has, declared the same way, draw alike: the
    // identifier chooses nothing.
    let mut app = app();
    let token = spawn_token(&mut app, &[payload("glorped", "ring", "arcane")]);
    app.update();
    let first = count_of(&mut app, Tone::Arcane.color());

    set(&mut app, token, &[payload("zzyzx", "ring", "arcane")]);
    assert_eq!(count_of(&mut app, Tone::Arcane.color()), first);
    assert_eq!(first, 4);
}

#[test]
fn a_glyph_or_colour_this_build_does_not_know_still_draws() {
    let mut app = app();
    spawn_token(&mut app, &[payload("new", "starburst", "ultraviolet")]);
    app.update();
    // As a dot, in the neutral colour.
    assert_eq!(count_of(&mut app, Tone::Neutral.color()), 1);
    assert_eq!(count_of(&mut app, backing_color()), 1);
}

#[test]
fn an_update_that_repeats_the_conditions_redraws_nothing() {
    let mut app = app();
    let token = spawn_token(&mut app, &[payload("prone", "bar", "warning")]);
    app.update();
    let before: Vec<Entity> = pieces(&mut app).into_iter().map(|(e, _)| e).collect();

    set(&mut app, token, &[payload("prone", "bar", "warning")]);
    let after: Vec<Entity> = pieces(&mut app).into_iter().map(|(e, _)| e).collect();
    assert_eq!(before, after);
}

#[test]
fn clearing_the_last_condition_removes_its_badge() {
    let mut app = app();
    let token = spawn_token(&mut app, &[payload("prone", "bar", "warning")]);
    app.update();
    assert!(!pieces(&mut app).is_empty());

    set(&mut app, token, &[]);
    assert!(pieces(&mut app).is_empty());
}

#[test]
fn a_culled_token_has_no_badges_and_gets_them_back_in_view() {
    let mut app = app();
    let token = spawn_token(&mut app, &[payload("prone", "bar", "warning")]);
    app.world_mut()
        .entity_mut(token)
        .insert(ViewportCull { culled: true });
    app.update();
    assert!(pieces(&mut app).is_empty());

    app.world_mut()
        .entity_mut(token)
        .insert(ViewportCull { culled: false });
    app.update();
    assert_eq!(count_of(&mut app, backing_color()), 1);
}

#[test]
fn badges_run_along_the_bottom_edge_and_wrap_upward() {
    let side = 50.0;
    let badge = badge_side(side);
    let first = badge_offset(0, side);
    // In the bottom-left corner, inside the token.
    assert_eq!(first, Vec2::splat(-side / 2.0 + badge / 2.0));

    let second = badge_offset(1, side);
    assert!(second.x > first.x);
    assert_eq!(second.y, first.y);

    // Fifteen conditions — a full 5e set — all stay inside the token's width.
    let mut rows = std::collections::BTreeSet::new();
    for index in 0..15 {
        let at = badge_offset(index, side);
        assert!(at.x + badge / 2.0 <= side / 2.0 + f32::EPSILON, "{at:?}");
        rows.insert(at.y.to_bits());
    }
    assert!(rows.len() > 1, "fifteen badges did not wrap");
}

#[test]
fn the_report_lists_what_was_drawn() {
    let mut app = app();
    let token = spawn_token(&mut app, &[payload("prone", "bar", "warning")]);
    app.update();
    let report: serde_json::Value =
        serde_json::from_str(&report_of(drawn(&mut app))).expect("the report is JSON");
    assert_eq!(
        report,
        serde_json::json!([{
            "tokenId": "t1",
            "conditions": [{ "id": "prone", "glyph": "bar", "color": "warning" }],
        }])
    );

    set(&mut app, token, &[]);
    assert_eq!(report_of(drawn(&mut app)), "[]");
}
