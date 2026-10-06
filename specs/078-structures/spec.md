# Feature Specification: Structures

**Feature Branch**: `078-structures`
**Created**: 2026-10-06
**Status**: FR-001 to FR-004 implemented 2026-10-06 (the join); the rest is a draft awaiting the owner
**Input**: The owner, answering spec 077's T-junction question on 2026-10-06: "drag across forms an intersection on a corner, that's fine, basically if someone's creating 4 rooms that's super common. Another one is if one wall gets ended on an existing wall that creates a join, really, so we can build the concept of a structure."

## Why

Spec 077 made a wall land on another wall's *end*, so a room drawn against
a room closes without a seam. It left the *middle* of a wall alone: a wall
drawn up to the side of a long one stopped at the nearest grid corner, a
hair short or a hair through, and the two were never one thing. Four rooms
sharing walls is the commonest map there is, and a Game Master drawing
them wants the walls they draw to meet.

The owner's picture is a step further than meeting. A wall that ends on
another wall *joins* it, and joined walls are a structure: a building, a
room, a corridor, that can later be named, moved, copied or deleted as one.
This spec builds the join now, because snapping made it one small rule
away, and writes down what a structure is for so the join is built with it
in mind.

## What exists

Counted on 2026-10-06:

- `SnapRule::vertex_among` (spec 077) lands a wall end on another wall's
  endpoint or a grid corner, walls ranking above corners.
- `WallSet` is a flat list of walls with a bounded undo stack of
  `WallEdit`s; nothing groups walls.
- `planned_walls` (`systems/wall_draw.rs`) turns a segment drag into a walk
  along the grid; `grid_walk` rounds both ends to the lattice.

## Requirements

### The join

- **FR-001** With snapping on, a segment wall whose end is released within
  the snap radius of the *middle* of an existing wall MUST end exactly on
  that wall, and that wall MUST be split there: the original keeps its id
  and is shortened to the join; the rest becomes a new wall with the same
  flags. Both ends of the drawn wall are treated this way, and so are the
  first and last point of a click-click-Enter chain (the chain's clicks
  land on the wall at once; the split waits for Enter, so Escape leaves the
  wall whole).
- **FR-002** A wall's own ends are not joins — two walls meeting at a corner
  already meet — and a door is never split: a door is one opening, and a
  wall aimed at its middle lands on the nearer end as spec 077 has it.
- **FR-003** The raw pointer decides whether a join is near, not the grid
  corner it would otherwise snap to; and the walk along the grid MUST reach
  the join exactly, with a short free wall from the last lattice vertex when
  the join is off the lattice. (This also closes a gap in spec 077: a wall
  snapped to another wall's off-lattice end used to be rounded away by the
  walk.)
- **FR-004** The preview MUST show the landing (FR-006 of spec 077 holds).
  Undo MUST take the drawn walls back first and un-split on the next press.
  Proof: core tests for `join_target`/`split_at`, an engine test for the
  join and its undo, and `wall-snapping.spec.ts`'s last step, which reads
  the shortened wall and the remainder back from the server.

### Crossings

- **FR-005** A drag that crosses an existing wall MUST NOT split either
  wall. With snapping on both lie on cell edges and already share the
  corner they cross at (the owner's four rooms); with it off, two free walls
  may cross without meeting, and that is what the Game Master drew.

### Structures (draft, for the owner)

- **FR-006** Walls that share an endpoint, or were joined under FR-001, form
  a *structure*. A structure is derived from the walls' geometry, not
  stored: moving one wall's endpoint off its neighbours leaves the
  structure.
- **FR-007** Selecting a wall offers "Select structure", which selects every
  wall in its structure; delete, and spec 073's move, then act on all of
  them.
- **FR-008** A structure can be named; the name shows when a wall of it is
  selected and in the walls panel, and is what a later spec exports as a
  "building" with its doors and lights.

## Success Criteria

- **SC-001** A Game Master draws a long wall, then two walls ending on it,
  and the server holds five walls: the original in three pieces and the
  two new ones, every end meeting another. (`wall-snapping.spec.ts`.)
- **SC-002** The full web e2e suite and the demo e2e stay green.

## Open questions for the owner

- FR-006 to FR-008 are my reading of "the concept of a structure". Is a
  structure the thing a later spec moves and copies as a building, or is
  the join itself what was wanted and the rest can wait?
- Should a wall ending on a wall also join with snapping *off*? Today it
  does not: off means the point goes exactly where it was put.
- Should joining split the *drawn* wall too when it is the one being
  crossed in the middle (a wall drawn through an existing wall's end)?
  Today only the existing wall splits, and only at the drawn wall's ends.

## What this spec does not do

- It does not store structures, name them, or move them (FR-006 to FR-008
  are a draft until the owner answers).
- It does not change crossings (FR-005 is what already happens).
- It does not join box or circle walls to what they are drawn against;
  their corners snap as spec 077 has it.
