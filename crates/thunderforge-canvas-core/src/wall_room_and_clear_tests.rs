use super::*;

fn wall(id: &str) -> Wall {
    Wall {
        id: id.to_string(),
        x1: 0.0,
        y1: 0.0,
        x2: 10.0,
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: false,
        door_state: DoorState::None,
        locked: false,
        secret: false,
    }
}

#[test]
fn clearing_removes_every_wall() {
    let mut set = WallSet::default();
    set.upsert(wall("a"));
    set.upsert(wall("b"));
    set.clear();
    assert!(set.walls().is_empty());
    assert!(set.get("a").is_none());
}

#[test]
fn clearing_discards_the_undo_history_too() {
    // An undo entry names a wall id from a scene nobody is looking at any
    // more. Replaying one would emit a mutation against the wrong scene,
    // which is worse than having no undo across a scene change.
    let mut set = WallSet::default();
    set.upsert(wall("a"));
    set.push_undo(WallEdit::Delete { deleted: wall("a") });
    set.clear();
    assert_eq!(set.undo_stack_len(), 0);
    assert!(set.pop_undo().is_none());
}

#[test]
fn clearing_marks_the_set_dirty() {
    // Occlusion and illumination recompute off `dirty`. A cleared scene
    // that did not set it would keep casting the previous scene's shadows.
    let mut set = WallSet::default();
    set.upsert(wall("a"));
    set.dirty = false;
    set.clear();
    assert!(set.dirty);
}

#[test]
fn a_room_is_four_segments_that_close() {
    let room = room_segments(Vec2::ZERO, Vec2::new(100.0, 50.0)).expect("non-degenerate");
    assert_eq!(room.len(), 4);
    // Each segment's end is the next one's start, and the last closes back
    // onto the first. This is the property the vision pass depends on: a
    // corner that misses by a fraction of a unit is a corner light escapes
    // through.
    for pair in 0..4 {
        assert_eq!(room[pair].1, room[(pair + 1) % 4].0, "segment {pair}");
    }
}

#[test]
fn a_room_covers_exactly_the_dragged_rectangle() {
    let room = room_segments(Vec2::new(-20.0, 5.0), Vec2::new(30.0, 45.0)).expect("room");
    let xs: Vec<f32> = room.iter().flat_map(|(a, b)| [a.x, b.x]).collect();
    let ys: Vec<f32> = room.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
    assert_eq!(xs.iter().cloned().fold(f32::MAX, f32::min), -20.0);
    assert_eq!(xs.iter().cloned().fold(f32::MIN, f32::max), 30.0);
    assert_eq!(ys.iter().cloned().fold(f32::MAX, f32::min), 5.0);
    assert_eq!(ys.iter().cloned().fold(f32::MIN, f32::max), 45.0);
}

#[test]
fn the_drag_direction_does_not_change_the_room() {
    // Four ways to drag out the same rectangle. A Game Master should not
    // have to start from a particular corner.
    let reference = room_segments(Vec2::new(0.0, 0.0), Vec2::new(60.0, 40.0)).expect("room");
    for (a, b) in [
        (Vec2::new(60.0, 40.0), Vec2::new(0.0, 0.0)),
        (Vec2::new(0.0, 40.0), Vec2::new(60.0, 0.0)),
        (Vec2::new(60.0, 0.0), Vec2::new(0.0, 40.0)),
    ] {
        assert_eq!(
            room_segments(a, b).expect("room"),
            reference,
            "{a:?}->{b:?}"
        );
    }
}

#[test]
fn a_flat_drag_is_not_a_room() {
    // No interior means no room. Emitting two pairs of coincident walls
    // would leave four walls stacked on a line with nothing to show for it.
    assert!(room_segments(Vec2::ZERO, Vec2::new(100.0, 0.0)).is_none());
    assert!(room_segments(Vec2::ZERO, Vec2::new(0.0, 100.0)).is_none());
    assert!(room_segments(Vec2::ZERO, Vec2::ZERO).is_none());
}

#[test]
fn a_non_finite_corner_is_not_a_room() {
    // A cursor position can arrive as NaN when the camera projection is
    // degenerate. Four walls at NaN would be invisible and unselectable.
    assert!(room_segments(Vec2::ZERO, Vec2::new(f32::NAN, 10.0)).is_none());
    assert!(room_segments(Vec2::new(f32::INFINITY, 0.0), Vec2::new(10.0, 10.0)).is_none());
}

#[test]
fn room_walls_built_from_the_segments_enclose_their_interior() {
    // The point of drawing a room rather than four walls: a sightline from
    // inside to outside is blocked on every side.
    let mut set = WallSet::default();
    for (index, (a, b)) in room_segments(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0))
        .expect("room")
        .into_iter()
        .enumerate()
    {
        let mut segment = wall(&format!("r{index}"));
        segment.x1 = a.x;
        segment.y1 = a.y;
        segment.x2 = b.x;
        segment.y2 = b.y;
        set.upsert(segment);
    }

    let inside = Vec2::ZERO;
    for outside in [
        Vec2::new(0.0, 200.0),
        Vec2::new(0.0, -200.0),
        Vec2::new(200.0, 0.0),
        Vec2::new(-200.0, 0.0),
    ] {
        assert!(
            !is_visible(inside, outside, &set),
            "the room leaked toward {outside:?}",
        );
    }
}
