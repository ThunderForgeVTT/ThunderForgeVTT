# Quickstart: Validating Scene Management Overhaul

Prerequisites: `make dev` running (postgres + rustfs + backend + frontend), migrations applied, a registered account.

## 1. Default grid type + relabeled System Settings (User Stories 3 & 4)

1. Create a world, open **System Settings** (`/world/:id/settings/system`).
2. Confirm the section heading reads **"System Settings"** (not "Change system"/"Assign a system"), the game-system picker is labeled **"Change System"**, and a **"Default Scene Grid Type"** control offers None/Squares/Hexagons.
3. Set it to **Hexagons**, then go create a new scene from the Scenes section (step 2 below) — it should start with a Hexagons grid.

## 2. GM scene management (User Story 1)

1. Open the **Scenes** section from the world sidebar nav (new entry, next to Session Setup).
2. Create a scene named "Ambush at the Bridge." Confirm it appears in the GM's list and, per the hidden-by-default clarification, is **not** visible in a second browser/session logged in as a non-GM member.
3. Import a `.dd2vtt` file into the scene (reuses the existing importer — any valid UVTT export works). Confirm the scene's background/walls/lights update.
4. Write a Markdown summary ("A rope bridge over a chasm, fog rolling in.") using the CodeMirror Markdown editor and save. Confirm the rendered (not raw) Markdown shows when viewing the scene.
5. Toggle the scene's **hidden** switch off. Confirm it now appears in the non-GM member's Scenes table (step 3 below).

## 3. Player browsing (User Story 2)

1. As a non-GM world member, open the Scenes section. Confirm the table lists only non-hidden scenes.
2. Click "Ambush at the Bridge." Confirm the detail view shows the rendered summary and a small preview thumbnail of the map (noticeably smaller/faster-loading than the full map).
3. Have the GM toggle the scene back to hidden; refresh the table and confirm it disappears.

## 4. Launch + live scene switch (the clarified Play/Launch behavior)

1. As the GM, open `/world/:id/play` in one browser tab, and have a second world member (or a second browser profile) also open `/world/:id/play`.
2. With nothing yet launched, confirm both tabs show the empty/unloaded canvas state (Play is accessible, just empty) rather than an error.
3. From the Scenes section, click **Launch** on "Ambush at the Bridge." Confirm both Play tabs load that scene's map/tokens/walls/lights within a few seconds, with no manual refresh or rejoin (SC-006).
4. Create a second scene and Launch it while both tabs are still open. Confirm both tabs immediately unload the first scene and load the second.
5. Confirm a non-GM member cannot trigger Launch (control absent or rejected server-side).

## 5. Grid type "None" behavior

1. Create a scene with grid type **None** (either via the world default or an explicit override).
2. In Play, confirm no grid lines render, tokens move freely without snapping, and the measurement tool reports raw pixel distances rather than grid-unit distances.

## Automated coverage expectations (for tasks phase)

- `cargo test` coverage for: `launchScene` authorization + event emission, `updateWorldDefaultSceneGridType` authorization, `createScene` default-grid-type inheritance, `scenes` query hidden-filtering (GM vs player), summary Markdown render-on-write.
- Playwright e2e coverage for: the Scenes section CRUD flow, hidden-toggle visibility filtering, System Settings relabel + default grid type control, and the multi-tab live-launch flow (two `browser.newContext()` pages, matching the existing multi-context pattern in `system-settings.spec.ts`/`world-compendium.spec.ts`).
