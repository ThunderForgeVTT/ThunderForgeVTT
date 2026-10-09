//! Spec 087 T034: the server and the browser measure a fight bit for bit alike.
//!
//! The server runs these rules on x86_64; the demo runs the same crate
//! compiled to wasm32. Float maths is not guaranteed to agree across the
//! two: a compiler may fuse a multiply and an add on one target and not on
//! the other, and a glam upgrade can move where that happens. A reach or a
//! move budget that comes out one bit apart is a different fight.
//!
//! `float_parity.json` holds reach and move-cost cases, deliberately at
//! awkward coordinates and on every grid kind, with the exact result the host
//! computes. This test pins the host to them; the demo's
//! `floatParity.test.ts` pins the wasm build to the same file. Together they
//! say the two targets agree to the bit.
//!
//! The JSON shapes are the wasm façade's (`measure` and `moveCost` in
//! `src/wasm.rs`), so the wasm side passes each input through unchanged.

use serde::Deserialize;
use serde_json::Value;
use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::wall::{DoorState, Wall, WallSet};
use thunderforge_combat::budget::move_cost;
use thunderforge_combat::reach::{Measured, Reach, measure, scene_grid};

const FIXTURES: &str = include_str!("float_parity.json");

#[derive(Deserialize)]
struct Fixtures {
    measure: Vec<Case<MeasureIn>>,
    #[serde(rename = "moveCost")]
    move_cost: Vec<Case<MoveIn>>,
}

#[derive(Deserialize)]
struct Case<I> {
    name: String,
    input: I,
    expected: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitsIn {
    per_cell: f32,
    label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WallIn {
    id: String,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

#[derive(Deserialize)]
struct Placed {
    x: f32,
    y: f32,
    #[serde(default = "one")]
    footprint: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeasureIn {
    grid_type: String,
    grid_size: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    units: UnitsIn,
    #[serde(default)]
    walls: Vec<WallIn>,
    from: Placed,
    to: Placed,
    #[serde(default)]
    reach: Reach,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveIn {
    grid_type: String,
    grid_size: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    units: UnitsIn,
    #[serde(default = "one")]
    footprint: f32,
    #[serde(default)]
    speed_kind: Option<String>,
    from: [f32; 2],
    #[serde(default)]
    route: Vec<[f32; 2]>,
    to: [f32; 2],
}

fn fixtures() -> Fixtures {
    serde_json::from_str(FIXTURES).expect("float_parity.json parses")
}

fn measured(m: &MeasureIn) -> Measured {
    let grid = scene_grid(&m.grid_type, m.grid_size, m.width, m.height);
    let units = GridUnits::new(m.units.per_cell, m.units.label.clone());
    let mut walls = WallSet::default();
    for w in &m.walls {
        walls.upsert(Wall {
            id: w.id.clone(),
            x1: w.x1,
            y1: w.y1,
            x2: w.x2,
            y2: w.y2,
            blocks_vision: true,
            blocks_movement: true,
            door_state: DoorState::from_str_loose(""),
            locked: false,
            secret: false,
        });
    }
    let at = |p: &Placed| (Vec2::new(p.x, p.y), Footprint::new(p.footprint));
    measure(&grid, &units, &walls, at(&m.from), at(&m.to), &m.reach)
}

fn moved(m: &MoveIn) -> f64 {
    let grid = scene_grid(&m.grid_type, m.grid_size, m.width, m.height);
    let units = GridUnits::new(m.units.per_cell, m.units.label.clone());
    let point = |p: &[f32; 2]| Vec2::new(p[0], p[1]);
    let route: Vec<Vec2> = m.route.iter().map(point).collect();
    move_cost(
        &grid,
        &units,
        Footprint::new(m.footprint),
        m.speed_kind.as_deref().unwrap_or("walk"),
        point(&m.from),
        &route,
        point(&m.to),
    )
}

/// The exact bits of a JSON number. serde_json reads an f64 back to the same
/// bits it was written from, so equal bits here is equal bits on the host.
fn bits(value: &Value) -> u64 {
    value.as_f64().expect("a number").to_bits()
}

#[test]
fn reach_measures_to_the_recorded_bit() {
    for case in fixtures().measure {
        let got = measured(&case.input);
        let distance = got.distance.expect("a distance");
        assert_eq!(
            distance.to_bits(),
            bits(&case.expected["distance"]),
            "{}: distance {distance} against {}",
            case.name,
            case.expected["distance"],
        );
        assert_eq!(
            serde_json::to_value(&got.flags).unwrap(),
            case.expected["flags"],
            "{}: flags",
            case.name,
        );
    }
}

#[test]
fn a_move_costs_the_recorded_bit() {
    for case in fixtures().move_cost {
        let got = moved(&case.input);
        assert_eq!(
            got.to_bits(),
            bits(&case.expected),
            "{}: cost {got} against {}",
            case.name,
            case.expected,
        );
    }
}

/// Not every case may land on a round number, or the fixture proves nothing
/// about fused arithmetic: a value like 15.0 survives most reorderings.
#[test]
fn the_fixtures_include_results_that_are_not_round() {
    let f = fixtures();
    let awkward = f
        .move_cost
        .iter()
        .map(|c| c.expected.as_f64().unwrap())
        .chain(
            f.measure
                .iter()
                .map(|c| c.expected["distance"].as_f64().unwrap()),
        )
        .filter(|v| v.fract() != 0.0)
        .count();
    assert!(awkward >= 3, "only {awkward} non-integral results");
}
