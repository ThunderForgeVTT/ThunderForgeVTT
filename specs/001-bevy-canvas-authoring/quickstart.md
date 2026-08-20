# Quickstart: Validating Native Canvas Authoring

Prerequisites: dev database migrated (`diesel migration run` from
`src/server/`), server running (`cargo run` in `src/server/`), web app
running (`pnpm dev` or equivalent in `apps/web/`), two browser sessions —
one logged in as the scene's GM/owner, one as a non-owner player who is a
world member.

## Scenario 1 — Walls block vision (User Story 1, P1)

1. As the GM, open a scene with at least two tokens (one controllable by
   the player session).
2. Select the wall tool; drag a segment across the middle of the scene.
3. Confirm the wall persists after reloading the GM's page.
4. In the player session, confirm the area on the far side of the wall is
   no longer visible from the player's token.
5. As the GM, toggle the wall's "blocks vision" flag off; confirm the
   player's view updates within a few seconds without a reload.
6. As the GM, delete the wall; confirm the player's view returns to
   pre-wall visibility.
7. Confirm the player session at no point shows wall-editing handles or a
   wall tool (FR-009).

**Pass condition**: all six confirmations hold; SC-001 (<5s propagation)
observed on step 5.

## Scenario 2 — Lighting occluded by walls (User Story 2, P2)

1. As the GM, draw two walls forming a closed room boundary with one
   opening, on the same scene as Scenario 1.
2. Place a light source inside the room via the lighting tool.
3. Confirm the room is illuminated for the player session and an adjacent,
   wall-separated area remains dark (SC-002).
4. Attach the light to a token (drag the token in; confirm the light
   follows it as the GM moves the token).
5. Delete the light; confirm the room returns to its prior (dark/default)
   state for the player.

**Pass condition**: illumination respects wall occlusion; token-attached
light follows movement; deletion cleanly reverts.

## Scenario 3 — Freeform annotations (User Story 3, P3)

1. As the GM, draw a freehand stroke and a labeled shape on the scene
   using the native draw tool (not tldraw).
2. Reload the GM's page; confirm both persist.
3. Mark one annotation as visible-to-players; confirm the player session
   now sees it but cannot select/edit/delete it.
4. Confirm the other (GM-only) annotation is never sent to/rendered in the
   player session.
5. Delete both annotations as the GM; confirm removal on both sessions
   where applicable.

**Pass condition**: FR-007/FR-008/FR-009 all hold; no tldraw UI is present
anywhere in this flow.

## Scenario 4 — Authorization boundary (SC-004)

1. As the player session, attempt to call `createWall`/`createLightSource`/
   `createAnnotation` directly against the GraphQL endpoint (e.g. via
   browser devtools or a REST client), using a scene the player does not
   own.
2. Confirm all three requests are rejected server-side with an error,
   regardless of what the player's client UI shows.

**Pass condition**: 100% rejection, independent of client-side tool
visibility.

## Scenario 5 — Empty scene regression (SC-005)

1. Open a brand-new scene with zero walls, lights, or annotations.
2. Confirm it renders normally and tokens can be created/selected/moved
   exactly as before this feature (FR-014).

**Pass condition**: no errors, no missing default fog-of-war behavior.

## Automated coverage (for CI, not manual-only)

- Engine crate: extend `integration_tests.rs` with wall/light/annotation
  plugin-independence tests (Constitution Principle II — each plugin must
  compile/run with the other two absent) and an occlusion unit test.
- Server crate: mutation tests for scene-ownership rejection on all six
  new mutations, mirroring existing `mutations_walls.rs` coverage
  expectations.
- Web: Playwright e2e covering Scenario 1 and Scenario 3 end-to-end
  (already-established e2e harness per the most recent `main` commit).
