# Feature Specification: Paths Around Walls

**Feature Branch**: `097-engine-pathing`
**Created**: 2026-10-15
**Status**: Draft
**Input**: The landing's walk-along (spec 098): "the token paths around
everything on the map ... pathing in the engine which should be full specs."

## Overview

The engine can say whether a move crosses a wall (`movement_blocked_by`,
`path_blocked_by`, spec 045), but it cannot find a way around one. Every
route the engine has drawn so far was given to it, cell by cell, by a
person dragging or pressing keys.

This spec adds the search: from here to there on a square grid, round the
walls and closed doors the engine knows, the shortest way a token could
walk it. It is the same rule that refuses moves, so a planned path is
never one the engine would then refuse.

The core lives in `thunderforge-canvas-core`, with no Bevy. The server
can use it later to check or suggest moves, and the engine exposes it to
its host.

## User Stories

### US1: A path round a wall (P1)

The host asks for a path from A to B. A wall stands between them, with a
gap at one end. The path goes through the gap, and no leg of it crosses a
wall.

### US2: Doors (P1)

A closed or locked door blocks the path, exactly as it blocks movement. An
open door does not. With the only door shut, there is no path, and the
answer says so rather than returning a straight line.

### US3: A path that looks walked (P2)

The answer is the fewest waypoints that stay clear of walls. That means
straight runs across open floor, not a staircase of cell centres, so a
token walking it moves the way a person would.

### US4: Bounded (P2)

The search is bounded by a rectangle of cells and a node budget. An
unreachable goal on an unbounded plane never hangs the frame.

## Requirements

- **FR-001**: `canvas_core::pathfinding::find_path(grid, from, to, walls,
  options)` returns `Some(waypoints)` in world space, starting at `from` and
  ending at `to`, or `None`.
- **FR-002**: Square grids only. A gridless or hex grid returns `None`
  (hex is a follow-up).
- **FR-003**: Movement is 8-connected. A diagonal step is allowed only when
  both orthogonal steps beside it are also clear. That means no squeezing
  between two walls that meet at a corner, which is FR-016 of 045.
- **FR-004**: A step is clear when `movement_blocked_by(centre, centre',
  walls)` is `None`. The legs from `from` to its cell's centre, and from
  the goal cell's centre to `to`, are judged the same way.
- **FR-005**: Costs are octile: 1 for orthogonal, √2 for diagonal. The
  heuristic is octile distance, so the path is shortest by that measure.
- **FR-006**: Options:
  - `bounds` (a min and max cell, inclusive);
  - `blocked` (cells no step may enter, such as other tokens);
  - `max_nodes` (default 20 000).
  Exceeding the budget returns `None`.
- **FR-007**: Smoothing removes every waypoint whose neighbours see each
  other with no wall between, judged by `movement_blocked_by`. It never
  crosses a blocked cell. The smoothed path passes `path_blocked_by` with
  no wall.
- **FR-008**: The engine keeps a snapshot of its walls and grid. A
  `plan_path(json)` wasm export takes `{from:{x,y}, to:{x,y},
  bounds?:{min:{q,r},max:{q,r}}, blocked?:[{q,r}], maxNodes?}`. It returns
  `{"path":[{x,y},…]}` or `{"path":null}`.
- **FR-009**: Deterministic. Ties are broken by a fixed order, so the same
  question always gets the same path.

## Success Criteria

- **SC-001**: canvas-core unit tests cover:
  - an open plane gives a straight line of two points;
  - a wall with a gap;
  - a shut door gives `None`;
  - an open door;
  - the corner squeeze is refused;
  - the budget is honoured;
  - bounds;
  - blocked cells;
  - the same answer twice.
- **SC-002**: A 200×40 cell search with a few dozen walls takes under 5 ms
  in a release wasm build on the desktop e2e machine.
- **SC-003**: The landing (098) walks its hero out of the tavern door and
  round the dungeon with paths from `plan_path`. An e2e spec asserts the
  hero never stands inside a wall's room unless it came in by its door.

## Out of scope

- Hex grids, token footprints larger than one cell, and difficult terrain
  costs.
- Using it in the web client's movement UI (a later spec can offer "walk
  there").
