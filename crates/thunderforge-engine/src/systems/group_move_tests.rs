use super::*;
use crate::resources::{DoorState, ShapeKind};
use thunderforge_canvas_core::grid::{GridKind, GridSpec};

fn wall() -> Wall {
    Wall {
        id: "w1".into(),
        x1: 0.0,
        y1: 0.0,
        x2: 100.0,
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state: DoorState::None,
        locked: false,
        secret: false,
    }
}

fn light() -> LightSource {
    LightSource {
        id: "l1".into(),
        x: 10.0,
        y: 20.0,
        radius: 100.0,
        intensity: 1.0,
        color: None,
        attached_token_id: None,
        casts_shadows: true,
        bright_radius: None,
    }
}

fn shape(created_by: Option<&str>) -> Shape {
    Shape {
        id: "s1".into(),
        kind: ShapeKind::Rect,
        geometry: json!({ "x": 5.0, "y": 5.0, "w": 10.0, "h": 10.0 }),
        text: None,
        style: None,
        visible_to_players: true,
        created_by: created_by.map(str::to_string),
    }
}

fn members() -> Vec<Member> {
    vec![
        Member::Token {
            id: "t1".into(),
            at: Vec3::new(25.0, 25.0, 3.0),
            scale: 1.5,
            rotation: 0.5,
        },
        Member::Wall(wall()),
        Member::Light(light()),
        Member::Shape(shape(None)),
    ]
}

fn stamp() -> GroupStamp {
    GroupStamp {
        id: "g-1".into(),
        size: 4,
        refused: 0,
    }
}

fn square(enabled: bool) -> SnapRule {
    SnapRule::new(
        GridSpec {
            kind: GridKind::Square,
            size: 50.0,
            origin: Vec2::ZERO,
        },
        enabled,
    )
}

#[test]
fn a_token_gets_upsert_token_with_its_fields_moved() {
    let events = release_events(&members(), Vec2::new(50.0, -50.0), &stamp(), "world");
    assert_eq!(
        events[0],
        json!({
            "type": "upsert_token",
            "token": { "id": "t1", "x": 75.0, "y": -25.0, "z": 3.0, "scale": 1.5, "rotation": 0.5 },
            "worldId": "world",
            "group": { "id": "g-1", "size": 4 },
        })
    );
}

#[test]
fn a_wall_gets_update_wall_with_all_four_ends_moved() {
    let events = release_events(&members(), Vec2::new(50.0, -50.0), &stamp(), "world");
    assert_eq!(events[1]["type"], "update_wall");
    assert_eq!(events[1]["wallId"], "w1");
    assert_eq!(
        events[1]["changes"],
        json!({ "x1": 50.0, "y1": -50.0, "x2": 150.0, "y2": -50.0 })
    );
}

#[test]
fn a_light_gets_update_light_with_x_and_y() {
    let events = release_events(&members(), Vec2::new(50.0, -50.0), &stamp(), "world");
    assert_eq!(events[2]["type"], "update_light");
    assert_eq!(events[2]["lightId"], "l1");
    assert_eq!(events[2]["changes"], json!({ "x": 60.0, "y": -30.0 }));
}

#[test]
fn a_shape_gets_update_shape_with_translated_geometry() {
    let events = release_events(&members(), Vec2::new(50.0, -50.0), &stamp(), "world");
    assert_eq!(events[3]["type"], "update_shape");
    assert_eq!(events[3]["shapeId"], "s1");
    assert_eq!(
        events[3]["changes"]["geometry"],
        json!({ "x": 55.0, "y": -45.0, "w": 10.0, "h": 10.0 })
    );
}

#[test]
fn every_event_carries_the_same_group_sized_to_the_count() {
    let mut stamps = GroupStamps::default();
    let all = members();
    let stamp = stamps.next(all.len());
    let events = release_events(&all, Vec2::ONE, &stamp, "world");
    assert_eq!(events.len(), stamp.size);
    for event in &events {
        assert_eq!(event["group"], json!({ "id": "g-1", "size": 4 }));
    }
    assert_eq!(stamps.next(2).id, "g-2");
}

#[test]
fn the_offset_follows_the_pressed_tokens_snapping() {
    let rule = square(true);
    let token = Anchor::Token {
        center: Vec2::new(25.0, 25.0),
        footprint: Footprint::default(),
        snaps: true,
    };
    // A one-cell token sits on a cell centre: 25 + 62 snaps to 75.
    assert_eq!(
        snapped_offset(token, Vec2::new(62.0, 3.0), &rule),
        Vec2::new(50.0, 0.0)
    );
    // A token that does not snap moves by the raw travel.
    let free_token = Anchor::Token {
        center: Vec2::new(25.0, 25.0),
        footprint: Footprint::default(),
        snaps: false,
    };
    assert_eq!(
        snapped_offset(free_token, Vec2::new(62.0, 3.0), &rule),
        Vec2::new(62.0, 3.0)
    );
}

#[test]
fn the_offset_follows_a_wall_to_a_corner_a_light_to_a_cell_and_a_shape_freely() {
    let rule = square(true);
    assert_eq!(
        snapped_offset(
            Anchor::Wall { start: Vec2::ZERO },
            Vec2::new(62.0, 3.0),
            &rule
        ),
        Vec2::new(50.0, 0.0)
    );
    assert_eq!(
        snapped_offset(
            Anchor::Light {
                at: Vec2::new(25.0, 25.0)
            },
            Vec2::new(40.0, 0.0),
            &rule
        ),
        Vec2::new(50.0, 0.0)
    );
    assert_eq!(
        snapped_offset(Anchor::Free, Vec2::new(62.0, 3.0), &rule),
        Vec2::new(62.0, 3.0)
    );
}

#[test]
fn with_snapping_off_the_offset_is_the_raw_travel() {
    let rule = square(false);
    assert_eq!(
        snapped_offset(
            Anchor::Wall { start: Vec2::ZERO },
            Vec2::new(62.0, 3.0),
            &rule
        ),
        Vec2::new(62.0, 3.0)
    );
}

#[test]
fn a_gm_deletes_every_member_but_a_carried_light() {
    let group = GroupSelection {
        tokens: vec!["t1".into()],
        walls: vec!["w1".into()],
        lights: vec!["l1".into(), "carried:t1".into()],
        shapes: vec!["s1".into()],
    };
    let doomed = deletable(
        &group,
        true,
        Some("gm"),
        |_| Some(shape(None)),
        |id| id.starts_with("carried:"),
    );
    assert_eq!(doomed.tokens, vec!["t1".to_string()]);
    assert_eq!(doomed.walls, vec!["w1".to_string()]);
    assert_eq!(doomed.lights, vec!["l1".to_string()]);
    assert_eq!(doomed.shapes, vec!["s1".to_string()]);

    let events = delete_events(
        &doomed,
        &GroupStamp {
            id: "g-3".into(),
            size: 4,
            refused: 0,
        },
        "world",
    );
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        vec![
            "remove_token",
            "delete_wall",
            "delete_light",
            "delete_shape"
        ]
    );
    for event in &events {
        assert_eq!(event["group"], json!({ "id": "g-3", "size": 4 }));
    }
}

fn two_tokens() -> Vec<Member> {
    vec![
        Member::Token {
            id: "near".into(),
            at: Vec3::new(-25.0, 0.0, 1.0),
            scale: 1.0,
            rotation: 0.0,
        },
        Member::Token {
            id: "far".into(),
            at: Vec3::new(-25.0, 500.0, 1.0),
            scale: 1.0,
            rotation: 0.0,
        },
        Member::Shape(shape(Some("me"))),
    ]
}

/// A wall along x = 0 from y = -100 to y = 100.
fn crosses_the_wall(from: Vec2, to: Vec2) -> bool {
    (from.x < 0.0) != (to.x < 0.0) && from.y.abs() <= 100.0 && to.y.abs() <= 100.0
}

#[test]
fn a_players_token_whose_path_crosses_a_wall_is_not_sent_and_the_rest_are() {
    let (sent, refused) = judge_paths(two_tokens(), Vec2::new(50.0, 0.0), false, crosses_the_wall);
    let ids = |members: &[Member]| -> Vec<String> {
        members
            .iter()
            .map(|m| match m {
                Member::Token { id, .. } => id.clone(),
                Member::Shape(s) => s.id.clone(),
                _ => String::new(),
            })
            .collect()
    };
    assert_eq!(ids(&refused), vec!["near".to_string()]);
    assert_eq!(ids(&sent), vec!["far".to_string(), "s1".to_string()]);
}

#[test]
fn a_gms_group_is_not_judged_on_the_board() {
    let (sent, refused) = judge_paths(two_tokens(), Vec2::new(50.0, 0.0), true, crosses_the_wall);
    assert_eq!(sent.len(), 3);
    assert!(refused.is_empty());
}

#[test]
fn size_counts_only_what_was_sent_and_the_stamp_says_how_many_stayed() {
    let (sent, refused) = judge_paths(two_tokens(), Vec2::new(50.0, 0.0), false, crosses_the_wall);
    let mut stamps = GroupStamps::default();
    let stamp = stamps.next_judged(sent.len(), refused.len());
    let events = release_events(&sent, Vec2::new(50.0, 0.0), &stamp, "world");
    assert_eq!(events.len(), 2);
    for event in &events {
        assert_eq!(
            event["group"],
            json!({ "id": "g-1", "size": 2, "refused": 1 })
        );
    }
    // Nothing held back: the stamp is the plain one.
    assert_eq!(
        stamps.next_judged(2, 0).to_json(),
        json!({ "id": "g-2", "size": 2 })
    );
}

#[test]
fn a_players_delete_sends_only_the_shapes_they_may_edit() {
    let group = GroupSelection {
        tokens: vec!["t1".into()],
        walls: vec!["w1".into()],
        lights: vec!["l1".into()],
        shapes: vec!["mine".into(), "gms".into()],
    };
    let doomed = deletable(
        &group,
        false,
        Some("me"),
        |id| Some(shape(Some(if id == "mine" { "me" } else { "gm" }))),
        |_| false,
    );
    assert!(doomed.tokens.is_empty());
    assert!(doomed.walls.is_empty());
    assert!(doomed.lights.is_empty());
    assert_eq!(doomed.shapes, vec!["mine".to_string()]);
}
