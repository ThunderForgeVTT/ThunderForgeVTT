//! Where a gesture's walls go when they walk the grid (spec 077).
//!
//! With snapping on, a wall drag no longer draws one wall between two
//! points: it lays walls along grid lines, one per cell edge, so that any
//! one edge can later be clicked into a door (FR-005, FR-017). The three
//! gestures the walls tool offers all reduce to the same two ideas:
//!
//! - A **walk** between two lattice vertices (`grid_walk`): on a square grid
//!   an L, the longer axis first (FR-006); on a hex grid the shortest path
//!   along hex edges (FR-007).
//! - A **boundary** of a set of cells (`boundary_edges`): every cell edge
//!   with an admitted cell on one side and not on the other. A box is the
//!   boundary of the cells inside a rectangle (FR-012); a circle is the
//!   boundary of the cells inside a disk — the rasterised circle (FR-013).
//!   The same function serves square and hex, because it asks the grid for
//!   outlines and neighbours rather than knowing either shape.
//!
//! Every endpoint is drawn from one [`VertexPool`], so two walls that meet
//! at a corner hold the *same* `Vec2`, bit for bit, rather than two values a
//! few ulps apart: the vision pass finds a room closed only when its corners
//! agree exactly, which is also why `wall::room_segments` was careful.
//!
//! Free (unsnapped) geometry lives elsewhere: `wall::room_segments` for a
//! rectangle, [`circle_chords`] here for a polygon. Nothing in this module
//! decides *whether* to snap; that is `snapping::SnapRule`'s.

use std::collections::HashSet;
use std::f32::consts::TAU;

use glam::Vec2;

use crate::grid::{Cell, GridKind, GridSpec};

/// One wall, from one point to another.
pub type Segment = (Vec2, Vec2);

/// Two vertices closer than this are the same vertex. Cell outlines are
/// computed per cell, so a shared corner arrives from each neighbour with
/// its own rounding; a thousandth of a world unit is far below anything a
/// grid distinguishes and far above any float drift.
const VERTEX_MERGE: f32 = 1e-3;

/// Vertices, merged: the first value seen for a corner is the value every
/// later wall at that corner gets.
#[derive(Default)]
struct VertexPool(Vec<Vec2>);

impl VertexPool {
    fn canonical(&mut self, point: Vec2) -> Vec2 {
        if let Some(known) = self
            .0
            .iter()
            .find(|known| known.distance_squared(point) <= VERTEX_MERGE * VERTEX_MERGE)
        {
            return *known;
        }
        self.0.push(point);
        point
    }
}

/// The walls along grid lines from vertex `a` to vertex `b`, one per cell
/// edge. Both points are taken to be lattice vertices already (the caller
/// has snapped them); the walk starts and ends on them exactly.
///
/// Square: the longer axis first, then the shorter, so a drag that is nearly
/// horizontal reads as a horizontal wall with a short return, not a
/// staircase. Hex: the shortest path through the hex vertex graph, found by
/// a greedy walk that is exact on a hex lattice for the distances a drag
/// covers; it stops when no neighbouring vertex is nearer the target.
///
/// A gridless grid, or `a == b`, yields nothing: there is no line to walk.
pub fn grid_walk(grid: &GridSpec, a: Vec2, b: Vec2) -> Vec<Segment> {
    if a.distance_squared(b) <= VERTEX_MERGE * VERTEX_MERGE {
        return Vec::new();
    }
    match grid.kind {
        GridKind::Gridless => Vec::new(),
        GridKind::Square => square_walk(grid, a, b),
        GridKind::HexPointyTop | GridKind::HexFlatTop => hex_walk(grid, a, b),
    }
}

fn square_walk(grid: &GridSpec, a: Vec2, b: Vec2) -> Vec<Segment> {
    let size = square_size(grid);
    // Integer lattice coordinates, so every point is rebuilt from integers
    // and consecutive walls share their endpoint exactly.
    let index = |p: Vec2| {
        let local = (p - grid.origin) / size;
        (local.x.round() as i32, local.y.round() as i32)
    };
    let point = |ix: i32, iy: i32| grid.origin + Vec2::new(ix as f32 * size, iy as f32 * size);
    let (ax, ay) = index(a);
    let (bx, by) = index(b);
    let dx = bx - ax;
    let dy = by - ay;
    if dx == 0 && dy == 0 {
        return Vec::new();
    }

    let mut path = vec![(ax, ay)];
    let push_run = |path: &mut Vec<(i32, i32)>, horizontal: bool| {
        let (last_x, last_y) = *path.last().expect("path starts with a point");
        if horizontal {
            let step = dx.signum();
            for i in 1..=dx.abs() {
                path.push((last_x + i * step, last_y));
            }
        } else {
            let step = dy.signum();
            for i in 1..=dy.abs() {
                path.push((last_x, last_y + i * step));
            }
        }
    };
    // The longer axis first (FR-006); a tie goes horizontal.
    let horizontal_first = dx.abs() >= dy.abs();
    push_run(&mut path, horizontal_first);
    push_run(&mut path, !horizontal_first);

    path.windows(2)
        .map(|pair| (point(pair[0].0, pair[0].1), point(pair[1].0, pair[1].1)))
        .collect()
}

fn hex_walk(grid: &GridSpec, a: Vec2, b: Vec2) -> Vec<Segment> {
    let mut pool = VertexPool::default();
    let target = pool.canonical(b);
    let mut current = pool.canonical(a);
    let mut path = vec![current];
    // A hex walk never needs more edges than this; the cap only guards
    // against a target that is not on the lattice at all.
    let cap = (a.distance(b) / grid.safe_size() * 4.0).ceil() as usize + 8;
    for _ in 0..cap {
        if current.distance_squared(target) <= VERTEX_MERGE * VERTEX_MERGE {
            break;
        }
        let next = hex_vertex_neighbours(grid, current)
            .into_iter()
            .map(|n| pool.canonical(n))
            .filter(|n| n.distance_squared(current) > VERTEX_MERGE * VERTEX_MERGE)
            .min_by(|p, q| {
                p.distance_squared(target)
                    .total_cmp(&q.distance_squared(target))
            });
        match next {
            Some(next) if next.distance_squared(target) < current.distance_squared(target) => {
                path.push(next);
                current = next;
            }
            _ => break,
        }
    }
    path.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// The vertices one hex edge away from `vertex`: in each cell that has it
/// as a corner, the two outline points beside it. A vertex meets three
/// cells, found by stepping a little towards each of six directions.
fn hex_vertex_neighbours(grid: &GridSpec, vertex: Vec2) -> Vec<Vec2> {
    let probe = grid.safe_size() * 0.25;
    let mut cells: Vec<Cell> = Vec::with_capacity(3);
    for i in 0..6 {
        let angle = i as f32 * TAU / 6.0 + TAU / 12.0;
        let cell = grid.world_to_cell(vertex + Vec2::new(angle.cos(), angle.sin()) * probe);
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    let mut neighbours = Vec::with_capacity(6);
    for cell in cells {
        let outline = grid.cell_outline(cell);
        let ring = &outline[..outline.len().saturating_sub(1)];
        let Some(at) = ring
            .iter()
            .position(|p| p.distance_squared(vertex) <= (probe * 0.5).powi(2))
        else {
            continue;
        };
        let n = ring.len();
        neighbours.push(ring[(at + n - 1) % n]);
        neighbours.push(ring[(at + 1) % n]);
    }
    neighbours
}

/// Every cell edge with an admitted cell on one side and not on the other:
/// the walls that fence the cells `inside` admits (by centre) within the
/// world rectangle `min`..`max`. Square or hex alike; the grid supplies the
/// outlines and the neighbour across each edge.
///
/// Edges come out once each, as the admitted cell's edge, with endpoints
/// merged through one pool so neighbouring edges share corners exactly.
pub fn boundary_edges(
    grid: &GridSpec,
    min: Vec2,
    max: Vec2,
    inside: impl Fn(Vec2) -> bool,
) -> Vec<Segment> {
    if grid.kind == GridKind::Gridless {
        return Vec::new();
    }
    let admitted: HashSet<Cell> = cells_touching(grid, min, max)
        .into_iter()
        .filter(|cell| inside(grid.cell_center(*cell)))
        .collect();

    let mut pool = VertexPool::default();
    let mut edges = Vec::new();
    // Deterministic order: by cell, so a test (and an undo) sees the same
    // list for the same gesture.
    let mut ordered: Vec<Cell> = admitted.iter().copied().collect();
    ordered.sort_by_key(|c| (c.r, c.q));
    for cell in ordered {
        let centre = grid.cell_center(cell);
        let outline = grid.cell_outline(cell);
        for pair in outline.windows(2) {
            let mid = (pair[0] + pair[1]) / 2.0;
            let outward = (mid - centre).normalize_or_zero();
            let across = grid.world_to_cell(mid + outward * grid.safe_size() * 0.25);
            if admitted.contains(&across) {
                continue;
            }
            edges.push((pool.canonical(pair[0]), pool.canonical(pair[1])));
        }
    }
    edges
}

/// Every cell whose centre could lie in `min`..`max`, found by sampling the
/// rectangle (grown by a cell) at half a cell's pitch: no square or hex cell
/// is narrower than that, so none is missed.
fn cells_touching(grid: &GridSpec, min: Vec2, max: Vec2) -> Vec<Cell> {
    let size = grid.safe_size();
    let step = size / 2.0;
    let from = min - Vec2::splat(size);
    let to = max + Vec2::splat(size);
    let mut seen = HashSet::new();
    let mut cells = Vec::new();
    let mut y = from.y;
    while y <= to.y {
        let mut x = from.x;
        while x <= to.x {
            let cell = grid.world_to_cell(Vec2::new(x, y));
            if seen.insert(cell) {
                cells.push(cell);
            }
            x += step;
        }
        y += step;
    }
    cells
}

/// The perimeter of the rectangle `a`..`b`, one wall per cell edge (FR-012).
/// On a square grid with the corners on vertices this is exactly the
/// rectangle; on a hex grid it is the hex cells whose centres lie inside.
pub fn box_edges(grid: &GridSpec, a: Vec2, b: Vec2) -> Vec<Segment> {
    let min = a.min(b);
    let max = a.max(b);
    if max.x - min.x <= VERTEX_MERGE || max.y - min.y <= VERTEX_MERGE {
        return Vec::new();
    }
    boundary_edges(grid, min, max, |centre| {
        centre.x > min.x && centre.x < max.x && centre.y > min.y && centre.y < max.y
    })
}

/// The closed ring of cell edges nearest a circle of `radius` about
/// `centre`: the boundary of the cells whose centres lie within it
/// (FR-013). A radius that admits no cell yields nothing (FR-014).
pub fn circle_edges(grid: &GridSpec, centre: Vec2, radius: f32) -> Vec<Segment> {
    if radius.is_nan() || radius <= VERTEX_MERGE {
        return Vec::new();
    }
    let reach = Vec2::splat(radius);
    boundary_edges(grid, centre - reach, centre + reach, |cell_centre| {
        cell_centre.distance_squared(centre) <= radius * radius
    })
}

/// A circle as `chords` straight walls, closed exactly at its first point:
/// the free (unsnapped) circle. Fewer than three chords is not a shape and
/// yields nothing, as does a radius of nothing (FR-014).
pub fn circle_chords(centre: Vec2, radius: f32, chords: usize) -> Vec<Segment> {
    if chords < 3 || radius.is_nan() || radius <= VERTEX_MERGE {
        return Vec::new();
    }
    let points: Vec<Vec2> = (0..chords)
        .map(|i| {
            let angle = i as f32 / chords as f32 * TAU;
            centre + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect();
    (0..chords)
        .map(|i| (points[i], points[(i + 1) % chords]))
        .collect()
}

/// How many chords a free circle of `radius` needs to look round when each
/// chord is about `chord_world` long on screen: at least twelve, never more
/// than a wall budget a scene can carry.
pub fn chords_for(radius: f32, chord_world: f32) -> usize {
    let wanted = (TAU * radius / chord_world.max(f32::EPSILON)).ceil() as usize;
    wanted.clamp(12, 72)
}

fn square_size(grid: &GridSpec) -> f32 {
    grid.safe_size()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(kind: GridKind, size: f32) -> GridSpec {
        GridSpec {
            kind,
            size,
            origin: Vec2::ZERO,
        }
    }

    /// Every wall is one cell edge long and the chain shares endpoints exactly.
    fn assert_chain(segments: &[Segment], edge: f32) {
        for (i, (from, to)) in segments.iter().enumerate() {
            assert!(
                (from.distance(*to) - edge).abs() < 1e-3,
                "wall {i} is {} long, not {edge}",
                from.distance(*to)
            );
            if i > 0 {
                assert_eq!(
                    segments[i - 1].1,
                    *from,
                    "wall {i} does not start where {} ended",
                    i - 1
                );
            }
        }
    }

    #[test]
    fn a_square_walk_along_one_line_is_one_wall_per_cell() {
        let g = grid(GridKind::Square, 50.0);
        let walls = grid_walk(&g, Vec2::new(0.0, 0.0), Vec2::new(150.0, 0.0));
        assert_eq!(walls.len(), 3);
        assert_chain(&walls, 50.0);
        assert_eq!(walls[0].0, Vec2::ZERO);
        assert_eq!(walls[2].1, Vec2::new(150.0, 0.0));
    }

    #[test]
    fn a_square_walk_off_the_line_is_an_l_with_the_long_leg_first() {
        let g = grid(GridKind::Square, 50.0);
        let walls = grid_walk(&g, Vec2::new(0.0, 0.0), Vec2::new(150.0, -100.0));
        assert_eq!(walls.len(), 5);
        assert_chain(&walls, 50.0);
        // Three horizontal, then two vertical.
        for w in &walls[..3] {
            assert_eq!(w.0.y, w.1.y);
        }
        for w in &walls[3..] {
            assert_eq!(w.0.x, w.1.x);
        }
        assert_eq!(walls[4].1, Vec2::new(150.0, -100.0));

        // Taller than wide: vertical first.
        let tall = grid_walk(&g, Vec2::new(0.0, 0.0), Vec2::new(50.0, 150.0));
        assert_eq!(tall.len(), 4);
        assert_eq!(tall[0].0.x, tall[0].1.x);
    }

    #[test]
    fn a_hex_walk_follows_hex_edges_and_arrives() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            let g = grid(kind, 60.0);
            let edge = g.cell_outline(Cell::new(0, 0));
            let a = edge[0];
            let far = g.cell_outline(Cell::new(3, -1))[2];
            let walls = grid_walk(&g, a, far);
            assert!(!walls.is_empty(), "{kind:?}: no walk");
            assert_chain(&walls, edge[0].distance(edge[1]));
            assert_eq!(walls[0].0, a, "{kind:?}");
            assert!(
                walls.last().unwrap().1.distance(far) < 1e-3,
                "{kind:?}: ended short"
            );
        }
    }

    #[test]
    fn a_box_is_the_rectangle_one_wall_per_edge() {
        let g = grid(GridKind::Square, 50.0);
        // 3 by 2 cells: ten edges.
        let walls = box_edges(&g, Vec2::new(0.0, 0.0), Vec2::new(150.0, 100.0));
        assert_eq!(walls.len(), 10);
        for (from, to) in &walls {
            assert!((from.distance(*to) - 50.0).abs() < 1e-3);
            let on_edge = |p: &Vec2| p.x == 0.0 || p.x == 150.0 || p.y == 0.0 || p.y == 100.0;
            assert!(
                on_edge(from) && on_edge(to),
                "{from:?}-{to:?} is not on the perimeter"
            );
        }
        assert!(
            box_edges(&g, Vec2::ZERO, Vec2::new(150.0, 0.0)).is_empty(),
            "degenerate"
        );
    }

    #[test]
    fn boundary_edges_share_their_corners_exactly() {
        for kind in [
            GridKind::Square,
            GridKind::HexPointyTop,
            GridKind::HexFlatTop,
        ] {
            let g = grid(kind, 40.0);
            let walls = circle_edges(&g, Vec2::new(17.0, -9.0), 130.0);
            assert!(walls.len() > 8, "{kind:?}: {} walls", walls.len());
            // Closed: every endpoint is used exactly twice.
            let mut uses: Vec<(Vec2, usize)> = Vec::new();
            for (from, to) in &walls {
                for p in [from, to] {
                    match uses.iter_mut().find(|(q, _)| q == p) {
                        Some(entry) => entry.1 += 1,
                        None => uses.push((*p, 1)),
                    }
                }
            }
            assert!(
                uses.iter().all(|(_, n)| *n == 2),
                "{kind:?}: a corner is not shared by exactly two walls: {:?}",
                uses.iter().filter(|(_, n)| *n != 2).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn a_circle_too_small_for_a_cell_is_nothing() {
        let g = grid(GridKind::Square, 50.0);
        assert!(circle_edges(&g, Vec2::new(10.0, 10.0), 0.0).is_empty());
        assert!(
            circle_edges(&g, Vec2::new(0.0, 0.0), 5.0).is_empty(),
            "no centre within 5 of a corner"
        );
        assert_eq!(
            circle_edges(&g, Vec2::new(25.0, 25.0), 5.0).len(),
            4,
            "one cell"
        );
    }

    #[test]
    fn free_chords_close_on_the_first_point() {
        let chords = circle_chords(Vec2::new(5.0, 5.0), 100.0, 16);
        assert_eq!(chords.len(), 16);
        assert_eq!(chords[15].1, chords[0].0);
        assert!(circle_chords(Vec2::ZERO, 100.0, 2).is_empty());
        assert!(circle_chords(Vec2::ZERO, 0.0, 16).is_empty());
        assert_eq!(chords_for(1.0, 12.0), 12);
        assert_eq!(chords_for(1e6, 12.0), 72);
    }

    #[test]
    fn gridless_walks_nowhere() {
        let g = grid(GridKind::Gridless, 50.0);
        assert!(grid_walk(&g, Vec2::ZERO, Vec2::new(100.0, 0.0)).is_empty());
        assert!(box_edges(&g, Vec2::ZERO, Vec2::new(100.0, 100.0)).is_empty());
    }
}
