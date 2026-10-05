# Feature Specification: Lights and Drawings From a Right-Click

**Feature Branch**: `073-shape-and-light-menus`
**Created**: 2026-10-05
**Status**: Draft
**Input**: The owner, 2026-10-05: "lets tackle the shape and lighting menus". Spec 071 gave walls and doors a right-click menu and said it did not do shapes or lights.

## Why

A placed light and a drawing could each be changed only from its tool: arm
the Lighting or the Shape tool, select the thing, then use the panel. In the
middle of a scene that is three steps to put a torch out, or to show the
table the outline of a pit that was drawn before the session. A right-click
on a token or a door already opens a menu about it; on a light or a drawing
it opened the bare board's menu, as if nothing were there.

## Decisions already made

- **Nothing new is stored, and no new mutation.** Every item is a change the
  tool panels already make, sent the way they send it: an intent into the
  world store (`update_light`, `delete_light`, `update_shape`,
  `delete_shape`).
- **These menus are the Game Master's.** Only a Game Master may change a
  light or a drawing, so only a Game Master's right-click is told one was
  under it. A player is offered nothing.
- **One right-click is about one thing: token, then light, then wall or
  door, then drawing.** The smaller and more deliberate target wins. A
  light's marker is a dot; a rectangle can cover a room, and what stands in
  the room must stay clickable.
- **A drawing is clicked where it is drawn.** Anywhere on a filled rectangle;
  on the outline of an ellipse, a line or a freehand stroke; on the letters
  of a text. The board inside an ellipse is still board. The Shape tool's own
  left-click picks by a drawing's middle, and is left as it is.
- **Putting a light out sets its intensity to nothing; lighting it sets it to
  full.** A light is on whenever its intensity is above zero (spec 045), and
  there is nowhere to keep what it was before. A light dimmed in the Lighting
  tool comes back at full and can be dimmed there again.
- **Removing does not ask first.** A light or a drawing takes a moment to put
  back. A token's removal asks, because a token carries a creature's state.

## User Scenarios & Testing

### User Story 1 — The Game Master works a light where it stands (Priority: P1)

The torch in the hall burns down. The Game Master right-clicks its marker and
puts it out; later lights it again; lets a magical light through walls; and
removes one that is no longer wanted.

**Acceptance scenarios**:

1. **Given** a burning light, **when** the Game Master right-clicks its
   marker, **then** the menu is named Light and offers Put it out, Shine
   through walls or Stop at walls, and Remove this light.
2. **Given** a light that is out, **when** they right-click it, **then** the
   first item is Light it.
3. **Given** a light a token carries or wears, **when** they right-click it,
   **then** the menu is the token's.

### User Story 2 — The Game Master works a drawing where it is drawn (Priority: P1)

The party finds the pit. The Game Master right-clicks its outline and shows
it to the table; hides a note meant only for them; removes a scribble.

**Acceptance scenarios**:

1. **Given** a drawing kept from the players, **when** the Game Master
   right-clicks it, **then** the menu is named for what it is and offers Show
   to players and Remove from the board.
2. **Given** a drawing the players see, **when** they right-click it,
   **then** the first item is Hide from players, and choosing it takes the
   drawing off every player's board.

### User Story 3 — A player is offered nothing (Priority: P2)

1. **Given** a light or a drawing, **when** a player right-clicks it,
   **then** no menu about it opens.

### Edge cases

- A token standing on a drawing, or on a light: it is about the token.
- A light set in a doorway, clicked on its marker: it is about the light.
  Clicked along the door beside the marker, it is about the door.
- Two drawings, one over the other: it is about the one drawn later.
- A drawing with nothing in it is under no click.

## Requirements

- **FR-001** The engine's right-click event names, for a Game Master only,
  the nearest placed light whose marker is within reach — never one a token
  carries or wears — and the drawing whose ink is within ten screen pixels.
- **FR-002** Which one thing a right-click is about is one rule: token,
  light, wall or door, drawing.
- **FR-003** What the menu offers on a light and on a drawing is one rule
  each, apart from the menu, decided from who is asking and its state.
- **FR-004** A Game Master's menu on a light: Put it out / Light it; Shine
  through walls / Stop at walls; Remove this light.
- **FR-005** A Game Master's menu on a drawing: Show to players / Hide from
  players; Remove from the board. It is named Rectangle, Ellipse, Line, Text
  or Drawing.
- **FR-006** Every change is an intent into the world store. The menu makes
  no network call of its own for a light or a drawing.
- **FR-007** A drawing hidden from the players leaves their boards at once.
  A player is never sent a hidden drawing, so when a shape event names a
  drawing the server no longer returns, the shape sync drops it. Before
  this spec the drawing stayed on a player's board until they reloaded.

## Success Criteria

- **SC-001** `canvas-light-and-drawing-menu.spec.ts` passes from the main
  checkout: put out, relit, through walls, shown to the table, refused
  player, hidden, both removed.
- **SC-002** The canvas, lighting and interactive slices still pass.

## What this spec does not do

- It does not open these menus from the keyboard. The ContextMenu key opens
  the selected token's menu; a light or a drawing is selectable only in its
  tool, whose panel remains the keyboard path.
- It does not change a light's reach or colour, or a drawing's colour or
  text. Those need a value, and the tool panels ask for one.
- It does not fasten a light to a token from the menu.
- It does not change how the Shape tool's left-click picks a drawing.
