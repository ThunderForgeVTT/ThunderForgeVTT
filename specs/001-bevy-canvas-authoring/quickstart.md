# Quickstart: Validating Native Canvas — Full tldraw Replacement

Prerequisites: dev database migrated (`diesel migration run` from
`src/server/`), server running (`cargo run` in `src/server/`), web app
running (`pnpm dev` or equivalent in `apps/web/`), two browser sessions —
one logged in as the scene's GM/owner, one as a non-owner player who is a
world member. Scenario 2 additionally needs `examples/maps/demo.dd2vtt`
from this repo.

## Scenario 1 — Walls (and doors) block vision and movement (User Story 1, P1)

1. As the GM, open a scene with at least two tokens (one controllable by
   the player session).
2. Select the wall tool; drag a segment across the middle of the scene.
3. Confirm the wall persists after reloading the GM's page.
4. In the player session, confirm the area on the far side of the wall is
   no longer visible from the player's token.
5. As the GM, toggle the wall's "blocks vision" flag off; confirm the
   player's view updates within a few seconds without a reload.
6. As the GM, mark the wall as a door and set it open; confirm the
   player's view updates as if the wall were absent, then set it closed
   again and confirm vision is blocked again (FR-017).
7. As the GM, delete the wall; confirm the player's view returns to
   pre-wall visibility.
8. Confirm the player session at no point shows wall-editing handles or a
   wall tool (FR-009).

**Pass condition**: all confirmations hold; SC-001 (<5s propagation)
observed on step 5.

## Scenario 2 — Import a Universal VTT map (User Story 2, P2)

1. As the GM, create a new, empty scene.
2. Use the map-import tool to upload `examples/maps/demo.dd2vtt`.
3. Confirm the scene's background now shows the imported art, scaled to
   fill the scene's grid correctly (no stretching/cropping mismatch).
4. Confirm wall segments exist matching the file's 8 `line_of_sight`
   polygons, and that they block player vision the same as a hand-drawn
   wall would (repeat Scenario 1 steps 4-5 using an imported wall instead
   of a hand-drawn one).
5. Confirm 2 door-flagged walls exist, in the open/closed state recorded
   in the file, and behave per Scenario 1 step 6.
6. Confirm 12 light sources exist, positioned per the file, and that at
   least one is occluded correctly by an imported wall (SC-002-style
   check, using imported content instead of hand-placed).
7. As the GM, edit one imported wall's `blocksVision` flag and delete one
   imported light using the ordinary wall/lighting tools; confirm both
   operations succeed exactly as they would on hand-authored content
   (FR-026).
8. Attempt to import a file with a fabricated unsupported `format` value
   (e.g. edit a copy of `demo.dd2vtt` and change `"format": 0.3` to
   `"format": 9.9`); confirm the import is rejected with a clear error and
   the scene is unchanged (FR-024).

**Pass condition**: SC-007 (<30s end-to-end) observed on steps 2-6;
FR-018 through FR-026 all hold.

## Scenario 3 — Lighting occluded by walls (User Story 3, P3)

1. As the GM, draw two walls forming a closed room boundary with one
   opening, on a scene separate from Scenario 2 (to isolate hand-authored
   lighting from imported lighting).
2. Place a light source inside the room via the lighting tool.
3. Confirm the room is illuminated for the player session and an adjacent,
   wall-separated area remains dark (SC-002).
4. Attach the light to a token (drag the token in; confirm the light
   follows it as the GM moves the token).
5. Delete the light; confirm the room returns to its prior (dark/default)
   state for the player.

**Pass condition**: illumination respects wall occlusion; token-attached
light follows movement; deletion cleanly reverts.

## Scenario 4 — Shapes/annotations, full tldraw parity (User Story 4, P4)

1. As the GM, draw one of each shape type — freehand stroke, rectangle,
   ellipse, line/arrow, and text label — on the scene using the native
   draw tool (not tldraw).
2. Reload the GM's page; confirm all five persist.
3. Move and restyle (color/line weight) one shape; confirm the change
   persists.
4. Mark one shape as visible-to-players; confirm the player session now
   sees it but cannot select/move/delete it.
5. Confirm another (GM-only) shape is never sent to/rendered in the
   player session.
6. Delete all shapes as the GM; confirm removal on both sessions where
   applicable.
7. With Scenarios 1-4 all passing, grep the `apps/web` source tree for
   `tldraw`; confirm zero matches, then confirm the `tldraw` package is
   absent from `apps/web/package.json` (SC-006).

**Pass condition**: FR-007/FR-008/FR-009 all hold across all five shape
kinds; no tldraw references remain anywhere in the codebase.

## Scenario 5 — Authorization boundary (SC-004)

1. As the player session, attempt to call `createWall`/`createLightSource`/
   `createShape`, and a direct `POST` to the map-import endpoint, using a
   scene the player does not own.
2. Confirm all four requests are rejected server-side with an error,
   regardless of what the player's client UI shows.

**Pass condition**: 100% rejection, independent of client-side tool
visibility.

## Scenario 6 — Empty scene regression (SC-005)

1. Open a brand-new scene with zero walls, lights, shapes, or imported
   content.
2. Confirm it renders normally and tokens can be created/selected/moved
   exactly as before this feature (FR-014).

**Pass condition**: no errors, no missing default fog-of-war behavior.

## Automated coverage (for CI, not manual-only)

- Engine crate: extend `integration_tests.rs` with wall/lighting/shape
  plugin-independence tests (Constitution Principle II — each plugin must
  compile/run with the others absent) and an occlusion unit test.
- Server crate: mutation tests for scene-ownership rejection on all new
  mutations and the import endpoint, mirroring existing
  `mutations_walls.rs` coverage expectations; a dedicated UVTT-parser unit
  test suite using both `examples/maps/*.dd2vtt` fixtures (valid-file
  happy path plus the malformed/unsupported-version rejection cases from
  spec.md's Edge Cases).
- Web: Playwright e2e covering Scenario 1, Scenario 2 (import), and
  Scenario 4 (shapes) end-to-end (already-established e2e harness per the
  most recent `main` commit).
