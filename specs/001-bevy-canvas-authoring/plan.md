# Implementation Plan: Native Canvas — Full tldraw Replacement (Walls, Map Import, Lighting, Shapes)

**Branch**: `001-bevy-canvas-authoring` | **Date**: 2026-08-20 (revised) | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-bevy-canvas-authoring/spec.md`

## Summary

Replace the wrapped tldraw whiteboard (ADR-004) with four independently
modular Bevy engine plugins — `WallPlugin`, `MapImportPlugin`,
`LightingPlugin`, and `ShapePlugin` — plus a new `CanvasLayerPlugin` that
gives all of them an explicit, shared z-order/visibility model (FR-016).
Walls already have a persisted backend (Phase 6: `walls` table +
`WallMutation`, extended here with door semantics); this feature adds new
backend entities (light sources, shapes) following the same
ownership-enforced GraphQL/Diesel pattern, a Universal VTT (`.dd2vtt`)
import pipeline that populates walls/lights/background from a file in one
shot (research.md §7-9, data-model.md), and the in-canvas authoring UI,
RxDB replication, and vision/light occlusion rendering for all of it.
`WorldWhiteboard.tsx` and the `tldraw` dependency are removed once
`ShapePlugin` is at full parity with tldraw's tool set — not before.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate, compiled to
`wasm32-unknown-unknown`; server crate, native); TypeScript/React for the
surrounding UI shell.

**Primary Dependencies**: Bevy 0.18 (engine/canvas), async-graphql + Diesel
+ PostgreSQL (server), RxDB (client-side replication/offline cache), Axum
(HTTP/WS transport, including its existing `multipart` feature — already a
dependency, reused for `.dd2vtt` file upload per the game-system-package
upload precedent in `src/server/src/systems.rs`), a JSON parser (`serde_json`,
already a dependency) and a PNG-capable image crate for validating/
re-encoding the imported background asset. No new third-party
canvas/whiteboard library — `tldraw` is being removed, not replaced with
an equivalent.

**Storage**: PostgreSQL via Diesel migrations (new `light_sources` and
`shapes` tables; `walls` extended with door fields; `scenes` extended with
a background-image reference). Imported background images are written to
the same on-disk/object storage location already used for other uploaded
assets (see the package-upload precedent in `systems.rs`), not embedded as
base64 in the database.

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
(ADR-032); a representative map import (`examples/maps/demo.dd2vtt`, ~4MB
with a background image, 8 wall polygons, 2 doors, 12 lights) completes in
under 30 seconds end-to-end (SC-007).

**Constraints**: Must respect Constitution Principle I (ECS owns canvas
simulation; React only renders chrome) and Principle II (each tool is an
independently addable/removable Bevy plugin). Must respect Principle III
(server-side ownership enforcement) for all new mutations, following the
existing `WallMutation` scene-ownership check as the template. Import must
not accept unbounded file sizes (edge case: "very large background
image") — enforce a request body size cap at the multipart handler,
consistent with how the existing package-upload endpoint bounds its input.

**Scale/Scope**: Per-scene entity counts in the tens-to-low-hundreds
(walls, lights, shapes combined) — same order of magnitude as existing
token counts, not a new scale regime. A single `.dd2vtt` import can
introduce dozens of walls/lights in one batch (demo.dd2vtt: 8 wall
polygons decomposing into dozens of segments, 12 lights) — within the
same order of magnitude, not a step-change requiring different indexing.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. ECS Owns Simulation | Walls/lights/shapes are authored, stored, and rendered as Bevy ECS entities/resources; React only supplies toolbar chrome and dispatches commands into the engine, mirroring the existing token/selection boundary. | PASS |
| II. Plugin-Modular Engine Architecture | Five new plugins (`WallPlugin`, `MapImportPlugin`, `LightingPlugin`, `ShapePlugin`, `CanvasLayerPlugin`), each with its own `systems/*` and `resources/*` module, addable/removable independently in `lib.rs`, matching the existing `SelectionPlugin`/`TokenPlugin` pattern. `MapImportPlugin` reads/writes `WallSet`/`LightSet` (owned by `WallPlugin`/`LightingPlugin`) but is itself still independently removable — a build without it simply has no import command; walls/lighting still work standalone. | PASS |
| III. Ownership & Authorization at the Data Boundary | New `light_sources` and `shapes` tables carry `created_by`/`updated_by`; `walls` gains door fields under the same existing check; the import endpoint applies the identical scene-ownership check as `mutations_walls.rs`, once for the whole batch rather than per row. | PASS |
| IV. Real ADRs and Specs Before Divergent Implementation | This spec + plan supersede ADR-004's tldraw decision; ADR-037 (`docs/adrs/20260820-037-native_canvas_authoring_supersedes_tldraw.md`) already records that decision. | PASS |
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

**Native-testable core (ADR-038)**: pure data/geometry for each
capability lives in `crates/thunderforge-canvas-core` (package
`thunderforge_canvas_core`) — **no Bevy/wasm-bindgen dependency**, only
`glam` (identical type to `bevy::prelude::Vec2`, zero-conversion). This
crate's tests run for real via native `cargo test`, unlike
`thunderforge_engine`'s wasm32-only tests, which only compile-check in
this environment. Built for walls already (`crates/thunderforge-canvas-core/src/wall.rs`,
18 passing tests); lighting and shapes follow the same pattern:

```text
crates/thunderforge-canvas-core/src/
├── lib.rs
├── wall.rs      # DONE — Wall, DoorState, WallEdit, WallSet, is_visible + tests
├── lighting.rs  # TODO — LightSource data + occlusion-aware illumination sampling + tests
└── shape.rs     # TODO — Shape data, undo-edit types + tests
```

```text
src/engine/src/
├── plugins/
│   ├── wall.rs            # DONE — WallPlugin: authoring input, door toggle, occlusion trigger
│   ├── map_import.rs       # NEW — MapImportPlugin: consumes parsed import batch, spawns
│   │                        #       wall/light/background entities
│   ├── lighting.rs         # NEW — LightingPlugin: light placement, occlusion-aware illumination
│   ├── shape.rs             # NEW — ShapePlugin: freehand/rect/ellipse/line/text draw+edit
│   └── canvas_layer.rs      # DONE — CanvasLayerPlugin: shared z-order + GM/player visibility
│                            #       resource consumed by all of the above (FR-016)
├── systems/
│   ├── wall.rs             # DONE — wall create/move/delete/door-toggle, vision recompute
│   ├── map_import.rs        # NEW — apply an already-parsed import batch to WallSet/LightSet
│   ├── lighting.rs          # NEW — light create/move/resize systems, occlusion sampling
│   ├── shape.rs              # NEW — shape draw/move/resize/restyle, undo stack, visibility
│   └── canvas_layer.rs       # DONE — layer ordering/visibility application
├── resources/
│   ├── wall.rs              # DONE — thin Bevy `Resource` newtype over
│   │                        #        `thunderforge_canvas_core::wall::WallSet` (Deref/DerefMut,
│   │                        #        no logic of its own — see ADR-038)
│   ├── lighting.rs           # NEW — same pattern: thin Resource newtype over
│   │                          #       `thunderforge_canvas_core::lighting::LightSet`
│   ├── shape.rs               # NEW — same pattern over `thunderforge_canvas_core::shape::ShapeSet`
│   └── canvas_layer.rs        # DONE — CanvasLayers resource (ordered layer list + visibility)
└── lib.rs                   # MODIFIED — register the five plugins (additive, no reordering
                              #             of existing plugins per Principle II)

src/server/src/graphql/
├── mutations_walls.rs        # EXISTING — extended with door fields (see data-model.md)
├── mutations_lighting.rs     # NEW — LightSourceMutation (create/update/delete)
├── mutations_shapes.rs       # NEW — ShapeMutation (create/update/delete)
└── queries/scene.rs          # MODIFIED — expose lightSources/shapes alongside walls;
                               #            scene payload includes background image reference

# New query/mutation types are registered into the QueryRoot/MutationRoot
# MergedObject tuples in src/server/src/graphql.rs (~L1633-1644) — NOT
# src/server/src/schema.rs, which is the unrelated Diesel table-macro file.

src/server/src/
└── map_import.rs             # NEW — Axum multipart handler (reuses the upload pattern in
                               #       systems.rs) + UVTT (format 0.3) JSON parser: decodes the
                               #       file, validates format version, writes the background
                               #       image under `state.directories.asset_directory`, and
                               #       bulk-inserts Wall/LightSource rows scaled to the scene's
                               #       grid — all inside one DB transaction (data-model.md)

src/server/migrations/
├── <ts>_create_light_sources_table/
├── <ts>_create_shapes_table/
├── <ts>_add_door_fields_to_walls/
└── <ts>_add_background_image_to_scenes/

apps/web/src/
├── engine/bevy/
│   └── useCanvasEngine.ts    # MODIFIED — no change to ownership boundary; still just mounts
├── engine/world/
│   └── sync/                 # MODIFIED — wire wall/light/shape commands into existing
│                              #            world-store dispatch, same shape as token commands
├── db/collections/
│   ├── worldWallsCollection.ts    # NEW — RxDB replication, mirrors worldTokensCollection.ts
│   ├── worldLightsCollection.ts   # NEW
│   └── worldShapesCollection.ts   # NEW
├── components/canvas-tools/       # NEW — modular toolbar/panel components, one per tool:
│   ├── WallTool/
│   ├── MapImportTool/             # file picker + progress/error UI for T-series import tasks
│   ├── LightingTool/
│   └── ShapeTool/
└── engine/tldraw/                 # REMOVED at end of feature (WorldWhiteboard.tsx + dep)

examples/maps/                     # NEW — .dd2vtt fixtures used by import parser tests and
                                    #       quickstart.md Scenario (demo.dd2vtt, README.md)
```

**Structure Decision**: Existing repository layout (Rust engine crate +
Rust server crate + React web app) is reused as-is; this feature adds
sibling plugin/system/resource modules to the engine crate (one directory
per capability, per Constitution Principle II), a single new server-side
module for import parsing/upload (reusing the existing multipart-upload
and asset-directory conventions rather than inventing new ones), and
sibling mutation/collection/component modules to the server and web app
following the exact pattern already established by walls/tokens. No new
top-level project or restructuring is introduced.

## Complexity Tracking

*No constitution violations — table intentionally omitted.*
