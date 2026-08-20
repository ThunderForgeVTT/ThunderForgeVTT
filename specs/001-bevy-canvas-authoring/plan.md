# Implementation Plan: Native Canvas Authoring (Walls, Lighting, Annotations)

**Branch**: `001-bevy-canvas-authoring` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-bevy-canvas-authoring/spec.md`

## Summary

Replace the wrapped tldraw whiteboard (ADR-004) with three independently
modular Bevy engine plugins — `WallPlugin`, `LightingPlugin`, and
`AnnotationPlugin` — that let a GM author walls, light sources, and
freeform annotations directly on the game canvas. Walls already have a
persisted backend (Phase 6: `walls` table + `WallMutation`); this feature
adds two new backend entities (light sources, annotations) following the
same ownership-enforced GraphQL/Diesel pattern, plus the in-canvas
authoring UI, RxDB replication, and vision/light occlusion rendering for
all three. `WorldWhiteboard.tsx` and the `tldraw` dependency are removed
once all three tools are at parity with what they replace.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate, compiled to
`wasm32-unknown-unknown`; server crate, native); TypeScript/React for the
surrounding UI shell.

**Primary Dependencies**: Bevy 0.18 (engine/canvas), async-graphql + Diesel
+ PostgreSQL (server), RxDB (client-side replication/offline cache), Axum
(HTTP/WS transport). No new third-party canvas/whiteboard library —
`tldraw` is being removed, not replaced with an equivalent.

**Storage**: PostgreSQL via Diesel migrations (new `light_sources` and
`annotations` tables, alongside the existing `walls` table).

**Testing**: `cargo test`/`cargo check --target wasm32-unknown-unknown` for
the engine crate (existing `integration_tests.rs` pattern), native
`cargo check`/`cargo test` for the server crate, Playwright e2e for the web
app (already introduced per the most recent commit on `main`).

**Target Platform**: Browser via WebAssembly (engine), server-side Linux
(Axum/Postgres backend).

**Project Type**: Web application (Rust/WASM game engine + React shell +
Rust GraphQL backend) — matches the existing repository structure, no new
top-level project.

**Performance Goals**: Vision/light recomputation and propagation to all
connected clients within a few seconds of an authoring edit (SC-001,
FR-003); no measurable frame-rate regression to existing token
rendering/selection at the engine's already-established 1000+ token target
(ADR-032).

**Constraints**: Must respect Constitution Principle I (ECS owns canvas
simulation; React only renders chrome) and Principle II (each tool is an
independently addable/removable Bevy plugin). Must respect Principle III
(server-side ownership enforcement) for all new mutations, following the
existing `WallMutation` scene-ownership check as the template.

**Scale/Scope**: Per-scene entity counts in the tens-to-low-hundreds
(walls, lights, annotations combined) — same order of magnitude as
existing token counts, not a new scale regime.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. ECS Owns Simulation | Walls/lights/annotations are authored, stored, and rendered as Bevy ECS entities/resources; React only supplies toolbar chrome and dispatches commands into the engine, mirroring the existing token/selection boundary. | PASS |
| II. Plugin-Modular Engine Architecture | Three new plugins (`WallPlugin`, `LightingPlugin`, `AnnotationPlugin`), each with its own `systems/*` and `resources/*` module, addable/removable independently in `lib.rs`, matching the existing `SelectionPlugin`/`TokenPlugin` pattern. | PASS |
| III. Ownership & Authorization at the Data Boundary | New `light_sources` and `annotations` tables carry `created_by`/`updated_by`; new GraphQL mutations reuse the scene-ownership check already implemented in `mutations_walls.rs`. | PASS |
| IV. Real ADRs and Specs Before Divergent Implementation | This spec + plan supersede ADR-004's tldraw decision; a superseding ADR entry will be added recording that decision at implementation time (Phase 1 output references it). | PASS (tracked) |
| V. Verify Before Claiming Done | Plan's Testing section specifies the correct per-crate check (wasm32 target for engine, native for server); quickstart.md defines the end-to-end validation scenario. | PASS |

No violations requiring Complexity Tracking justification.

## Project Structure

### Documentation (this feature)

```text
specs/001-bevy-canvas-authoring/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (GraphQL schema additions)
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src/engine/src/
├── plugins/
│   ├── wall.rs            # NEW — WallPlugin: authoring input, occlusion recompute trigger
│   ├── lighting.rs         # NEW — LightingPlugin: light placement, occlusion-aware illumination
│   └── annotation.rs       # NEW — AnnotationPlugin: freeform draw/erase, GM-only visibility
├── systems/
│   ├── wall.rs             # NEW — wall create/move/delete input systems, vision recompute
│   ├── lighting.rs          # NEW — light create/move/resize systems, occlusion sampling
│   └── annotation.rs        # NEW — stroke capture, undo stack, visibility filtering
├── resources/
│   ├── wall.rs              # NEW — WallSet resource (segments + spatial index)
│   ├── lighting.rs           # NEW — LightSet resource
│   └── annotation.rs         # NEW — AnnotationSet resource, per-session undo stack
└── lib.rs                   # MODIFIED — register the three plugins (additive, no reordering
                              #             of existing plugins per Principle II)

src/server/src/graphql/
├── mutations_walls.rs        # EXISTING — reference pattern, extended with delete_wall if
│                              #            not already present (see data-model.md)
├── mutations_lighting.rs     # NEW — LightSourceMutation (create/update/delete)
├── mutations_annotations.rs  # NEW — AnnotationMutation (create/update/delete)
└── queries/scene.rs          # MODIFIED — expose lightSources/annotations alongside walls

src/server/migrations/
├── <ts>_create_light_sources_table/
└── <ts>_create_annotations_table/

apps/web/src/
├── engine/bevy/
│   └── useCanvasEngine.ts    # MODIFIED — no change to ownership boundary; still just mounts
├── engine/world/
│   └── sync/                 # MODIFIED — wire wall/light/annotation commands into existing
│                              #            world-store dispatch, same shape as token commands
├── db/collections/
│   ├── worldWallsCollection.ts        # NEW — RxDB replication, mirrors worldTokensCollection.ts
│   ├── worldLightsCollection.ts       # NEW
│   └── worldAnnotationsCollection.ts  # NEW
├── components/canvas-tools/           # NEW — modular toolbar/panel components, one per tool:
│   ├── WallTool/
│   ├── LightingTool/
│   └── AnnotationTool/
└── engine/tldraw/                     # REMOVED at end of feature (WorldWhiteboard.tsx + dep)
```

**Structure Decision**: Existing repository layout (Rust engine crate +
Rust server crate + React web app) is reused as-is; this feature adds
sibling plugin/system/resource modules to the engine crate (one directory
triple per tool, per Constitution Principle II) and sibling
mutation/collection/component modules to the server and web app following
the exact pattern already established by walls/tokens. No new top-level
project or restructuring is introduced.

## Complexity Tracking

*No constitution violations — table intentionally omitted.*
