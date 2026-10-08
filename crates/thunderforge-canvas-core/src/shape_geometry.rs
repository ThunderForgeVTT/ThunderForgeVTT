//! Where a shape's geometry sits, and moving it (spec 085).
//!
//! Each kind stores its position under its own field names (spec 012's
//! contracts/graphql.md): a rect or an ellipse as `x`, `y`, `w`, `h`, a line
//! as two ends, text as one point, a stroke as a list of points. Both the
//! single shape's move-drag and a group move translate geometry, and the box
//! measures it, so the reading of those fields lives here once.

use glam::Vec2;
use serde_json::{Value, json};

use crate::shape::ShapeKind;

fn num(geometry: &Value, field: &str) -> f32 {
    geometry[field].as_f64().unwrap_or(0.0) as f32
}

fn stroke_points(geometry: &Value) -> Vec<Vec2> {
    geometry["points"]
        .as_array()
        .map(|points| {
            points
                .iter()
                .map(|p| {
                    let pair = p.as_array().cloned().unwrap_or_default();
                    let x = pair.first().and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    let y = pair.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    Vec2::new(x, y)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The geometry moved by `delta`, size and shape kept.
pub fn translate(kind: ShapeKind, geometry: &Value, delta: Vec2) -> Value {
    match kind {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let w = geometry["w"].as_f64().unwrap_or(0.0);
            let h = geometry["h"].as_f64().unwrap_or(0.0);
            json!({
                "x": num(geometry, "x") + delta.x,
                "y": num(geometry, "y") + delta.y,
                "w": w,
                "h": h,
            })
        }
        ShapeKind::Line => json!({
            "x1": num(geometry, "x1") + delta.x, "y1": num(geometry, "y1") + delta.y,
            "x2": num(geometry, "x2") + delta.x, "y2": num(geometry, "y2") + delta.y,
        }),
        ShapeKind::Text => json!({
            "x": num(geometry, "x") + delta.x,
            "y": num(geometry, "y") + delta.y,
        }),
        ShapeKind::Stroke => {
            let points: Vec<Value> = stroke_points(geometry)
                .into_iter()
                .map(|p| json!([p.x + delta.x, p.y + delta.y]))
                .collect();
            json!({ "points": points })
        }
    }
}

/// The smallest rect holding the shape, as `(min, max)`. Text is its
/// anchor point, since the engine does not measure the rendered label. A
/// stroke with no points has no bounds.
pub fn shape_bounds(kind: ShapeKind, geometry: &Value) -> Option<(Vec2, Vec2)> {
    let points = match kind {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let corner = Vec2::new(num(geometry, "x"), num(geometry, "y"));
            let size = Vec2::new(num(geometry, "w"), num(geometry, "h"));
            vec![corner, corner + size]
        }
        ShapeKind::Line => vec![
            Vec2::new(num(geometry, "x1"), num(geometry, "y1")),
            Vec2::new(num(geometry, "x2"), num(geometry, "y2")),
        ],
        ShapeKind::Text => vec![Vec2::new(num(geometry, "x"), num(geometry, "y"))],
        ShapeKind::Stroke => stroke_points(geometry),
    };
    let first = *points.first()?;
    Some(
        points
            .iter()
            .fold((first, first), |(min, max), p| (min.min(*p), max.max(*p))),
    )
}

#[cfg(test)]
#[path = "shape_geometry_tests.rs"]
mod tests;
