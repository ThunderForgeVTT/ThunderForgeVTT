# Implementation Plan: Universal VTT (.dd2vtt) Map Import Fidelity & From-Scratch Map Editor Tooling

**Branch**: `003-dd2vtt-map-fidelity` | **Date**: 2026-08-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-dd2vtt-map-fidelity/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

This plan turned out much smaller than spec.md's User Story 1 implied. Research confirmed the wall-passability capability User Story 1 asks for is **already fully built and wired end-to-end**: `walls.blocks_vision`/`walls.blocks_movement` are independent DB columns (`schema.rs:313-328`), `update_wall`'s GraphQL input already accepts both independently (`input_types.rs:87-101`, `mutations_walls.rs:41-42`), and `WallTool.tsx:101-121` already renders two independent checkboxes bound to them on wall selection, plus a `B` keyboard shortcut in the engine (`wall.rs:334`). Torch (light-source) placement is likewise already fully built (spec 001). So this feature's real work is: (1) **verify** the existing passability/torch capabilities actually hold up live, end-to-end, mid-session, with a second connected player (User Story 1 is a verification task, like spec 002's T014 found for walls — not a build task); (2) build the round-trip persistence verification that has never existed (User Story 2); (3) add field-skip disclosure to the `.dd2vtt` import result, which is genuinely new (User Story 3). No schema migration, no new GraphQL mutation, and likely no new engine plugin are needed anywhere in this feature.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate → `wasm32-unknown-unknown` via Bevy 0.18; server crate native), TypeScript/React (`apps/web`) — unchanged from specs 001/002.

**Primary Dependencies**: Existing stack only — Bevy 0.18 (engine), Axum + `async-graphql` + Diesel/PostgreSQL (server), Playwright (`apps/web/e2e/`). No new dependency is anticipated by any user story.

**Storage**: PostgreSQL (existing `walls`, `light_sources`, `scenes`, `shapes`, `tokens`, `canvas_image_assets` tables) — **no schema change**. User Story 1 needs none (data model already supports it); User Story 2 is read-only verification; User Story 3 only changes the shape of the *response* of the existing import endpoint (adds a `warnings` field), not persisted schema.

**Testing**: `cargo test` (server, native — the round-trip verification and import-warnings tests land here), Playwright (`apps/web/e2e/`, extending `canvas-authoring.spec.ts` and/or a new `map-editor-tooling.spec.ts` for User Story 1's live verification), `cargo check --target wasm32-unknown-unknown` (engine, expected to need zero or near-zero changes).

**Target Platform**: Linux server + WASM in-browser (unchanged).

**Project Type**: Web application — existing `src/engine` / `src/server` / `apps/web` three-part layout, unchanged.

**Performance Goals**: SC-001 (empty scene → walls/door/passable segment/torch, by hand) in "a few minutes"; SC-002 (mid-session change visible to a connected player) within the same few-seconds window already achieved by existing wall/shape/light sync — no new performance work implied, just confirmation the existing bar is met.

**Constraints**: Zero schema/migration footprint (a design outcome of research, not an artificial limit); reuse the existing scene-owner GraphQL authorization filter verbatim (Constitution Principle III) rather than introducing a parallel check; the round-trip verification must be deterministic in CI (spec.md SC-006 / Edge Cases: distinguish infra flakiness — e.g. RustFS unreachable — from a real fidelity bug).

**Scale/Scope**: Builds directly on specs 001 (wall/light/shape data model, hand-drawn authoring engine plugins) and 002 (canvas image asset storage, GM-only enforcement pattern, `world_members`) — both merged to `main`. Five new fixture files added to `examples/maps/` this session serve as this feature's primary test/reference data.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. Wall passability and torch placement are already fully engine-owned (`WallPlugin`, `LightingPlugin`); User Story 1 adds no new React-side simulation state — at most a UI affordance (e.g. a right-click entry point) that still routes through the existing `updateSelectedWall`/engine-selection path, not a new store.
- **Principle II (Plugin-modular engine)**: PASS, trivially — no new plugin is anticipated. If verification (Phase where User Story 1 is exercised live) finds a real gap, any fix extends `WallPlugin`/`LightingPlugin`'s existing systems, per the same "confirm working, fix only if broken" pattern spec 002's T014/T018 used successfully.
- **Principle III (Ownership/authorization at the data boundary)**: PASS. All three user stories reuse existing, already-enforced scene-owner checks (`mutations_walls.rs`, `mutations_lighting.rs`) or add a read-only verification (User Story 2) / a response-shape addition to an already-owner-gated endpoint (User Story 3, `map_import.rs`'s `import_uvtt`) — no new authorization surface is introduced.
- **Principle IV (ADRs before divergent implementation)**: PASS, no ADR needed — unlike spec 002 (new RustFS subsystem, new ownership-boundary mechanism), this feature introduces no new subsystem, dependency, or ownership boundary. It is verification + a response-shape addition on top of infrastructure already documented by spec 001/002's ADRs.
- **Principle V (Verify before done)**: Plan calls for `cargo check --target wasm32-unknown-unknown` (engine), native `cargo check`/`cargo test` (server), and live Playwright verification (User Story 1 explicitly, since its whole premise is "does the already-built thing actually work end-to-end with a second connected session").

No violations. Complexity Tracking left empty.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
examples/maps/                         # + 5 new fixtures (already added), README updated
├── grassy-path-ambush.dd2vtt          # gold-standard blank-canvas fixture (US1)
├── azheim-meeting.dd2vtt              # smaller blank-canvas fixture
├── road-side-in.dd2vtt                # richest real fixture (US2 round-trip)
├── dwarven-forge.dd2vtt               # walls-only dungeon
└── little-fish-academy.dd2vtt         # non-default ambient_light (US3)

src/engine/src/
├── plugins/wall.rs, systems/wall.rs   # US1 — verify passability toggle live; fix only if broken
└── plugins/lighting.rs, systems/lighting.rs  # US1 — verify torch placement live; fix only if broken

src/server/src/
├── graphql/mutations_walls.rs         # US1 — already exposes blocks_vision/blocks_movement; no change expected
├── graphql/mutations_lighting.rs      # US1 — already exposes light placement; no change expected
├── map_import.rs                      # US2 (round-trip covers import path) + US3 (add `warnings` to import response)
├── test_support.rs                    # US2 — reuse insert_test_user/world/scene fixtures (spec 002 convention)
└── (new) round-trip test module(s)    # US2 — likely inline #[cfg(test)] in map_import.rs / a new tests file, following the mutations_assets.rs "create → re-fetch → assert equality" pattern

apps/web/src/components/canvas-tools/
└── WallTool/WallTool.tsx              # US1 — already has independent blocksVision/blocksMovement checkboxes; verify, and only add a right-click entry point if verification/UX review finds the existing select-then-panel flow insufficient

apps/web/e2e/
└── canvas-authoring.spec.ts (or new map-editor-tooling.spec.ts)  # US1 — live verification: passability toggle + torch placement + mid-session second-session sync
```

**Structure Decision**: Existing three-part layout (`src/engine`, `src/server`, `apps/web`) reused as-is, per specs 001/002 precedent. This feature adds no new backend module, no new engine plugin, and no new top-level directory — the bulk of the work is verification tests plus one response-shape addition (`warnings` on the import endpoint) and possibly one small UI affordance addition, not new subsystems.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
