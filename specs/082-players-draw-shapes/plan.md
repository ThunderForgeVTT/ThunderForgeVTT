# Implementation Plan: Players Draw Shapes

**Branch**: `main` | **Date**: 2026-10-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/082-players-draw-shapes/spec.md`

## Summary

Select and Shapes become a player's by default, and a player owns what they
draw. The default lives in code; taking a tool away is a row in a new
revocations table ([R1](./research.md#r1-where-the-player-defaults-live),
[R2](./research.md#r2-revocations-in-their-own-table)). One synchronous
check, `shape_authority`, decides every shape write (R3). The GM gains a
bulk clear, `clearShapes`, and a picker fed by `shapeCreators` (R5, R6).

- **Server**:
  - a migration for `world_authoring_tool_revocations`;
  - `effective_authoring_tools` resolves defaults − revocations + grants;
  - `setAuthoringToolGrant` writes revocations for a default tool;
  - `authoringToolGrants` answers effective tools per member;
  - the three shape mutations use `shape_authority` and force a player's
    shape visible;
  - `clearShapes` and `shapeCreators` are new;
  - account deletion removes a deleted player's shapes in kept worlds (R9).
- **Engine**:
  - the core `Shape` carries `created_by`;
  - a `ViewerUserId` resource set by `set_viewer_user`;
  - the shape systems and context menu gate on the allow list and on
    `may_edit_shape` instead of `IsGameMaster` (R7).
- **Web**:
  - the rail renders for any member with tools (R8);
  - the shapes panel hides the visibility toggle from a player and shows
    the two clear actions to a DM;
  - a `clear_shapes` store command, sent by the shape mutation bridge;
  - the bridge rolls back a refused update or delete;
  - the text tool dispatches instead of calling the API;
  - the grants card shows the defaults ticked.
- **Demo**: the shape handlers move to `handlers/shapes.ts`, apply the same
  ownership rules for the tab's viewer, and answer `clearShapes`,
  `shapeCreators` and per-viewer `authoringTools` (R10).

What is deliberately not built: per-player shape colours, a batched delete
event, default access to walls, lights, tokens or interactions, and the
`walls`/`light_sources` account-deletion gap (R9).

## Technical Context

**Language/Version**: Rust 2021 (server, engine, canvas-core); TypeScript 5
(web, demo).

**Primary Dependencies**: `async-graphql`, Diesel, Bevy. No new dependency.

**Storage**: PostgreSQL. One new table; `shapes` is unchanged
(data-model.md).

**Testing**:

- `cargo test -p thunderforge-server`: tool resolution, revocation
  cascade, shape authority per role, forced visibility, `clearShapes`,
  `shapeCreators`, account deletion, the pause surface table;
- `cargo test -p thunderforge-engine` (host): `may_edit_shape`, the
  allow-list gate;
- Vitest in `apps/web`: `railTools`, the bridge's `clear_shapes` and
  rollback;
- Vitest in `apps/demo`: `handlers/shapes.test.ts`;
- Playwright: `apps/web/e2e/canvas-shapes-by-players.spec.ts` in the
  `canvas` slice, and a demo e2e for US6.

**Target Platform**: Linux server; Chromium/Firefox/WebKit; the demo on
the static host.

**Project Type**: web service + wasm engine + web app + demo app.

**Performance Goals**: SC-001, 1 s to every board; SC-003, a 200-shape
clear within 2 s on a local stack. T042 measures it.

**Constraints**: no client-side database (AGENTS.md). Components dispatch
into the world store; only the mutation bridge sends GraphQL. The engine's
checks are a courtesy; the server's are the rule (FR-019).

**Scale/Scope**: one migration, two new GraphQL fields, three changed
mutations, one changed query, six engine gates, one new store command.

## Constitution Check

| Principle                   | How this plan meets it                                                                                                                                                                                                                                                                   |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. Server is the authority  | Every write goes through `shape_authority`; the engine and rail only hide what the server would refuse.                                                                                                                                                                                  |
| II. Circular flow           | A clear removes nothing locally; shapes leave on the server's `deleted` events.                                                                                                                                                                                                          |
| III. Agnostic core          | `created_by` is added to canvas-core's `Shape` with no server or network type.                                                                                                                                                                                                           |
| IV. Tests first             | Each phase opens with its failing tests.                                                                                                                                                                                                                                                 |
| V. Docs by audience         | `docs/guides/lights-and-drawings.md` for players and GMs; CONTRIBUTING for the shape authority rule.                                                                                                                                                                                     |
| VI. Proven by its own slice | Slice `canvas`; own spec `canvas-shapes-by-players.spec.ts` (the `canvas-` prefix); neighbour `scene-management.spec.ts`. The gate before merge is that slice plus every slice `pnpm e2e:which --diff` names (the owner's decision, 2026-10-07). |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/082-players-draw-shapes/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql-shapes.md
│   └── engine-viewer.md
└── tasks.md
```

### Source Code (touched)

```text
crates/thunderforge-server/
├── migrations/2026-10-07-110000-0000_authoring_tool_revocations/{up,down}.sql   (new)
├── src/schema.rs
├── src/models.rs                                  (revocation row)
├── src/auth/authoring_tools.rs                    (defaults, sync resolver)
├── src/auth/shape_authority.rs                    (new)
├── src/auth/mod.rs
├── src/graphql/mutations_authoring_tools.rs
├── src/graphql/queries/authoring_tools.rs
├── src/graphql/mutations_shapes.rs                (authority, clearShapes)
├── src/graphql/queries/shape_creators.rs          (new)
├── src/graphql/play_pause_surface_tables.rs
└── src/users/mod.rs                               (account deletion)

crates/thunderforge-canvas-core/src/shape.rs        (created_by)
crates/thunderforge-engine/src/
├── payloads.rs                                    (createdBy, SetViewerUser)
├── sdk.rs, app.rs                                 (set_viewer_user)
├── systems/shape.rs                               (may_edit_shape, gates)
└── plugins/context_menu.rs

apps/thunderforge/schema.graphql                   (regenerated)

apps/web/src/
├── lib/authoringTools.ts                          (railTools)
├── api/authoringTools.ts, api/shapes.ts           (clearShapes, shapeCreators)
├── engine/bevy/index.ts                           (setViewerUser)
├── engine/world/types.ts, sync/shapes.ts          (createdBy, clear_shapes, rollback)
├── components/canvas-tools/ShapeTool/ShapeTool.tsx
├── components/canvas-tools/ShapeTool/ClearShapesDialog.tsx   (new)
├── pages/world/WorldPage.tsx
└── pages/world/settings/AuthoringToolGrantsCard.tsx

apps/web/e2e/canvas-shapes-by-players.spec.ts      (new)

apps/demo/src/backend/
├── handlers.ts
├── handlers/shapes.ts                             (new)
└── handlers/shapes.test.ts                        (new)
apps/demo/e2e/shapes-by-players.spec.ts            (new)

docs/guides/lights-and-drawings.md
docs/CONTRIBUTING.md
```

## Complexity Tracking

None. The one structural addition, a second table beside the grants, is
justified in R2.
