# Implementation Plan: Scene Management Overhaul

**Branch**: `022-scene-management-overhaul` | **Date**: 2026-08-24 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/022-scene-management-overhaul/spec.md`

## Summary

Give scenes a real home: a new "Scenes" section (alongside NPCs/Lore/Items/Abilities in the world sidebar) where a GM creates scenes, imports dd2vtt maps (reusing the existing importer), writes a Markdown summary (reusing the Lore editor/renderer), and controls per-scene visibility with a hidden-by-default flag. Players get a table of visible scenes and a detail view with a server-generated preview thumbnail. A world-level default grid type (None/Squares/Hexagons) seeds new scenes, and "None" is a real gridless mode (no snap, pixel-based measurement), configured from a relabeled System Settings page. The GM's only play action on a scene is "Launch," which extends the existing `world_events`/WebSocket live-sync transport (already used for tokens/walls/lights/shapes) to make scene selection server-authoritative and push live scene switches to everyone already in Play.

## Technical Context

**Language/Version**: Rust 2024 edition (server, `src/server`), Rust compiled to WASM (engine, `src/engine`), TypeScript 6.0 / React (frontend, `apps/web`)

**Primary Dependencies**: Axum 0.8 + async-graphql 7.2 + Diesel 2.3/PostgreSQL (server); Bevy (WASM) for canvas simulation (engine); React + Radix-based design system, CodeMirror (`@uiw/react-codemirror`) for Markdown editing, `graphql-ws` for the live subscription client (frontend); `image` crate (server) for thumbnail generation, already used by `storage/transcode.rs`

**Storage**: PostgreSQL via Diesel migrations (`src/server/migrations/`); scene map images and generated thumbnails stored via the existing RustFS-backed object storage used for lore/canvas assets

**Testing**: `cargo test` (server, includes resolver-level GraphQL tests per existing `*_tests.rs` convention); Playwright e2e (`apps/web/e2e/*.spec.ts`) as the project's primary frontend test layer (no unit test runner — e.g. vitest — is configured); `cargo check --target wasm32-unknown-unknown` for any engine crate change (Constitution Principle V)

**Target Platform**: Server-side web (Linux), browser client (WASM canvas + React shell)

**Project Type**: Web application — Rust/GraphQL backend (`src/server`) + Bevy/WASM engine (`src/engine`) + React frontend (`apps/web`), existing monorepo layout

**Performance Goals**: Live scene-switch propagation to all connected Play clients within seconds (SC-006) — achieved via the already-live `world_events` push transport, not polling; scene preview thumbnails must load and render inside the Scenes table/detail view noticeably faster than the full-resolution map (SC-005)

**Constraints**: Server remains authoritative for ownership/visibility (Constitution Principle III) — hidden-state filtering, launch authorization, and summary/import mutations are all GM/Owner-gated server-side, not just hidden in the UI; canvas simulation state (which scene is loaded, grid/snap/measurement behavior) stays owned by the Bevy engine (Principle I), not reimplemented in React

**Scale/Scope**: Single-world scoped feature (no cross-world concerns); scenes per world in the tens, not thousands, based on existing usage patterns

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design below.*

| Principle | Assessment |
|---|---|
| I. ECS Owns Simulation, React Owns Chrome | **Pass.** "Which scene is loaded" and grid/snap/measurement behavior remain engine-owned; React only observes `selectedSceneId` (now server-synced via the live subscription instead of purely local state) and passes it to the existing engine bridge loaders. No new simulation logic moves into React components. |
| II. Plugin-Modular Engine Architecture | **Pass, with a scoped task flag.** No new Bevy plugin is required for CRUD/summary/hidden/thumbnail work (that's all server+React). The grid-type "None" behavior (disable snap, pixel-unit measurement) touches whichever existing plugin(s) already own grid/measurement systems — this must be implemented within that plugin's existing module boundary, not as ad hoc logic elsewhere, and is called out as its own task in `research.md`'s Outstanding Technical Risk. |
| III. Ownership & Authorization at the Data Boundary | **Pass, by design.** Every new/changed mutation (`createScene` default grid type, `updateScene` summary/hidden, dd2vtt import wiring, `launchScene`, `updateWorldDefaultSceneGridType`) is GM/Owner-gated server-side following the existing `is_dm_of_world`/owner-check convention — see data-model.md and contracts/. |
| IV. Real ADRs and Specs Before Divergent Implementation | **Action required, tracked.** This spec satisfies the Spec Kit requirement. The new server-authoritative "active scene" concept plus its live-broadcast event is an architecturally significant decision (new ownership boundary — see research.md §6) and **requires an ADR under `docs/adrs/`** landing in the same change set as its implementation. This is called out explicitly so `/speckit-tasks` includes an ADR-authoring task before/alongside the live-launch work. |
| V. Verify Before Claiming Done | **Pass, by process.** Server changes verified with `cargo check`/`cargo test`; any engine-crate change (grid-type "None" behavior) verified with `cargo check --target wasm32-unknown-unknown`; frontend changes verified with `tsc`/build and exercised in a running dev instance, consistent with how prior features in this session were verified. |

No unjustified violations — Complexity Tracking section is not needed.

## Project Structure

### Documentation (this feature)

```text
specs/022-scene-management-overhaul/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── scene-management.graphql.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   ├── <ts>_add_scene_summary_and_hidden/{up,down}.sql       # scenes: summary_markdown, summary_rendered_html, hidden
│   ├── <ts>_add_world_default_scene_grid_type/{up,down}.sql  # worlds: default_scene_grid_type
│   └── <ts>_add_world_active_scene/{up,down}.sql             # worlds: active_scene_id
├── src/
│   ├── schema.rs                    # scenes + worlds column additions
│   ├── models.rs                    # Scene/NewScene/SceneUpdate, World field additions
│   ├── world_events.rs              # new event code for "scene launched"
│   ├── markdown/                    # reused as-is for scene summary rendering
│   ├── storage/transcode.rs         # reused as-is for scene preview thumbnails
│   ├── map_import/                  # reused as-is (dd2vtt importer); no changes expected
│   ├── graphql/
│   │   ├── queries/scene.rs         # add hidden-aware filtering (GM vs player), active_scene_id on world
│   │   ├── mutations_scenes.rs      # NEW: create/update-summary/toggle-hidden/launchScene (file may not exist yet — verify during tasks)
│   │   └── input_types.rs           # new input types for the above
│   └── routes (main.rs / asset routes) # new scene-thumbnail serve route mirroring lore_assets_serve.rs
└── docs/adrs/
    └── <next>-server-authoritative-active-scene.md   # required by Constitution Principle IV (see Constitution Check)

apps/web/src/
├── layouts/world-layout/
│   ├── WorldSidebarNav.tsx          # add "Scenes" category
│   └── WorldStagingPage.tsx         # remove SceneSwitcher/scene-management UI
├── pages/world/
│   ├── scenes/                      # NEW: ScenesRoutePage, ScenesGmView, ScenesPlayerTable, SceneDetailView
│   └── WorldPage.tsx                # extend subscribeToWorldEvents handling with scene-launch event → setSelectedSceneId
├── engine/world/sync/               # new applySceneWorldEvent-style handler alongside existing wall/token/light/shape handlers
├── api/
│   ├── scenes.ts                    # add updateScene, launchScene, hidden/summary fields to SCENE_FIELDS
│   └── world.ts                     # add updateWorldDefaultSceneGridType
└── pages/world/settings/
    └── WorldSystemSettingsPage.tsx  # relabel heading, add Default Scene Grid Type control
```

**Structure Decision**: Follows the existing monorepo layout exactly (`src/server` Rust/GraphQL backend, `src/engine` Bevy/WASM, `apps/web` React frontend) — no new top-level project or directory convention is introduced. New backend logic lands in existing modules (`graphql/`, `map_import/` untouched, `storage/`, `markdown/`) rather than new subsystems, per the "reuse over rebuild" findings in research.md.
