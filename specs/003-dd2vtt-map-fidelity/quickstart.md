# Quickstart: dd2vtt Map Fidelity & From-Scratch Map Editor Tooling

Validation scenarios below map to spec.md's Acceptance Scenarios. Run against a local dev stack (`docker compose up`, server, `apps/web` dev server) per this project's existing local-dev flow.

## Prerequisites

- `docker compose up` (Postgres + RustFS)
- Server running (`cargo run -p thunderforge`)
- `apps/web` dev server running
- A GM test account owning a world, and a second (player) account invited to it (for the mid-session sync scenarios)

## Scenario 1 — From-scratch map building (US1)

1. As the GM, create a new scene with no import — either truly empty or using `examples/maps/grassy-path-ambush.dd2vtt`'s background art as a starting point (this feature's gold-standard blank-canvas reference).
2. Select the wall tool, draw a wall.
3. Select the wall, toggle "Blocks vision" and "Blocks movement" independently in the property panel (`WallTool.tsx`) — confirm each checkbox's effect is visible immediately (occlusion/movement-blocking updates live).
4. Toggle the same wall's door state (Door select: none/open/closed) — confirm this is independent of the passability checkboxes and behaves as it does today (unchanged).
5. Place a light source ("torch") on the canvas — confirm it illuminates immediately.
6. As the player (second browser context/session), view the same scene: confirm the wall, its current passability, the door, and the torch's light are all visible, and confirm no wall/door/passability/torch authoring controls are shown to the player.
7. Back as the GM, with the player still connected, toggle the wall's "Blocks movement" checkbox again: confirm the player's view updates within a few seconds, no reload.

**Expected**: every step succeeds without a server restart, page reload, or import step. If step 3 or 7 fails to propagate correctly, that's the "confirm working, fix only if broken" gap this feature's plan anticipates might exist despite the code looking complete (research.md §1).

## Scenario 2 — Round-trip persistence (US2)

1. Import `examples/maps/road-side-in.dd2vtt` (richest fixture: 24 walls, 16 doors, 4 lights) into a fresh scene.
2. Record (e.g. via a GraphQL query or direct DB inspection) every wall's coordinates + door state + `blocks_vision`/`blocks_movement`, every light's position/properties, and the background image reference.
3. Reload the scene (page reload, or re-fetch from the server as a fresh session would) and re-record the same data.
4. Confirm exact equality between steps 2 and 3 — this is what the automated round-trip test (FR-008) checks in CI; this manual walkthrough is a sanity check, not a replacement for it.
5. Repeat with `examples/maps/dwarven-forge.dd2vtt` (walls-only, no doors/lights) to confirm the walls-only path is equally durable.
6. Hand-build additions from Scenario 1 on top of an imported scene, reload, confirm those survive too (spec.md US2 Acceptance Scenario 3).

**Expected**: 100% field match every time, per SC-003.

## Scenario 3 — Field-gap disclosure (US3)

1. Import `examples/maps/little-fish-academy.dd2vtt` (has a non-default `ambient_light`). Confirm the import response's `warnings` field mentions ambient light was not applied.
2. Import the new hand-crafted synthetic fixture exercising `freestanding` portals and `objects_line_of_sight` (research.md §7). Confirm both are called out in `warnings`.
3. Import any of the other fixtures in `examples/maps/` (e.g. `demo.dd2vtt`, `road-side-in.dd2vtt`). Confirm `warnings` is empty — no new noise for files that don't use these fields (FR-014, SC-004).

**Expected**: `warnings` is present, accurate, and non-empty only when relevant, per contracts/map-import-response.md.

## Scenario 4 — Regression check

Run the existing `apps/web/e2e/canvas-authoring.spec.ts` suite (wall/shape authoring from specs 001/002) and the full `cargo test` suite to confirm nothing in this feature's changes broke prior behavior — especially door-state toggling, which sits right next to the passability checkboxes in the same property panel.
