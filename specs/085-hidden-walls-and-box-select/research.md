# Research: Hidden Walls and Box Select

Read on 2026-10-07 against `main` at `65e57ff6`, with spec 082 in progress
in the same tree. Line numbers are from that reading.

## R1. Hidden is `secret`, set by `setDoorSecret` on any wall

**Decision**: Hidden is the existing `walls.secret`. The web sets it with
the existing `setDoorSecret(wallId, secret)`, one call per wall. There is no
new column, migration or mutation.

**Verified**:

- `set_door_flag_impl` (`mutations_interactives_support.rs:402`) reads the
  wall's scene, checks `is_dm_of_scene` and `refuse_scene_if_paused`, and
  writes `walls.secret`. It never reads `door_state`, so a plain wall is
  accepted.
- `announce_door` records `DOOR_CHANGED` and then `WALL_CHANGED`
  (`EVENT_CODE_WALL_CHANGED`, action `updated`). Every board's wall sync
  therefore re-reads the wall and its `secret`, with no web change.
- The demo's `setDoorSecret` → `changeDoor` (`handlers.ts:220`, `507`)
  updates any wall.
- `setDoorSecret` is already classified in `play_pause_surface_tables.rs`.
  FR-016 holds: no new GraphQL.

**Alternatives rejected**:

- _A `secret` field on `updateWall`._ `mutations_walls.rs:147` deliberately
  leaves it out ("Lock and secret are door properties with their own
  GM-only mutations"). Adding it would be a second path to the same flag,
  and a schema change, which makes the full suite the gate.
- _Renaming to `setWallHidden`._ That is a schema change with nothing
  gained.

## R2. The web side of hiding: one store command, bridged

**Decision**: There is a new world-store command,
`set_walls_hidden { wallIds: string[], hidden: boolean }`.

- **The store.** The store applies it optimistically: it sets `secret` on
  each wall in `walls`.
- **The bridge.** `startWallMutationBridge` (`sync/walls.ts`) answers it
  with one `setDoorSecret` per wall, through the bridge's existing
  per-wall turn (`inTurn`), so it cannot race an edit of the same wall.
- **Rollback.** Each call that fails restores that wall's record as the
  store held it before the command. It is dispatched with source `"sync"`,
  so it is not sent again.
- **Notice.** The refusals are counted into one notice (R7).

**Why**: AGENTS.md forbids a component calling GraphQL. `DoorControls.tsx`
and `doorActions.ts` already do that for doors. That is existing debt, and
this spec leaves it alone: they keep working and are not this spec's
scope. A follow-up can route them through the same command.

## R3. The door click keeps `secret`

**Decision**: In `systems/wall.rs` (≈278–292), the door click stops setting
`updated.secret = false` and drops `secret: false` from the emitted
`update_wall` changes. The server ignored that field already. The change
makes the engine's own copy agree with the server until the next re-read.

`door.reveal` (the interaction effect) still clears `secret` on the
server. This spec does not touch it.

## R4. A group beside the single selections

**Decision**: There is a new resource, `GroupSelection { tokens, walls,
lights, shapes: Vec<String> }`, in
`crates/thunderforge-engine/src/resources/group_selection.rs`.

- **The existing selections stay.** `SelectedWall`, `SelectedLight` and
  `SelectedShape` keep their `Option<String>` and name the primary of
  their kind. `SelectedToken` stays the token list.
- **Writing a box.** A box writes the group, sets `SelectedToken` to the
  group's tokens, and sets each single selection to its kind's first id.
- **Clearing.** Any single-item selection, whether a click in a tool's mode
  or a click on empty board, clears the group to that one item or to
  nothing.
- **To the web.** The engine emits one `select_group` event, so the store
  learns the whole group at once.

**Why not make the three resources lists (the spec's first draft)**: Their
`.0` is read in eleven files. `systems/shape.rs`, `systems/wall.rs` and
`plugins/shape.rs` are among them, and spec 082 is rewriting those files
now. Keeping the type unchanged means 085 touches none of those readers.
The primary-per-kind is what every panel shows anyway. spec.md FR-007 is
corrected to say this.

## R5. The box: a pure rule in canvas-core, a gesture in the engine

**Decision**: There is a new module,
`crates/thunderforge-canvas-core/src/box_select.rs`, with tests in
`box_select_tests.rs`.

```rust
pub struct ScreenBox { pub min: Vec2, pub max: Vec2 }   // world space once converted
pub enum Footprint { Point(Vec2), Segment(Vec2, Vec2), Rect { min: Vec2, max: Vec2 } }
pub fn wholly_inside(b: &ScreenBox, f: &Footprint) -> bool;
pub fn apply_box(current: &[String], hits: &[String], toggle: bool) -> Vec<String>;
```

- **Building the box.** The engine converts the two screen corners to
  world space with the camera, then orders them into `min` and `max`.
- **How each kind is measured.** A token is the rect of its footprint, the
  same size the hit test uses. A wall is its segment, so both ends must be
  inside. A light is its centre point. A shape is the bounding rect of its
  geometry, from `shape_bounds(kind, &geometry)`. That function sits
  beside `translate_geometry` (R6).
- **Toggle.** With shift held, `apply_box` removes the hits already
  selected and appends the rest, keeping order. Without shift it returns
  `hits`.

The gesture is `crates/thunderforge-engine/src/systems/box_select.rs`, and
it runs only in `AuthoringMode::Select`.

- **The press.** A press on empty board records the anchor. "Empty" means
  no token is under it (the same `tokens_at` the drag uses) and no member
  of the group is under it.
- **The drag.** Past the existing click threshold, the drag draws the box
  as a gizmo outline in world space.
- **The release.** The release selects the candidates wholly inside
  (R8). A release without the drag keeps today's deselect.
- **Shift.** Shift is read from the mouse gesture's `ButtonInput<KeyCode>`
  at release. Keyboard route planning (`token_move.rs:166`) is
  keyboard-only, so the two do not meet.

**The one change to `handle_token_drag`**: Today a press on empty board
deselects at once (`token.rs` ≈245). It must not do that while shift is
held, or a shift-box would lose the selection it toggles against. A press
on a token that is already in a group of two or more must leave the drag
to the group move (R6). Both are guard clauses at the top of the press
branch.

## R6. Group move

**Decision**: There is a new system,
`crates/thunderforge-engine/src/systems/group_move.rs`, in Select only. It
starts when a press lands on a member of a group of two or more.

- **The offset.** On every frame it shows each member moved by the
  cursor's offset from the press. The offset is snapped the way the
  pressed item snaps: a token to the grid, and anything else as its tool
  snaps (spec 077). So a token in the group lands on a cell, and
  everything else keeps its distance to it.
- **The release.** Each member gets its existing per-item event:
  - a token gets `upsert_token`, with the same fields as `token.rs`'s
    release;
  - a wall gets `update_wall`, carrying `x1`, `y1`, `x2` and `y2`;
  - a light gets `update_light`, carrying `x` and `y`;
  - a shape gets `update_shape`, carrying the translated `geometry`.
- **A player's tokens.** A player's token is judged on its own path with
  `token_move::refuse_at_wall` (FR-013). A refused token is not sent, and
  goes back at once. Each event carries `group: { id, size }` (R7).
- **Shape geometry.** `translate_geometry` (`systems/shape.rs:179`) is
  private in a file spec 082 is editing. After 082 lands, it moves to
  canvas-core as `shape_geometry::translate`, with its two tests, beside
  `shape_bounds`. `systems/shape.rs` calls it from there.

## R7. One notice for a group

**Decision**: There is a web module, `apps/web/src/engine/world/sync/groupMoves.ts`.

- **Opening.** The engine stamps every event of a group move or group
  delete with `group: { id, size }`. The store command types gain an
  optional `group` field. `groupMoves.expect(id, size)` opens a tally.
- **Counting.** Each bridge calls `groupMoves.settle(id, ok)` once its
  mutation answers.
- **The notice.** When `size` answers have arrived and any refused, it
  shows one toast: "2 of 5 could not be moved." For a delete it says
  "deleted".
- **Tokens.** `applyMoveRefusal` (`sync/tokens.ts:229`) still re-reads and
  puts the token back. When the command carried a `group`, it skips its
  own toast.
- **Rollback per kind** (FR-014):
  - tokens, by the existing re-read;
  - walls and lights, by a new cached-record rollback in their bridges,
    which `set_walls_hidden` (R2) also uses;
  - shapes, by spec 082 T018.

**Why not an engine-side tally**: The answers arrive in the bridges, not in
the engine. And a notice is the web's job.

## R8. Who the box takes

**Decision**: The rule is
`box_candidate(viewer: &Viewer, kind, item) -> bool`, in
`crates/thunderforge-engine/src/systems/box_select.rs`. It is pure over
plain values, so it is host-tested.

- **For a GM:** every kind that `SelectionFilter` allows. `SelectionFilter`
  is read for the first time here; spec.md is corrected to say so.
- **For a player:**
  - a token qualifies when `TokenOwner` is the viewer (082's
    `ViewerUserId`) and its `Visibility` is not `Hidden`, since lighting
    writes that for "this viewer cannot see this token"
    (`token_culling.rs` documents it);
  - a shape qualifies when `may_edit_shape` allows it (082 T012);
  - walls and lights never qualify;
  - `SelectionFilter` is ignored.

There is no level test: the engine holds one level only (`sync/levels.ts`).

**Not changed**: A single click in Select still picks any token under it,
regardless of the filter. Making the click honour the filter is a separate
behaviour change, and the spec does not ask for it.

## R9. The token's owner in the engine

**Decision**: `WorldTokenPayload` gains `owner_user_id`, read from JSON as
`ownerUserId`.

- **Absent or null.** The field is declared as
  `Option<Option<String>>`, with `#[serde(default,
deserialize_with = …)]`. An absent field keeps the current owner,
  because position-only upserts omit it. `null` clears the owner.
- **Where it is kept.** It lands on a new `TokenOwner(Option<String>)`
  component, added where the token entity is spawned or updated in
  `systems/token.rs`. The upsert handler is there.
- **The web side.** The web's `WorldToken` already has `ownerUserId`,
  filled by the token sync. The store hands the engine the token record
  as is, so nothing changes on the web side.
- **Changing hands.** When a token's owner changes to another user, the
  token leaves the player's group (spec edge case). The upsert handler
  removes it from `GroupSelection` and `SelectedToken` when the viewer is
  not a GM and the new owner is not the viewer.

## R10. Delete for a group

**Decision**: In Select, Delete and Backspace with a group of two or more
emit one `delete_*` per member, stamped with `group`.

- **For a player:** only their own shapes are sent. Players do not delete
  tokens today, and the spec does not add that.
- **The existing handlers.** The per-mode Delete handlers (`wall.rs:678`,
  `lighting_edit.rs:141`, `shape.rs:514`) are unchanged, because they run
  only in their own modes.

## R11. The Select bar

**Decision**: There is a new component,
`apps/web/src/components/canvas-tools/SelectionBar/SelectionBar.tsx`. It
renders in Select mode when the store's group holds more than one item.

- **What it shows:** a count by kind, **Hidden from the table** for the GM
  when the group holds a wall, and **Delete**.
- **What it does:** it dispatches only: `set_walls_hidden`, and a
  `delete_group` command that the engine turns into R10's events. It
  reaches the engine as the `DeleteSelection` external command, so Delete
  from the bar and from the key take one path.
- **The single wall.** `WallTool` gains the same checkbox for its single
  selected wall.

## R12. Proof and probe

**Decision**: The web probe gains two readers, `__engineProbe.selection()`
and `__engineProbe.drawnWalls()`. Their wasm sources are new SDK functions,
`selection_state()` and `wall_visuals()`.

- **`selection()`** returns `{ tokens, walls, lights, shapes }`, read from
  the engine's own `GroupSelection`.
- **`drawnWalls()`** returns the ids of the walls this canvas draws. That
  is how the e2e asserts that a player's board does not draw a hidden wall
  without inferring it from pixels.

The e2e is `apps/web/e2e/canvas-box-select.spec.ts`. It is owned by the
`canvas` slice through its `canvas-` prefix, so `slices.json` is
unchanged. Run it with `pnpm e2e:canvas`.

## R13. Sequencing against spec 082

Spec 085's engine and web work starts after these spec 082 tasks:

| 082 task | What 085 needs from it         |
| -------- | ------------------------------ |
| T008     | `ViewerUserId` and `createdBy` |
| T012     | `may_edit_shape`               |
| T015     | the player's Select rail       |
| T018     | shape rollback                 |

Phases that touch neither those files nor 082's surface can start at once:

- canvas-core `box_select`;
- the door-click fix;
- `set_walls_hidden`.

`translate_geometry` moves only after 082 is merged, because
`systems/shape.rs` is 082's file until then.
