# Implementation Plan: Hand-Drawn Authoring & Per-Campaign Asset Storage

**Branch**: `002-canvas-authoring-asset-storage` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-canvas-authoring-asset-storage/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Two independent halves. (1) Close T067: `WallPlugin`/`ShapePlugin` already implement
hand-drawn wall and shape authoring in the Bevy engine (`src/engine/src/plugins/wall.rs`,
`shape.rs`); the remaining work is Playwright e2e coverage for the two scenarios T067
left open (hand-drawn wall + cross-session vision occlusion; hand-drawn shapes + GM/player
visibility), plus closing two known gaps flagged in code comments (ellipse renders as a
rect placeholder; text has no in-canvas entry system yet). (2) Net-new: a self-hosted
RustFS object-storage service, added to `compose.yml`, fronted by a server-minted
short-lived STS-style credential scoped to `{owner_user_id}/{world_id}/*`, used for both
newly-pasted canvas images (auto-transcoded to WebP server-side) and the existing
map-import background-image path (migrated onto the same mechanism per FR-018), with
write authorization gated on `world_members` (owner or accepted invite) via a new shared
guard function — the first non-`mutations_invites.rs` consumer of that table.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate → `wasm32-unknown-unknown` via
Bevy 0.18; server crate native), TypeScript/React (`apps/web`)

**Primary Dependencies**: Bevy 0.18 (engine, `webp`/`png` features already enabled);
Axum 0.8 + `async-graphql` 7.2 + Diesel 2.3/PostgreSQL (server, existing); new:
`aws-sdk-s3` + `aws-sdk-sts` (or equivalent S3-compatible client) for RustFS access and
STS `AssumeRole`-style credential minting; `image` crate (`webp` encode feature) for
server-side transcoding; RustFS (new self-hosted object storage service)

**Storage**: PostgreSQL (existing, new `canvas_image_assets` table + migration of
`scenes.background_image_path`) + RustFS (new, S3-compatible object storage for asset
bytes)

**Testing**: `cargo test` (server, native); Playwright (`apps/web/e2e/`, existing
`canvas-authoring.spec.ts` harness — this feature extends it for T067's open scenarios
plus new paste/RBAC scenarios); engine crate has no native `cargo test` target
(wasm32-only per constitution Principle V — verify via `cargo check --target
wasm32-unknown-unknown`)

**Target Platform**: Linux server (Axum + RustFS, both containerized via `compose.yml`)
+ WASM in-browser (Bevy engine)

**Project Type**: Web application — `src/engine` (WASM canvas engine) + `src/server`
(Rust backend) + `apps/web` (React frontend), existing three-part structure

**Performance Goals**: SC-001 wall-authoring interaction completes within 10s; SC-004
pasted image appears on canvas within 10s under normal network conditions

**Constraints**: Reuse existing 50MB upload ceiling (`MAX_UPLOAD_BYTES`,
`map_import.rs`) for pasted images (FR-013); credentials must be short-lived
(target 15 min TTL) and scoped to one world prefix, never the RustFS root credential
(FR-017); zero additional manual local-dev setup steps beyond the existing
`docker compose up` (FR-020/SC-008)

**Scale/Scope**: Additive to existing per-world/per-scene data model; no new entity
above "world" (Assumptions); asset lifecycle/cleanup explicitly out of scope

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. Wall/shape authoring is already fully
  ECS-owned (`WallPlugin`/`ShapePlugin`); this feature adds no React-side simulation
  state. Pasted-image placement follows the existing `BackgroundPlugin`/
  `sync_scene_background` pattern (`src/engine/src/systems/background.rs`) —
  image-as-canvas-entity stays engine-owned; React only adds the paste-trigger UI
  affordance and toolbar chrome, not the placed-image state itself.
- **Principle II (Plugin-modular engine)**: PASS. No new engine plugin is strictly
  required for US1/US2 (existing `WallPlugin`/`ShapePlugin` already cover the surface);
  if the ellipse/text gaps found in Phase 0 need engine changes, they extend the
  existing `shape.rs` plugin's systems rather than introducing new coupling. Any new
  pasted-image-entity spawning extends the existing `BackgroundPlugin`-style pattern
  as its own resource/system pair, not a new cross-cutting plugin.
- **Principle III (Ownership/authorization at the data boundary)**: PASS, and this
  feature is the first place `world_members` becomes the enforcement source for a
  mutation outside `mutations_invites.rs` (see research.md §7) — consistent with, not
  a deviation from, this principle. New `canvas_image_assets` rows carry
  `created_by`/`updated_by` per existing convention.
- **Principle IV (ADRs before divergent implementation)**: GATE — a new ADR covering
  the RustFS/STS-credential decision (research.md §2-3) MUST land alongside this
  feature's implementation, since it's a new subsystem and a new ownership-boundary
  mechanism (world-scoped storage credentials). Not yet written as of this plan;
  tracked as a Phase 1 deliverable alongside code, per constitution wording ("MAY
  proceed in parallel with drafting").
- **Principle V (Verify before done)**: PASS — plan calls out `cargo check --target
  wasm32-unknown-unknown` for engine changes, native `cargo check`/`cargo test` for
  server changes, and running Playwright e2e for the UI-affecting paste/wall/shape
  flows, matching existing project verification practice.

No unjustified violations. Complexity Tracking left empty.

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
compose.yml                          # + new `rustfs` service (FR-020)
.env                                 # + RustFS root credentials/bucket config

src/engine/src/
├── plugins/wall.rs, shape.rs        # existing — verify/close ellipse+text gaps only
├── plugins/background.rs            # existing pattern to mirror for pasted-image entities
├── systems/wall.rs, shape.rs        # existing authoring systems
└── systems/background.rs            # existing AssetServer::load pattern

src/server/src/
├── graphql/mutations_walls.rs       # existing, unchanged
├── graphql/mutations_shapes.rs      # existing, unchanged
├── graphql/mutations_invites.rs     # existing world_members query pattern to generalize
├── graphql/mutations_assets.rs      # NEW — uploadCanvasImage, canvasImageAssetsForScene
├── auth/world_membership.rs         # NEW — shared require_world_member guard (research.md §7)
├── storage/rustfs.rs                # NEW — STS AssumeRole credential minting, S3 client
├── map_import.rs                    # migrate save_background_image onto storage/rustfs.rs
├── schema.rs                        # + canvas_image_assets table
├── models.rs                        # + CanvasImageAsset model
└── migrations/                      # + new migration: canvas_image_assets, background_image_path FK swap

apps/web/src/components/canvas-tools/
├── WallTool/, ShapeTool/            # existing — no structural change expected
└── AssetPasteTool/                  # NEW — paste-to-canvas trigger, follows existing per-tool dir pattern

apps/web/e2e/
└── canvas-authoring.spec.ts         # extend: T067's open Scenario 1/4, plus paste + RBAC scenarios

docs/adrs/
└── YYYYMMDD-0NN-rustfs_scoped_asset_storage.md   # NEW, required by Constitution Principle IV gate
```

**Structure Decision**: Existing three-part web-application layout (`src/engine` WASM
canvas engine, `src/server` Rust/Axum/GraphQL backend, `apps/web` React frontend) is
reused as-is; this feature adds one new backend module group (`storage/`, `auth/`,
`graphql/mutations_assets.rs`), one new frontend tool directory
(`AssetPasteTool/`, matching the existing `WallTool/`/`ShapeTool/` convention), and one
new infra service in the existing single `compose.yml`. No new top-level directory or
build target is introduced.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
