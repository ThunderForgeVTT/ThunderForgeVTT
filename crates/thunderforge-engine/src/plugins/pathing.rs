//! Spec 097 FR-008: the host asks the engine for a way round its walls.
//!
//! The search itself is `thunderforge_canvas_core::pathfinding`; this keeps
//! a copy of the walls and grid the engine holds, refreshed when either
//! changes, so the export can answer between frames without a `World`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use serde::Deserialize;
use thunderforge_canvas_core::grid::{Cell, GridKind, GridSpec};
use thunderforge_canvas_core::pathfinding::{PathOptions, find_path};
use thunderforge_canvas_core::wall::WallSet as CoreWalls;

use crate::resources::SceneGrid;
use crate::resources::wall::WallSet;

type Snapshot = (CoreWalls, Option<GridSpec>);

static SNAPSHOT: OnceLock<Mutex<Snapshot>> = OnceLock::new();

fn slot() -> &'static Mutex<Snapshot> {
    SNAPSHOT.get_or_init(|| Mutex::new((CoreWalls::default(), None)))
}

pub struct PathingPlugin;

impl Plugin for PathingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, mirror_for_pathing);
    }
}

fn mirror_for_pathing(walls: Option<Res<WallSet>>, grid: Option<Res<SceneGrid>>) {
    let walls_changed = walls.as_ref().is_some_and(|w| w.is_changed());
    let grid_changed = grid.as_ref().is_some_and(|g| g.is_changed());
    if !walls_changed && !grid_changed {
        return;
    }
    if let Ok(mut snapshot) = slot().lock() {
        if let Some(walls) = walls {
            snapshot.0 = walls.0.clone();
        }
        snapshot.1 = grid.map(|g| g.0);
    }
}

#[derive(Deserialize)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Deserialize, Clone, Copy)]
struct CellAt {
    q: i32,
    r: i32,
}

#[derive(Deserialize)]
struct Bounds {
    min: CellAt,
    max: CellAt,
}

/// A host's own lattice, when its cells are not the scene's.
#[derive(Deserialize)]
struct GridAt {
    size: f32,
    #[serde(default, rename = "originX")]
    origin_x: f32,
    #[serde(default, rename = "originY")]
    origin_y: f32,
}

#[derive(Deserialize)]
struct Request {
    from: Point,
    to: Point,
    #[serde(default)]
    bounds: Option<Bounds>,
    #[serde(default)]
    blocked: Vec<CellAt>,
    #[serde(default, rename = "maxNodes")]
    max_nodes: Option<usize>,
    #[serde(default)]
    grid: Option<GridAt>,
}

/// The answer to `request` over `walls` and `grid`, as the export returns it.
pub(crate) fn plan(request: &str, walls: &CoreWalls, grid: Option<GridSpec>) -> String {
    let none = || serde_json::json!({ "path": null }).to_string();
    let Ok(request) = serde_json::from_str::<Request>(request) else {
        return none();
    };
    let grid = match (&request.grid, grid) {
        (Some(g), _) => GridSpec {
            kind: GridKind::Square,
            size: g.size,
            origin: Vec2::new(g.origin_x, g.origin_y),
        },
        (None, Some(g)) => g,
        (None, None) => GridSpec::default(),
    };
    let cell = |c: CellAt| Cell::new(c.q, c.r);
    let mut options = PathOptions {
        bounds: request.bounds.map(|b| (cell(b.min), cell(b.max))),
        blocked: request
            .blocked
            .into_iter()
            .map(cell)
            .collect::<HashSet<_>>(),
        ..PathOptions::default()
    };
    if let Some(max) = request.max_nodes {
        options.max_nodes = max;
    }
    let from = Vec2::new(request.from.x, request.from.y);
    let to = Vec2::new(request.to.x, request.to.y);
    match find_path(&grid, from, to, walls, &options) {
        Some(path) => {
            let points: Vec<_> = path
                .iter()
                .map(|p| serde_json::json!({ "x": p.x, "y": p.y }))
                .collect();
            serde_json::json!({ "path": points }).to_string()
        }
        None => none(),
    }
}

/// A way from `from` to `to` round the walls and closed doors the engine
/// holds: `{"from":{x,y},"to":{x,y},"bounds"?:{min:{q,r},max:{q,r}},
/// "blocked"?:[{q,r}],"maxNodes"?,"grid"?:{size,originX,originY}}` in, and
/// `{"path":[{x,y},…]}` or `{"path":null}` out, in world units.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn plan_path(request: &str) -> String {
    match slot().lock() {
        Ok(snapshot) => plan(request, &snapshot.0, snapshot.1),
        Err(_) => serde_json::json!({ "path": null }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_canvas_core::wall::{DoorState, Wall};

    fn walls() -> CoreWalls {
        let mut set = CoreWalls::default();
        set.upsert(Wall {
            id: "w".into(),
            x1: 50.0,
            y1: -100.0,
            x2: 50.0,
            y2: 40.0,
            blocks_vision: true,
            blocks_movement: true,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        });
        set
    }

    fn path_of(answer: &str) -> Option<Vec<(f32, f32)>> {
        let value: serde_json::Value = serde_json::from_str(answer).unwrap();
        value["path"].as_array().map(|points| {
            points
                .iter()
                .map(|p| {
                    (
                        p["x"].as_f64().unwrap() as f32,
                        p["y"].as_f64().unwrap() as f32,
                    )
                })
                .collect()
        })
    }

    #[test]
    fn a_request_round_a_wall_is_answered_with_waypoints() {
        let request = r#"{"from":{"x":5,"y":5},"to":{"x":95,"y":5},
            "bounds":{"min":{"q":-5,"r":-5},"max":{"q":15,"r":15}},
            "grid":{"size":10}}"#;
        let path = path_of(&plan(request, &walls(), None)).unwrap();
        assert_eq!(path.first(), Some(&(5.0, 5.0)));
        assert_eq!(path.last(), Some(&(95.0, 5.0)));
        assert!(path.iter().any(|&(_, y)| y > 40.0), "{path:?}");
    }

    #[test]
    fn no_way_and_nonsense_both_answer_null() {
        let fenced = r#"{"from":{"x":5,"y":5},"to":{"x":95,"y":5},
            "bounds":{"min":{"q":0,"r":0},"max":{"q":9,"r":3}},
            "grid":{"size":10}}"#;
        assert_eq!(path_of(&plan(fenced, &walls(), None)), None);
        assert_eq!(path_of(&plan("not json", &walls(), None)), None);
    }

    #[test]
    fn the_scene_grid_is_used_when_the_host_names_none() {
        let grid = GridSpec {
            kind: GridKind::Square,
            size: 10.0,
            origin: Vec2::ZERO,
        };
        let request = r#"{"from":{"x":5,"y":5},"to":{"x":95,"y":5},
            "bounds":{"min":{"q":-5,"r":-5},"max":{"q":15,"r":15}}}"#;
        assert!(path_of(&plan(request, &walls(), Some(grid))).is_some());
    }
}
