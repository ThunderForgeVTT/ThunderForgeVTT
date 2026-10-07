# Feature Specification: Hidden Walls and Box Select

**Feature Branch**: `085-hidden-walls-and-box-select`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "can walls optionally be hidden and can the select tool get a select box for allowing multi select? which should operate a little diff for players vs gm"

## Why

Walls are the GM's scaffolding. Some of them should be part of the picture,
such as a fence or a cliff edge, and the players should see them. Others are
there to stop a token walking through a painted pillar. Drawn over the map,
those lines spoil the map, and they hint at things the GM would rather
leave to the art. A GM should be able to keep a wall working while not
drawing it for the table.

Selecting one thing at a time is slow. A GM tidying a room picks up six
goblins one by one. A player moving their familiar and their mount does the
same. A box dragged around them is how every drawing tool works.

The two roles want different things from that box. The GM authors the
scene, so their box takes whatever they have asked Select to act on. A
player only moves what is theirs, so their box takes their own tokens and
their own shapes and leaves everything else alone.

## What exists

Counted on 2026-10-07:

- **Hidden walls already exist, but only for doors.** Every wall has a
  `secret` flag:
  - It is stored in `walls.secret` (`schema.rs`) and carried on the core
    `Wall` (`canvas-core/src/wall.rs:64`).
  - The engine does not draw a `secret` wall for a player
    (`systems/wall.rs:739`), but it still blocks sight and movement. For the
    GM it is drawn faint (`SECRET_DOOR_COLOR`).
  - The door context menu leaves it out for players (`context_menu.rs:86`).
  - `setDoorSecret` (`mutations_interactives.rs:271`) is GM only and checks
    the pause. It records `EVENT_CODE_DOOR_CHANGED` and
    `EVENT_CODE_WALL_CHANGED` (`announce_door`).
  - Nothing in `setDoorSecret` requires the wall to be a door. Only the UI
    does: the toggle lives in `DoorControls.tsx`, which shows only for doors.
  - Turning a wall into a door with a click clears `secret` locally and
    sends `secret: false` in its `update_wall` changes
    (`systems/wall.rs:280`). The server ignores that field: `updateWall`
    cannot set `secret` (`mutations_walls.rs:147`), and the web's
    `WallFieldChanges` has no `secret`.
  - The web calls `setDoorSecret` straight from components
    (`DoorControls.tsx:87`, `doorActions.ts:52`). The wall mutation bridge
    (`sync/walls.ts`) sends only `updateWall`, and has no rollback: a
    refusal is logged and the board keeps the refused edit.
  - The demo answers `setDoorSecret` (`handlers.ts:507`), and the pause
    table classifies it.
- **Selection.**
  - `SelectedToken` is already a list with a primary
    (`resources/selection.rs:39`), and a click on a pile takes the whole
    stack. `SelectedWall`, `SelectedLight` and `SelectedShape` hold one id
    each.
  - `SelectionFilter` (`plugins/selection_filter.rs`, spec 031 FR-008) says
    which kinds the GM's Select acts on. The GM sets it in
    `SelectionFilterMenu.tsx`, and it is kept per user. **No engine system
    reads it** (checked 2026-10-07 while planning): the resource is set and
    never consulted, so today it changes nothing. The box is the first
    thing to read it.
  - **Select picks only tokens.** In `AuthoringMode::Select` only
    `handle_token_drag` runs; a wall, light or shape is selected only in its
    own tool's mode (`authoring_tool_allowed`).
  - **A wall cannot be moved whole.** The wall tool drags an endpoint
    (`WallDragMode::MovingEndpoint`); a press on a wall's body selects it.
    Lights and shapes move whole in their own modes
    (`translate_geometry`, `systems/shape.rs:179`).
  - The engine holds one level at a time: the web loads only the level in
    view (`sync/levels.ts`). Nothing in the engine knows about levels.
  - A press on empty board deselects (`systems/token.rs:247`).
  - A stack drag sends one `upsert_token` per token, through the mutation
    bridge (`systems/token.rs:353`). The server checks each token on its
    own: a GM moves anything, a player only a token whose `owner_user_id`
    is theirs (`mutations_tokens.rs:73`).
  - The engine does not know who owns a token: `WorldTokenPayload` has no
    owner, although the web's `WorldToken.ownerUserId` already carries it.
  - A refused token move is put back from a re-read of the server, with a
    toast per token (`applyMoveRefusal`, `sync/tokens.ts:229`). The light
    and wall bridges only log a refusal. Spec 082 T018 adds rollback to the
    shape bridge.
  - Shift held on the keyboard plans a route (`systems/token_move.rs:166`).
    No mouse gesture uses shift.
- **Players and the Select tool.** Spec 082 gives every player Select and
  Shapes by default. It records `created_by` on the core `Shape` and gives
  the engine the viewer's user id (082 FR-016, FR-017). This spec builds on
  both and does not repeat them.

## User Scenarios & Testing

### User Story 1 - The GM hides a wall from the table (Priority: P1)

The GM selects a wall and ticks **Hidden from the table**. The players stop
seeing the line, and their tokens still stop at it. The GM still sees it,
faint.

**Why this priority**: This is the first half of the ask, and almost all of
it exists already.

**Independent Test**: A GM and a player. The GM hides a plain wall. The
player's board no longer draws it, and the player's token cannot be dragged
through it.

**Acceptance Scenarios**:

1. **Given** a plain wall (not a door),
   **When** the GM ticks **Hidden from the table** in the wall panel,
   **Then** the player's board stops drawing it, and the GM's draws it in
   the faint hidden colour.
2. **Given** a hidden wall,
   **When** the player drags their token across it,
   **Then** the token goes back to where the drag began, exactly as for a
   drawn wall (spec 045 FR-015).
3. **Given** a hidden wall that blocks vision,
   **When** the player's token stands beside it,
   **Then** sight stops at it.
4. **Given** several walls selected (US2),
   **When** the GM ticks **Hidden from the table**,
   **Then** every selected wall is hidden.

---

### User Story 2 - The GM boxes several things (Priority: P1)

With Select, the GM drags a box over empty board. Everything inside it that
`SelectionFilter` allows is selected: tokens, walls, lights and shapes.
Dragging any one of them moves them all. **Delete** removes them all.

**Why this priority**: This is the second half of the ask.

**Independent Test**: A GM, a scene with three tokens and a wall. Box the
four, drag them, reload. All four are where they were dropped.

**Acceptance Scenarios**:

1. **Given** Select armed and the filter allowing tokens and walls,
   **When** the GM drags a box from empty board around three tokens and a
   wall,
   **Then** all four are selected. A token or wall wholly inside the box is
   taken, and one only crossing its edge is not.
2. **Given** that selection,
   **When** the GM drags one of the tokens,
   **Then** all four move by the same offset. Each one's move goes to the
   server and comes back to every board.
3. **Given** a selection,
   **When** the GM shift-drags a second box, or shift-clicks one item,
   **Then** the items it touches are added if unselected and removed if
   selected.
4. **Given** the filter with walls turned off,
   **When** the GM boxes the same area,
   **Then** the wall is not taken.
5. **Given** a selection,
   **When** the GM presses Delete,
   **Then** every selected item is deleted.

---

### User Story 3 - A player boxes what is theirs (Priority: P2)

A player drags a box over their own token, their familiar and a goblin. The
box takes their two tokens and not the goblin. They drag both through the
door together.

**Why this priority**: This is the "a little different for players" half.
It needs spec 082's player Select.

**Independent Test**: A player who owns two tokens. They box both tokens
and a GM-owned token, and only their two are selected. Dragging moves
those two, and the GM-owned token stays put.

**Acceptance Scenarios**:

1. **Given** a player with Select,
   **When** they box their two tokens, an NPC, a wall, a light, their own
   shape and the GM's shape,
   **Then** the selection is their two tokens and their own shape, and
   nothing else.
2. **Given** a token the player cannot see (hidden by sight, spec 076),
   **When** it is inside the box,
   **Then** it is not taken, even if the player owns it.
3. **Given** the player's two tokens selected,
   **When** they drag them so that one path crosses a wall and the other
   does not,
   **Then** the one that would cross goes back, the other lands, and the
   player is told that one token could not move there.

---

### Edge Cases

- **Converting a hidden wall into a door** keeps it hidden. The door click
  stops clearing `secret`, so a hidden wall made into a door is a secret
  door. Revealing a door (the interaction effect `door.reveal`) still
  clears it.
- **A hidden wall's shadow** shows where it is to a player whose sight it
  cuts. That is accepted, as it is for secret doors: the geometry reaches
  every client, and only the drawing differs (`canvas-core/src/wall.rs:59`).
- **A box that starts on an item** is a drag of that item, not a box. A box
  starts only on empty board.
- **A box with nothing in it** clears the selection, as a click on empty
  board does today. A shift-box with nothing in it changes nothing.
- **A token on another level** is never taken. Only what is on the level in
  view is selectable.
- **Part of a group refused.** Each item is its own mutation. One the
  server refuses rolls back alone, the rest stay where they landed, and the
  mover sees one notice: "2 of 5 could not be moved".
- **A paused scene** refuses every move for a player, as one move does
  today (spec 064). The group goes back.
- **Keyboard shift** still plans a route. Box select reads shift only with
  the mouse.
- **A token's owner changes** while it is selected by a player. The next
  move is refused by the server and rolls back. The engine drops it from
  the selection when the new owner arrives.

## Requirements

### Functional Requirements

**Hidden walls**

- **FR-001**: Any wall, door or not, MAY be hidden. Hidden is the existing
  `secret` flag. No column, no migration and no new mutation:
  `setDoorSecret` already applies to every wall, and keeps its name.
- **FR-002**: The wall panel (`WallTool`) MUST offer **Hidden from the
  table** for the selected wall, and the Select bar (FR-018) for every wall
  in a group. Either dispatches one world-store command,
  `set_walls_hidden { wallIds, hidden }`. The wall mutation bridge turns it
  into one `setDoorSecret` per wall and rolls back each refused wall. No
  component calls `setDoorSecret` for this. `DoorControls.tsx` keeps its own
  toggle.
- **FR-003**: The door click (`systems/wall.rs:280`) MUST stop clearing
  `secret`, both on the engine's wall and in the `update_wall` changes it
  emits.
- **FR-004**: A hidden wall MUST keep blocking vision and movement for
  everyone. Nothing reads `secret` in the sight or movement code, and that
  stays true.

**Box select (both roles)**

- **FR-005**: With Select armed, a press on empty board followed by a drag
  past the click threshold MUST draw a box in screen space. Its release
  selects every candidate (FR-008, FR-009) that lies wholly inside the box.
  A press and release without that drag still clears the selection, as
  today.
- **FR-006**: Shift held at release MUST toggle the candidates inside the
  box against the current selection. Shift-click MUST toggle one item.
- **FR-007**: A selection MUST hold several items of every kind.
  `SelectedToken` is already a list. `SelectedWall`, `SelectedLight` and
  `SelectedShape` stay as they are and name the primary of their kind, so
  every single-selection caller and panel is unchanged. A new
  `GroupSelection` resource holds the full list per kind, and the engine
  emits it as one `select_group` event. The world store keeps
  `selectedWallIds`, `selectedLightIds` and `selectedShapeIds` beside the
  primaries.

**Who the box takes**

- **FR-008**: For a GM, the candidates are the tokens, walls, lights and
  shapes on the level in view that `SelectionFilter` allows. Tokens the GM
  can see include NPCs hidden from players.
- **FR-009**: For a player, the candidates are the tokens whose
  `owner_user_id` is the viewer and that the viewer can currently see, and
  the shapes whose `created_by` is the viewer (spec 082). Walls and lights
  are never candidates. The player's box ignores `SelectionFilter`, since a
  player has no filter menu.
- **FR-010**: The engine's token payload MUST carry `owner_user_id`, so
  that FR-009 can be decided in the engine. The viewer's id comes from
  spec 082 FR-017.

**Acting on a selection**

- **FR-011**: In Select, dragging any item of a group of two or more MUST
  move every selected item by the same offset. A wall moves whole: both
  endpoints, in one `update_wall` carrying `x1`, `y1`, `x2` and `y2`. Each move is the item's existing per-item mutation
  through its bridge (`upsert_token`, `update_wall`, `update_light`,
  `update_shape`). There is no batch mutation. A group move is N persisted
  changes, as a stack move already is.
- **FR-012**: Delete MUST delete every selected item that the viewer may
  delete, one mutation each.
- **FR-013**: For a player, a group move judges each token's own path
  against the walls (spec 045 FR-015). A token whose path crosses goes
  back, and the others land.
- **FR-014**: A refusal from the server MUST roll back only the refused
  item. A token goes back by its existing re-read; a wall or light by the
  record the store held before the move; a shape by spec 082's rollback.
  The mover sees one notice that counts the refusals, in place of a toast
  per token.
- **FR-015**: The engine's candidate rules are a courtesy. The server's
  per-item checks are the rule and do not change.

**Surface**

- **FR-016**: No new GraphQL mutation is added, so
  `play_pause_surface_tables.rs` is unchanged. If planning finds one is
  needed after all, it MUST be classified there.
- **FR-017**: The demo needs no backend change for hidden walls: its
  `setDoorSecret` (`handlers.ts:507`) does not ask for a door either. Box
  select is engine and web only and runs in the demo unchanged.
- **FR-018**: In Select, while the group holds more than one item, a
  Select bar MUST show how many of each kind are selected, **Hidden from
  the table** when the group holds a wall (GM only), and **Delete**.

### Key Entities

- **Hidden wall**: a wall with `secret = true`. It is drawn only for the GM
  and blocks for everyone. It is the existing flag with a wider use.
- **Selection**: per kind, an ordered list of ids, with the primary first.
  It is session-local engine state and never persisted.
- **Box**: a screen-space rectangle, from a press on empty board to its
  release. It is never sent anywhere.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A hidden plain wall is not drawn on a player's board, and it
  stops that player's token and sight. Both are asserted in one e2e.
- **SC-002**: A GM boxes three tokens and a wall, moves them, and after a
  reload all four are at their new positions.
- **SC-003**: A player's box over two owned tokens, an NPC, a wall and the
  GM's shape selects exactly the two tokens.
- **SC-004**: A group move with one refused item leaves the other items
  moved and the refused one back where it started, on both boards.
- **SC-005**: `make lint` (host and wasm32) passes.

### Proof

- Engine unit tests, which run on the host:
  - the candidate rule for each role;
  - wholly inside versus crossing the edge;
  - shift toggling;
  - the door click keeping `secret`.
- `pnpm e2e:canvas`, with a new `apps/web/e2e/canvas-box-select.spec.ts`
  owned by the `canvas` slice:
  - US1: a GM and a player, a hidden wall;
  - US2: the GM's box, move and reload;
  - US3: the player's box, including a partial refusal.

## Assumptions

- Spec 082 lands first. It gives players Select, `Shape.created_by` and the
  viewer's id in the engine. Without it, US3 covers tokens only.
- "Wholly inside" is the rule, not "touches". It is the predictable choice
  when walls run edge to edge across a room.
- There is no scene-wide "hide every wall" switch. The GM boxes a room and
  ticks **Hidden from the table**. If GMs ask for a per-scene default
  later, it is a scene setting of its own.
- Rotating or resizing a group is out of scope. Only moving and deleting a
  group are in.
