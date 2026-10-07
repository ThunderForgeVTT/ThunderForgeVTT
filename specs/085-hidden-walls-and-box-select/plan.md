# Implementation Plan: Hidden Walls and Box Select

**Branch**: `085-hidden-walls-and-box-select` | **Date**: 2026-10-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/085-hidden-walls-and-box-select/spec.md`

## Summary

The GM can hide any wall from the table. Hidden is the existing `secret`
flag, set by the existing `setDoorSecret`. The web reaches it through a
new world-store command, `set_walls_hidden`, which the wall mutation bridge
sends one wall at a time and rolls back per wall. The door click stops
clearing `secret`.

In Select, a drag from empty board draws a box. The box takes what is wholly
inside it and what the viewer may take:

- for a GM, every kind `SelectionFilter` allows;
- for a player, their own visible tokens and their own shapes.

The selection is kept in a new `GroupSelection` resource. Dragging a member
of a group moves all of them, each by its existing per-item mutation, and a
wall now moves whole. Delete removes all of them. Refusals roll back per
item and are counted into one notice.

The server and GraphQL are unchanged.

The planning corrected the spec (see research.md), and each correction is
now in spec.md:

- `SelectionFilter` is read by nothing today.
- Select picks only tokens.
- Walls had no whole-wall move.
- The wall bridge can neither carry `secret` nor roll back.
- The three single selections stay as they are, and the group sits beside
  them.

## Technical Context

**Language/Version**: Rust (latest stable, edition 2024) for
`thunderforge-canvas-core` and `thunderforge-engine`, which is Bevy,
wasm32. TypeScript and React 19 for `apps/web`.
**Primary Dependencies**: Bevy (the engine) and Vitest (web units). The
e2e runs on Playwright. There are no new dependencies.
**Storage**: None. `walls.secret` already exists, and the selection is
session-local.
**Testing**:

- canvas-core host tests: `box_select_tests.rs` and the geometry tests;
- engine host tests: `cargo test -p thunderforge-engine`, covering the
  candidate rule, the door click, `TokenOwner` and the toggle;
- web Vitest: `set_walls_hidden`, the bridge rollbacks, `groupMoves`, the
  store's group;
- e2e: `canvas-box-select.spec.ts` in `pnpm e2e:canvas`.

**Target Platform**: Browser (wasm engine), the self-hosted stack and the
in-browser demo.
**Project Type**: The web app plus the engine. The server is untouched.
**Performance Goals**: The box is judged once, at release, over the
entities of one level. That is linear in what the board holds, and the
gesture adds no per-frame scan.
**Constraints**:

- Engine lints for wasm32, and `make lint` runs both targets.
- Files stay under 1000 lines (`systems/wall.rs` is at 905 and `app.rs`
  at 885), so the new behaviour goes in new files.
- No client-side database.
- No edits to spec 082's files before 082 lands.

**Scale/Scope**: About 10 new files and about 14 edited. There are 3 user
stories.

## Constitution Check

| Principle                         | How this plan meets it                                                                                                                                                    |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. Server is the authority        | Every move, delete and hide is the existing per-item mutation, checked per item on the server. The engine's candidate rule is a courtesy (FR-015).                        |
| II. Circular flow                 | Components dispatch `set_walls_hidden` and `delete_group` into the store. Bridges send the mutations. Boards render what comes back. No component calls GraphQL for this. |
| III. Optimistic with rollback     | `set_walls_hidden` and group moves apply at once and roll back per refused item (R2, R7). The wall and light bridges gain the rollback they lacked.                       |
| IV. Pure logic is tested natively | The box geometry, toggle and candidate rule live in canvas-core or as pure engine fns, all host-tested.                                                                   |
| V. Agnostic core + adapters       | The canvas-core `box_select` knows nothing of Bevy or the web. The engine adapts ECS state into its `Footprint`s.                                                         |
| VI. Proven by its own slice       | `canvas-box-select.spec.ts` in `pnpm e2e:canvas`. No cross-cutting path changes, so the slice is the gate.                                                                |

There are no violations.

## Project Structure

### Documentation (this feature)

```text
specs/085-hidden-walls-and-box-select/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R13
├── data-model.md        # session state only; no persisted change
├── quickstart.md
├── contracts/
│   ├── engine-selection.md   # GroupSelection, events, commands, probe
│   └── world-store.md        # set_walls_hidden, select_group, delete_group, group stamps
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-canvas-core/src/
├── box_select.rs            # NEW: ScreenBox, Footprint, wholly_inside, apply_box
├── box_select_tests.rs      # NEW
├── shape_geometry.rs        # NEW (after 082): translate, shape_bounds, + tests
└── lib.rs                   # pub mod box_select; pub mod shape_geometry

crates/thunderforge-engine/src/
├── resources/group_selection.rs   # NEW: GroupSelection
├── systems/box_select.rs          # NEW: gesture, box_candidate, gizmo
├── systems/group_move.rs          # NEW: drag, release events, Delete for a group
├── systems/wall.rs                # door click keeps secret (R3)
├── systems/token.rs               # TokenOwner on upsert; two guard clauses (R5)
├── systems/shape.rs               # calls shape_geometry::translate (after 082)
├── payloads.rs                    # owner_user_id; select_group/DeleteSelection
├── app.rs                         # register resource and systems
└── sdk.rs                         # selection_state(), wall_visuals(), delete_selection()

apps/web/src/
├── engine/world/types.ts          # SetWallsHiddenCommand, SelectGroupCommand, DeleteGroupCommand, group stamp
├── engine/world/store.ts          # selected*Ids, the three commands
├── engine/world/sync/walls.ts     # setDoorSecret per wall, rollback, group settle
├── engine/world/sync/lights.ts    # rollback, group settle
├── engine/world/sync/shapes.ts    # group settle (082 owns its rollback)
├── engine/world/sync/tokens.ts    # group settle; no per-token toast in a group
├── engine/world/sync/groupMoves.ts            # NEW
├── engine/bevy/index.ts           # probe readers, deleteSelection
├── components/canvas-tools/WallTool/WallTool.tsx            # Hidden from the table
├── components/canvas-tools/SelectionBar/SelectionBar.tsx    # NEW
└── pages/world/WorldPage.tsx      # render SelectionBar in Select

apps/web/e2e/canvas-box-select.spec.ts   # NEW, canvas slice
docs/guides/                             # hidden walls, box select (existing canvas guide)
```

**Structure Decision**: The pure rules go in canvas-core, and the gestures
in two new engine systems. `systems/wall.rs` and `systems/token.rs` take
only small edits. The web keeps its one-bridge-per-kind shape, plus a tally
module. Nothing is added under `crates/thunderforge-server`.

## Complexity Tracking

None. The one structural choice, a group resource beside the single
selections rather than replacing them, keeps the change smaller and away
from spec 082's files.
