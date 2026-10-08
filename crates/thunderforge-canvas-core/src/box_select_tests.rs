use super::*;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_string()).collect()
}

fn unit_box() -> ScreenBox {
    ScreenBox::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
}

#[test]
fn from_corners_orders_the_corners_whichever_way_the_box_is_dragged() {
    let expected = ScreenBox {
        min: Vec2::new(0.0, 0.0),
        max: Vec2::new(10.0, 10.0),
    };
    let drags = [
        (Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
        (Vec2::new(10.0, 10.0), Vec2::new(0.0, 0.0)),
        (Vec2::new(0.0, 10.0), Vec2::new(10.0, 0.0)),
        (Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)),
    ];
    for (a, b) in drags {
        assert_eq!(ScreenBox::from_corners(a, b), expected, "{a} → {b}");
    }
}

#[test]
fn a_point_is_inside_when_it_lies_within_or_on_the_edge() {
    let b = unit_box();
    assert!(wholly_inside(&b, &Footprint::Point(Vec2::new(5.0, 5.0))));
    assert!(wholly_inside(&b, &Footprint::Point(Vec2::new(10.0, 0.0))));
    assert!(!wholly_inside(&b, &Footprint::Point(Vec2::new(10.5, 5.0))));
}

#[test]
fn a_segment_is_inside_only_with_both_ends_inside() {
    let b = unit_box();
    assert!(wholly_inside(
        &b,
        &Footprint::Segment(Vec2::new(1.0, 1.0), Vec2::new(9.0, 9.0))
    ));
    assert!(!wholly_inside(
        &b,
        &Footprint::Segment(Vec2::new(1.0, 1.0), Vec2::new(12.0, 9.0))
    ));
}

#[test]
fn a_rect_is_inside_only_when_it_does_not_cross_the_edge() {
    let b = unit_box();
    assert!(wholly_inside(
        &b,
        &Footprint::Rect {
            min: Vec2::new(2.0, 2.0),
            max: Vec2::new(4.0, 4.0)
        }
    ));
    assert!(!wholly_inside(
        &b,
        &Footprint::Rect {
            min: Vec2::new(8.0, 8.0),
            max: Vec2::new(12.0, 12.0)
        }
    ));
    assert!(!wholly_inside(
        &b,
        &Footprint::Rect {
            min: Vec2::new(-1.0, -1.0),
            max: Vec2::new(11.0, 11.0)
        }
    ));
}

#[test]
fn without_toggle_the_box_takes_its_hits() {
    assert_eq!(
        apply_box(&ids(&["a", "b"]), &ids(&["c", "d"]), false),
        ids(&["c", "d"])
    );
}

#[test]
fn with_toggle_held_hits_leave_and_the_others_join_in_order() {
    assert_eq!(
        apply_box(&ids(&["a", "b", "c"]), &ids(&["b", "d", "e"]), true),
        ids(&["a", "c", "d", "e"])
    );
}

#[test]
fn an_empty_toggle_changes_nothing() {
    assert_eq!(apply_box(&ids(&["a", "b"]), &[], true), ids(&["a", "b"]));
}

#[test]
fn no_id_is_taken_twice() {
    assert_eq!(
        apply_box(&[], &ids(&["a", "a", "b"]), false),
        ids(&["a", "b"])
    );
    assert_eq!(
        apply_box(&ids(&["a"]), &ids(&["b", "b"]), true),
        ids(&["a", "b"])
    );
}
