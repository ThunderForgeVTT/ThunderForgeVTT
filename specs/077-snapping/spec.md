# Feature Specification: Snapping

**Feature Branch**: `077-snapping`
**Created**: 2026-10-06
**Status**: Implemented 2026-10-06 (FR-024 demo e2e and SC-004 measurement pending)
**Input**: The owner, running the demo on 2026-10-06: "one thing we're missing is snapping like building walls on the gridlines or lights, and using the S key or a button to turn snapping on or off … for lights I like it snaps to squares but I want the ability to snap to walls or corners too, and for wall building being able to chase gridlines would be huge, and we could add quick tools like box or circle for walls … and we could really improve it by having wall segments be each straight on the grid when snapping, allowing you to click and turn one into a door for faster room building."

## Why

Rooms are most of what a Game Master draws. Today a room is a chain of
drags, each endpoint snapped to a grid corner, with a door added afterwards
by selecting a wall and changing its door state (spec 001 T021, spec 071).
A wall drawn diagonally across cells snaps its two ends and cuts through
the cells between; a wall long enough to want a door in the middle has to
be split by hand; a light wanting to sit in a doorway or against a wall
can only sit in the middle of a cell. And snapping cannot be turned off at
all, though the engine has had the switch since spec 001 FR-024: nothing
in the page ever sends it.

The owner's picture is the one every map editor converges on. Snapping is
a mode with a key and a visible button. A wall drawn with snapping on runs
along grid lines, one wall per cell edge, so any one edge can become a door
with a click. A box and a circle lay down a room in one gesture. A light
snaps to the nearest useful thing — a cell, a wall, a corner — not only to
a cell.

## What exists

Counted on 2026-10-06:

- `thunderforge-canvas-core/src/snapping.rs`: `SnapRule`, the one policy
  layer every placement asks. `token` (footprint), `cell` (centre) and
  `vertex` (grid corner), each honouring hex and square and passing a
  position through untouched when snapping is off or the scene is gridless.
- `resources::GridSnapEnabled`, on by default, ANDed with a token's own
  `TokenGridBehaviour::snap`. The engine accepts `set_grid_snap` (`sdk.rs`,
  `ExternalCommand::SetGridSnap`). **No code in `apps/web` sends it.** The
  switch exists and has no handle.
- `resources::WallPrimitive`: `Segment` (default; a drag, or a click-chain
  ended by Enter), `Room` (four walls between two corners, via
  `canvas_core::wall::room_segments`, sharing corner points exactly) and
  `Door` (one wall, already a closed door). `set_wall_primitive` is exported
  to the page (`engine/bevy/index.ts`). **Nothing in `WallTool.tsx` offers a
  choice**; the tool draws segments and edits the selected wall's vision,
  movement and door state. `Room` and `Door` are unreachable from the UI.
- Wall endpoints snap with `SnapRule::vertex`, both when created and while
  dragged (`systems/wall.rs`). They do not snap to other walls' endpoints,
  so two rooms drawn to adjoin meet only if both drags landed on the same
  corner.
- Lights snap with `SnapRule::cell` (`systems/lighting.rs`), and the preview
  rings are drawn from the same rule, so what is promised is what lands.
- `S` is taken while a token is selected: WASD walks it (`120284e1`).
- Spec 071 gives a wall's door state a right-click; spec 073 does the same
  for lights and drawings. Both are Draft.

## Decisions already made

- **Snapping is a mode, with one key and one button.** The button lives in
  the tool rail, shows the state, and is the same control in every
  authoring tool. The key is `S`, and it toggles snapping only while an
  authoring tool is open (walls, lights, shapes, placement); with the select
  tool open `S` keeps walking the token, as `120284e1` made it. One state
  for the whole board, not one per tool: a Game Master who turns it off to
  nudge a light does not find walls still snapping.
- **The state is the Game Master's, for the session.** It is not scene data
  and is not sent to the server; it lives in the engine and is on again
  after a reload, as it is today. A tooltip on the button names the key.
- **Snapped walls walk the grid, one wall per cell edge.** With snapping on,
  a wall drag from one corner to another is laid down along grid lines, and
  each cell edge it covers is its own wall. A drag that is not along a line
  is an L: the long leg first, then the short. Every edge is then a thing a
  click can turn into a door, which is the point. With snapping off a drag
  is one wall between two free points, as today; nothing a Game Master can
  do today is taken away.
- **Walls snap to walls.** With snapping on, a wall endpoint near an
  existing wall's endpoint lands on it, ahead of the grid corner, so a room
  drawn against another closes without a seam. Near is measured in screen
  pixels, not world units, so it is the same gesture at every zoom.
- **Lights snap to the nearest useful thing.** With snapping on, a light
  lands on whichever is nearest within the same screen radius: a wall's
  endpoint (a corner), the nearest point on a wall, or, when neither is
  near, the cell centre as today. A torch in a sconce, a lantern over a
  door, a brazier in the middle of the room: each is one click.
- **Box and circle are quick tools, not modes of the drag.** The walls tool
  offers Segment (default), Box and Circle; the engine's `Room` primitive is
  Box. Each lays its walls along the grid when snapping is on — a box is
  one wall per cell edge on its perimeter, a circle is the ring of cell
  edges nearest a true circle through the drag's radius — and as free
  geometry when it is off (a rectangle of four walls; a circle of as many
  chords as it needs to look round at the zoom it was drawn at).
- **A click makes a door.** In the walls tool, with the Door primitive
  chosen, clicking an existing wall turns that wall into a closed door
  rather than drawing a new one. Spec 071's right-click remains the way to
  change a door's state afterwards; this spec only shortens the first step.
- **The rule stays in `SnapRule`.** Wall-to-wall and light-to-wall snapping
  are new candidates `SnapRule` ranks, not arithmetic in a system. Every
  placement keeps asking the one function (spec 001 FR-006), and the hex
  grid keeps getting hex answers (FR-025).

## Requirements

### The switch

- **FR-001** The tool rail MUST show a snapping control whenever an
  authoring tool is open, reflecting the engine's `GridSnapEnabled`, and
  clicking it MUST toggle it.
- **FR-002** `S` MUST toggle snapping while an authoring tool is open, and
  MUST NOT while the select tool is open or a text field has focus.
- **FR-003** Turning snapping off MUST free every placement at once —
  walls, lights, shapes, token placement — and turning it on MUST restore
  each token's own setting rather than flattening them (spec 001 FR-024's
  AND, already in `SnapRule::for_target`).
- **FR-004** On a gridless scene the control MUST read as unavailable
  (`SnapRule::is_active` is already false there), and wall-to-wall and
  light-to-wall snapping MUST still work, because they need no grid.

### Walls along the grid

- **FR-005** With snapping on, a segment drag between two grid corners MUST
  produce walls along grid lines only, one wall per cell edge covered, with
  consecutive walls sharing their endpoint exactly.
- **FR-006** A drag whose corners do not share a grid line MUST produce an L
  of two runs, the longer axis first; the preview MUST draw the same L
  before the pointer is released, so what is promised is what lands.
- **FR-007** On a hex grid the walk MUST follow hex edges between the two
  nearest vertices, by the shortest edge path; a cell edge is a hex edge.
- **FR-008** With snapping off a drag MUST produce one wall between two free
  points, as today.
- **FR-009** With snapping on, a wall endpoint within the snap radius of an
  existing wall's endpoint MUST land on that endpoint, in preference to a
  grid corner; dragging an existing endpoint MUST obey the same rule.
- **FR-010** The snap radius MUST be a screen-space constant (pixels at the
  current zoom), shared by every candidate kind, named once.

### Box and circle

- **FR-011** The walls tool MUST offer Segment, Box, Circle and Door, with
  Segment the default and the choice shown in the tool's panel.
- **FR-012** Box MUST produce the perimeter of the dragged rectangle: with
  snapping on, one wall per cell edge along it, corners on grid corners;
  with snapping off, four walls.
- **FR-013** Circle MUST be drawn by dragging from centre to radius. With
  snapping on it MUST produce the closed ring of cell edges nearest the true
  circle (the rasterised circle), one wall per edge; with snapping off, a
  closed polygon of chords, closed exactly at its first point.
- **FR-014** A degenerate drag (zero width, height or radius) MUST produce
  nothing, as `room_segments` already does.
- **FR-015** Every wall a quick tool produces MUST arrive at the server as
  ordinary `create_wall` commands with the drawn defaults (blocks vision and
  movement, `doorState: none`), so undo, sync and permissions are the ones
  walls already have. One gesture MUST be one undo.

### A click makes a door

- **FR-016** With the Door primitive chosen, clicking an existing wall MUST
  set its door state to closed (and unlocked, shown) without creating a
  wall; clicking empty board MUST draw a one-edge door as today.
- **FR-017** A wall produced by FR-005, FR-012 or FR-013 MUST be exactly
  one cell edge long when snapping was on, so FR-016 makes a door of one
  edge and never of a whole side.

### Lights snap to walls

- **FR-018** With snapping on, a placed or dragged light MUST land on the
  nearest candidate within the snap radius, ranked: a wall endpoint, then
  the nearest point on a wall, then the cell centre; with nothing within
  the radius it MUST land on the cell centre as today.
- **FR-019** The light preview MUST show the chosen candidate before the
  click, from the same rule.
- **FR-020** A light on a wall MUST light both sides of it as the vision
  pass already decides for a light on a wall's line; this spec does not
  change what a wall occludes.

### Proof

- **FR-021** Core tests (`canvas-core`): the grid walk between two corners
  on a square grid, the L, the hex walk, the rasterised circle's closure,
  candidate ranking by distance and kind, and the snap radius applied at
  two zooms.
- **FR-022** Engine tests: `S` toggles only under an authoring tool; a snapped
  drag emits one `create_wall` per edge and one undo entry; a Door click on
  an existing wall emits an update and no create; a light within the radius
  of a wall endpoint lands on it.
- **FR-023** A web e2e on the full stack: the Game Master draws a 3×2 box
  with snapping on and the store holds ten walls each one cell long; clicks
  one into a door with the Door primitive; places a light that lands on the
  box's corner; presses `S` and draws a free diagonal that arrives as one
  wall.
- **FR-024** A demo e2e: on Grassy Path Ambush, a box of walls around the
  fighter, one edge made a door, and spec 076's probe reports the road
  outside the box unseen through the walls and seen through the open door.

## Success Criteria

- **SC-001** A Game Master can wall a 4×4 room with one door in three
  gestures: a box drag, a Door click, and nothing else.
- **SC-002** A light can be put in a doorway or on a corner in one click,
  with the preview showing where it will land.
- **SC-003** `S` and the rail button agree at all times, and the state
  survives switching between authoring tools.
- **SC-004** A 20×20 room drawn as cell edges (80 walls) keeps the frame
  time within the budget spec 028 measures today; if it does not, FR-005's
  one-per-edge rule is revisited before the spec is marked implemented.
- **SC-005** The full web e2e suite, `pnpm playtest --only=dungeon-crawl`
  and the demo e2e stay green.

## How it was built

- **Geometry in core** (`thunderforge-canvas-core/src/wall_layout.rs`):
  `grid_walk` (the L, long leg first), `box_edges`, `circle_edges` (the
  rasterised ring, closed), `circle_chords` with `chords_for(radius,
  chord_length)` for the free circle. `snapping.rs` gained
  `SnapRule::vertex_among` (a wall endpoint within the radius beats the grid
  corner) and `point_near_walls` (endpoint, then nearest point on a wall,
  then the cell), with `SNAP_RADIUS_PIXELS = 12` named once (FR-010).
- **One plan for preview and release** (`engine/systems/wall_draw.rs`):
  `planned_walls(primitive, rule, start, end, pixel)` is asked by the
  preview system while the button is held and by `handle_wall_input` on
  release, so FR-006 holds by construction. `emit_planned` sends one
  `create_wall` per segment and pushes one `WallEdit::Created` (FR-015);
  undo deletes by endpoints.
- **The switch** (`engine/systems/grid_snap.rs`, `GmToolRail/SnapToggle.tsx`):
  `S` toggles `GridSnapEnabled` under any authoring tool and is left to
  WASD under Select; the engine reports `grid_snap_changed` on every change,
  and the rail's button only subscribes and asks, so key and button cannot
  disagree (SC-003).
- **The walls tool** offers Segment, Box, Circle and Door as a radiogroup
  (`wall-primitive-*`), replacing a toggle that armed nothing. With Door
  chosen, a press within `WALL_SELECT_DISTANCE` of a wall sets it closed and
  emits `update_wall`; the drag never starts (FR-016).
- **Lights** ask `point_near_walls` in both the preview and the click, with
  the radius converted at the camera's zoom (FR-018, FR-019).
- **Proof**: core and engine unit tests (FR-021, FR-022);
  `e2e/wall-snapping.spec.ts` is FR-023 verbatim — ten cell-edge walls from
  a 3×2 box drag, a door by click, a light on the corner, `S` and a free
  diagonal. `drawn-walls-block.spec.ts` now turns snapping off first, since
  its four-wall room is the free case.
- **Not yet**: FR-024's demo e2e, and SC-004's 80-wall frame-time
  measurement against spec 028's budget.

## Open questions for the owner

- Should the quick tools lay walls as one wall per cell edge *always*, or
  only when snapping is on? The decision above is only when on; a free box
  is four walls.
- ~~Should wall-to-wall snapping also catch the *middle* of an existing wall
  (a T-junction), splitting it, or only its endpoints?~~ Answered by the
  owner 2026-10-06, in two parts. A drag that *crosses* an existing wall at
  a corner is fine as it is — "if someone's creating 4 rooms that's super
  common" — and splits nothing. A wall that *ends* on an existing wall
  "creates a join, really, so we can build the concept of a structure":
  that is spec 078, and its first requirement is now built.
- A snapped light on a wall: which side does it hang? The decision above
  puts it on the line and lights both sides. If a sconce should light only
  the room it faces, that is an offset and a facing, and a different spec.

## What this spec does not do

- It does not add full keyboard navigation of the board and its tools. The
  owner wants it ("I want full keyboard navigation"), and it is the next
  spec, not this one; `S` is the only key this spec binds.
- It does not change what a wall occludes, what a door does, or who may
  draw either.
- It does not persist the snapping state to the scene or the account.
- It does not add snapping for drawings (spec 073's shapes) beyond what
  turning the switch off already frees; shapes keep their own rules.
