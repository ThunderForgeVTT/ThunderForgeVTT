use super::*;

#[test]
fn translate_rect_shifts_position_keeps_size() {
    let g = json!({ "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 });
    let moved = translate(ShapeKind::Rect, &g, Vec2::new(5.0, -5.0));
    assert_eq!(moved["x"], 5.0);
    assert_eq!(moved["y"], -5.0);
    assert_eq!(moved["w"], 10.0);
    assert_eq!(moved["h"], 10.0);
}

#[test]
fn translate_stroke_shifts_every_point() {
    let g = json!({ "points": [[0.0, 0.0], [1.0, 1.0]] });
    let moved = translate(ShapeKind::Stroke, &g, Vec2::new(2.0, 3.0));
    let points = moved["points"].as_array().unwrap();
    assert_eq!(points[0], json!([2.0, 3.0]));
    assert_eq!(points[1], json!([3.0, 4.0]));
}

fn bounds(kind: ShapeKind, g: Value) -> Option<(Vec2, Vec2)> {
    shape_bounds(kind, &g)
}

#[test]
fn a_rect_is_bounded_by_its_corner_and_size() {
    assert_eq!(
        bounds(
            ShapeKind::Rect,
            json!({ "x": 10.0, "y": 20.0, "w": 30.0, "h": 40.0 })
        ),
        Some((Vec2::new(10.0, 20.0), Vec2::new(40.0, 60.0)))
    );
}

#[test]
fn an_ellipse_is_bounded_like_its_rect() {
    assert_eq!(
        bounds(
            ShapeKind::Ellipse,
            json!({ "x": -5.0, "y": -5.0, "w": 10.0, "h": 4.0 })
        ),
        Some((Vec2::new(-5.0, -5.0), Vec2::new(5.0, -1.0)))
    );
}

#[test]
fn a_line_is_bounded_by_its_ends_whichever_way_it_runs() {
    assert_eq!(
        bounds(
            ShapeKind::Line,
            json!({ "x1": 50.0, "y1": 0.0, "x2": 10.0, "y2": 30.0 })
        ),
        Some((Vec2::new(10.0, 0.0), Vec2::new(50.0, 30.0)))
    );
}

#[test]
fn a_stroke_is_bounded_by_its_points() {
    assert_eq!(
        bounds(
            ShapeKind::Stroke,
            json!({ "points": [[0.0, 5.0], [-3.0, 2.0], [4.0, 9.0]] })
        ),
        Some((Vec2::new(-3.0, 2.0), Vec2::new(4.0, 9.0)))
    );
    assert_eq!(bounds(ShapeKind::Stroke, json!({ "points": [] })), None);
}

#[test]
fn text_is_bounded_by_its_anchor() {
    assert_eq!(
        bounds(ShapeKind::Text, json!({ "x": 7.0, "y": 8.0 })),
        Some((Vec2::new(7.0, 8.0), Vec2::new(7.0, 8.0)))
    );
}
