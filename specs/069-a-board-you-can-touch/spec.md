# Feature Specification: A Board You Can Touch

**Feature Branch**: `069-a-board-you-can-touch`
**Created**: 2026-10-04
**Status**: Draft
**Input**: The owner, 2026-10-04, on the engine's readiness for a field test: "Bevy should support touch input."

## Why

Every system that points at the board reads a mouse: `ButtonInput<MouseButton>`
and the window's cursor. A browser does not turn a finger into either. On a
tablet or a phone the board drew, and could not be panned, zoomed or played
on.

## Decisions already made

- **Touch is translated once, not taught to every system.** One plugin, in
  `PreUpdate`, turns fingers into what the board already reads. Nothing that
  handles a click knows whether a finger made it.
- **The gestures**:

  | On the glass | What it does |
  | --- | --- |
  | One finger | What the left mouse button does there: select, drag a token, draw with the armed tool |
  | One finger held still for half a second | A right-click when it lifts: the menu |
  | Two fingers | Pan by how they move together, zoom by how they spread |

- **One finger never pans.** A left-drag on bare board does not pan either,
  and a finger that sometimes moved a token and sometimes the map would be
  the worse of the two.
- **The finger's position is not written into the window's cursor.** The
  window backend tries to move the real cursor to a position set from inside
  the app, and a browser refuses with an error per frame. Systems ask a
  `Pointer` for the position: the finger if one is down, the cursor if not.

## Requirements

- **FR-001** One finger presses the left button at the finger, moves the
  pointer with it, and releases where it lifted.
- **FR-002** A finger that has not travelled as far as a drag, held for
  `LONG_PRESS_SECS`, releases left and presses right; lifting it releases
  right. A finger that dragged is never a long press, even back where it
  started.
- **FR-003** Two fingers press nothing. The camera pans by their midpoint and
  zooms by the ratio of their spread, anchored between them. A second finger
  landing lets go of whatever the first held.
- **FR-004** The finger left over after a pinch does nothing until every
  finger is up.
- **FR-005** With no finger down, nothing changes for a mouse.

## Success Criteria

- **SC-001** With real touch events in a browser: a token follows one finger;
  holding on it opens its menu and does not move it; two fingers pan the map
  by the distance they moved and spreading them to twice as far apart halves
  the camera's scale.
- **SC-002** The canvas, interactive and lighting e2e slices pass, since
  every pointer reader changed where it asks for the position.

## What this spec does not do

- It does not audit the React interface around the board for touch — the
  dock, panels, sheets and dialogs. Those are ordinary DOM and take taps, but
  nobody has checked their target sizes or hover-only affordances on a
  tablet.
- No rotate gesture, no three-finger gestures, no stylus pressure.
- Keyboard movement has no touch counterpart; a token is dragged.
