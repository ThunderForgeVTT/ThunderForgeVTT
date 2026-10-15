# Implementation Plan: Paths Around Walls

**Branch**: `097-engine-pathing` | **Date**: 2026-10-15 | **Spec**: [spec.md](./spec.md)

## Summary

- `crates/thunderforge-canvas-core/src/pathfinding.rs` holds:
  - A* over `Cell`s on the grid's own lattice, using `world_to_cell` and
    `cell_center`;
  - a `BinaryHeap` with an `Ord` wrapper over the f-score, with ties
    broken on the h-score and then (r, q);
  - `HashMap` g-scores.
- Smoothing is greedy string-pulling: from each kept point, jump to the
  farthest later point it sees.
- In the engine:
  - a `PathingSnapshot` mutex is refreshed by a system when `WallSet` or
    `SceneGrid` changes;
  - a `plan_path` export in `lib.rs` (the same shape as `token_footprints`).

## Technical Context

- Uses canvas-core's `glam` and `serde`. No new dependencies.
- `movement_blocked_by` counts touching as crossing. That is why the
  smoothing tests sight between waypoints with the same function: a line
  grazing a wall's end is refused, and the corner is kept.
- `blocked` is a `HashSet<Cell>`. The goal cell is always allowed, so a
  host can ask for a path to stand beside a token.

## Files

- `crates/thunderforge-canvas-core/src/pathfinding.rs` and
  `pathfinding_tests.rs`, plus `lib.rs`.
- `crates/thunderforge-engine/src/plugins/pathing.rs`: the snapshot system
  and the export body.
- `crates/thunderforge-engine/src/lib.rs`: `#[wasm_bindgen] pub fn plan_path`.
- `apps/landing/src/engine/loadEngine.ts`: the `Engine` type gains
  `plan_path?`.
