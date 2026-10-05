use super::*;

fn wall(id: &str, y: f32, door_state: DoorState) -> Wall {
    Wall {
        id: id.to_string(),
        x1: 0.0,
        y1: y,
        x2: 100.0,
        y2: y,
        blocks_vision: true,
        blocks_movement: true,
        door_state,
        locked: false,
        secret: false,
    }
}

fn id_of(found: Option<&Wall>) -> Option<&str> {
    found.map(|wall| wall.id.as_str())
}

#[test]
fn a_game_master_is_asking_about_the_wall_within_reach() {
    let walls = [
        wall("near", 0.0, DoorState::None),
        wall("far", 50.0, DoorState::None),
    ];
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 4.0), 10.0, true)),
        Some("near")
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 25.0), 10.0, true)),
        None
    );
}

#[test]
fn a_player_is_only_ever_asking_about_a_door_they_are_shown() {
    let mut secret = wall("secret", 20.0, DoorState::Closed);
    secret.secret = true;
    let walls = [
        wall("plain", 0.0, DoorState::None),
        secret,
        wall("door", 40.0, DoorState::Closed),
    ];

    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 0.0), 10.0, false)),
        None
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 20.0), 10.0, false)),
        None
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 40.0), 10.0, false)),
        Some("door")
    );
    // The Game Master is shown it, so the Game Master may ask.
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 20.0), 10.0, true)),
        Some("secret")
    );
}

#[test]
fn a_door_beats_the_wall_it_is_set_in() {
    let walls = [
        wall("plain", 0.0, DoorState::None),
        wall("door", 6.0, DoorState::Open),
    ];
    // Nearer the plain wall, and still about the door.
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 1.0), 10.0, true)),
        Some("door")
    );
}

fn light(id: &str, x: f32, y: f32) -> LightSource {
    LightSource {
        id: id.to_string(),
        x,
        y,
        radius: 200.0,
        intensity: 1.0,
        color: None,
        attached_token_id: None,
        casts_shadows: true,
        bright_radius: None,
    }
}

fn shape(id: &str, kind: ShapeKind, geometry: serde_json::Value) -> Shape {
    Shape {
        id: id.to_string(),
        kind,
        geometry,
        text: None,
        style: None,
        visible_to_players: false,
    }
}

fn shape_id(found: Option<&Shape>) -> Option<&str> {
    found.map(|shape| shape.id.as_str())
}

#[test]
fn a_right_click_is_about_the_nearest_light_whose_marker_it_lands_on() {
    let lights = [light("near", 0.0, 0.0), light("far", 20.0, 0.0)];
    let found = |at| light_under(&lights, at, 15.0).map(|light| light.id.as_str());

    assert_eq!(found(Vec2::new(4.0, 3.0)), Some("near"));
    assert_eq!(found(Vec2::new(14.0, 0.0)), Some("far"));
    // Inside the light it casts, and nowhere near its marker.
    assert_eq!(found(Vec2::new(0.0, 100.0)), None);
}

#[test]
fn a_light_a_token_carries_or_wears_is_not_under_a_right_click() {
    let mut worn = light("worn", 0.0, 0.0);
    worn.attached_token_id = Some("token".to_string());
    let carried = light(
        &format!(
            "{}token",
            thunderforge_canvas_core::lighting::CARRIED_LIGHT_ID_PREFIX
        ),
        0.0,
        0.0,
    );
    assert!(carried.is_carried());

    assert!(light_under(&[worn, carried], Vec2::ZERO, 15.0).is_none());
}

#[test]
fn a_filled_rectangle_is_clicked_anywhere_on_it() {
    let shapes = [shape(
        "rect",
        ShapeKind::Rect,
        json!({ "x": 0.0, "y": 0.0, "w": 100.0, "h": 60.0 }),
    )];

    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(50.0, 30.0), 10.0)),
        Some("rect")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(106.0, 30.0), 10.0)),
        Some("rect")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(120.0, 30.0), 10.0)),
        None
    );
}

#[test]
fn an_ellipse_is_clicked_on_its_outline_and_the_board_inside_it_is_board() {
    let shapes = [shape(
        "ring",
        ShapeKind::Ellipse,
        json!({ "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0 }),
    )];

    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(197.0, 100.0), 10.0)),
        Some("ring")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(100.0, 100.0), 10.0)),
        None
    );
}

#[test]
fn a_line_and_a_stroke_are_clicked_along_their_length() {
    let shapes = [
        shape(
            "line",
            ShapeKind::Line,
            json!({ "x1": 0.0, "y1": 0.0, "x2": 100.0, "y2": 0.0 }),
        ),
        shape(
            "stroke",
            ShapeKind::Stroke,
            json!({ "points": [[0.0, 50.0], [50.0, 80.0], [100.0, 50.0]] }),
        ),
    ];

    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(60.0, 5.0), 10.0)),
        Some("line")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(50.0, 76.0), 10.0)),
        Some("stroke")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(50.0, 30.0), 10.0)),
        None
    );
}

#[test]
fn text_is_clicked_on_its_letters() {
    let mut label = shape("label", ShapeKind::Text, json!({ "x": 0.0, "y": 0.0 }));
    label.text = Some("Trapdoor".to_string());
    let shapes = [label];

    // Eight letters, set centred: forty either side of its point.
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(35.0, 4.0), 5.0)),
        Some("label")
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(60.0, 0.0), 5.0)),
        None
    );
    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(0.0, 30.0), 5.0)),
        None
    );
}

#[test]
fn of_two_drawings_under_a_right_click_it_is_about_the_one_on_top() {
    let on = json!({ "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0 });
    let shapes = [
        shape("under", ShapeKind::Rect, on.clone()),
        shape("over", ShapeKind::Rect, on),
    ];

    assert_eq!(
        shape_id(shape_under(&shapes, Vec2::new(50.0, 50.0), 10.0)),
        Some("over")
    );
}

#[test]
fn a_drawing_with_nothing_in_it_is_under_nothing() {
    let shapes = [shape("empty", ShapeKind::Stroke, json!({ "points": [] }))];

    assert_eq!(shape_id(shape_under(&shapes, Vec2::ZERO, 10.0)), None);
}
