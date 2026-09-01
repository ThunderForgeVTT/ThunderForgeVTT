# Contract: Engine ↔ chrome boundary

What React may ask the engine to do, and what the engine reports back. This
contract exists to keep Constitution Principle I enforceable: React observes
and requests; it never becomes a second source of truth for canvas state.

**The rule this contract protects**: the cursor-attached token in Place is
engine state. A React element following the mouse would not line up with the
camera or the grid, and would be a second simulation. The real `<canvas>` is a
`body`-level, `position: fixed` element inserted by Bevy/winit — chrome cannot
reason about map coordinates on its own.

---

## Requests (chrome → engine)

### Begin placement

Chrome asks the engine to attach a given actor's token to the cursor.

- The engine owns the preview from this moment: following the cursor, snapping
  to the scene's grid, and rendering it as provisional rather than real.
- Chrome may not position the preview.

### Cancel placement

Chrome, or the engine's own input handling, ends a placement without creating
anything.

- Must leave no trace: no token, no partial state, no lingering preview.
- Must be reachable by keyboard (Escape) as well as by whatever pointer gesture
  is chosen — the spec's edge case of a dropped connection mid-carry resolves
  here and at the server, never as a half-placed token.

### Set selection filter

Chrome tells the engine which content kinds Select acts on.

- Selection is engine state; chrome supplies the preference and renders the
  menu.
- With every kind disabled the engine selects nothing — a legitimate state the
  interface must make obvious rather than appearing broken (spec edge case).

### Set authoring mode

Chrome asks the engine to enter a mode (select, walls, lights, shapes, tokens,
interactions).

- **The engine owns which mode is active** and the transition into it. Chrome
  renders the rail and requests changes; it does not hold the answer.
- The transition must be atomic with respect to input (FR-040a): no input may be
  attributed to the mode just left or the one not yet entered, and a gesture in
  flight when the mode changes must not complete under the new mode's rules.
- Entering a mode must never place, create or modify content — entering is not
  an action.

### Set snapping

Chrome tells the engine whether snapping is on. The engine applies the scene's
grid type; chrome does not compute grid maths (research R8).

### Change scene

Chrome tells the engine the scene changed and whether the party is coming.

- The engine unloads the previous scene's tokens, walls and lights and loads
  the new scene's (FR-018).
- Which tokens survive is decided by the server and the retention rule, not by
  the engine's own judgement.

---

## Reports (engine → chrome)

### Placement confirmed

The user left-clicked to place. Carries the world position the engine resolved,
after snapping. Chrome turns this into the server mutation; the engine does not
persist.

### Placement cancelled

Nothing was created. Chrome returns its own state to normal.

### Interaction activated

Something on the map was activated — a lore marker, a placed item. Carries what
was activated and by whom, so chrome can present the right affordance (open a
tab; offer Pickup or View).

### Engine readiness

Already exists as a monotonic frame counter on the engine's stats. Chrome uses
it to know the loop is running rather than sleeping — the same signal the e2e
suite now waits on.

---

## Right-click (FR-029)

The browser's context menu must be suppressed **on the canvas surface only**,
and nowhere else in the application. Chrome retains normal context menus over
panels, lists and editors.

Because the canvas is a `body`-level element outside the React tree, this
suppression is bound to the canvas itself. Note that a stray-input defect
already exists in this area (research R6) — whatever handles right-click must
not deepen it.

---

## Invariants

1. **Chrome never computes map coordinates.** Screen-to-world is the engine's.
2. **The engine never persists.** It reports; chrome calls the server; the
   server decides.
3. **A rejected server call restores engine state.** Optimistic application is
   permitted; disagreement is resolved in the server's favour.
4. **No canvas state has two owners.** If chrome needs to display something
   about the canvas, it observes engine/world-store state rather than keeping
   its own copy.
