# Quickstart: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

Validation scenarios below map to spec.md's Acceptance Scenarios. Run against a local dev stack (`docker compose up`, server, `apps/web` dev server), with a GM account owning a world and a second (player) account invited to it.

## Prerequisites

- `docker compose up` (Postgres + RustFS)
- Server running (`cargo run -p thunderforge`), migrations applied (new `tokens` columns from data-model.md)
- `apps/web` dev server running
- A scene with at least one GM-created token, with `ownerUserId` assigned to the player test account and `isPrimary: true`

## Scenario 1 — GM drags a token on the canvas (US1)

1. As the GM, open a scene with an existing token.
2. Click and drag the token to a new grid location directly on the canvas (do not open the token panel).
3. Confirm the token visually follows the drag and settles at the drop point.
4. Reload the page; confirm the token is still at the new position.
5. As a second (player) browser context viewing the same scene, confirm the token's new position appears within a few seconds, no reload.
6. Open the token panel while the token is selected; confirm its displayed position matches the canvas.

**Expected**: matches FR-001–FR-005, SC-001, SC-002.

## Scenario 2 — Resize and rotate via canvas handles (US2)

1. As the GM, select the token; drag its resize handle — confirm it grows/shrinks only in whole grid-cell increments (never a fractional cell).
2. Drag its rotate handle independently; confirm facing changes without affecting size.
3. Reload; confirm both persist exactly.
4. As the player, confirm no resize/rotate handles are visible on any token, including their own.

**Expected**: matches FR-006–FR-008, FR-010.

## Scenario 3 — Player controls their own token(s) (US3)

1. As the player, confirm exactly one token is marked as their primary token (visually distinguished or confirmable via the panel).
2. Drag their primary token on the canvas; confirm it moves and syncs to the GM.
3. Attempt to drag a token not assigned to them (an NPC or another player's token); confirm no movement occurs.
4. As the GM, grant the player control of a second token (e.g. a summoned creature); confirm the player can now drag that token too.
5. As the player, change their primary token's photo via the panel; confirm the GM and other players see the updated image. Confirm the player has no "create token" control anywhere.

**Expected**: matches FR-009, FR-009a, FR-009b, SC-003.

## Scenario 4 — Scene-switch loading and error feedback (US4)

1. As the GM, switch to a different scene via SceneSwitcher; confirm a loading indicator appears immediately and clears once the scene (background, walls, lights, tokens) is fully rendered.
2. As a connected player, confirm the same loading → ready sequence appears on their client without a manual reload.
3. Simulate a background-asset load failure (e.g. temporarily break the asset URL/permissions); switch to that scene; confirm a visible, distinct error state appears (not a blank or stuck canvas), with a retry action.
4. Fix the underlying issue; click retry; confirm the scene now loads successfully without switching away and back.

**Expected**: matches FR-011–FR-013a, SC-004, SC-005, SC-006.

## Scenario 5 — Regression check

Run the existing `apps/web/e2e/canvas-authoring.spec.ts` and this feature's new e2e suite together, plus `cargo test` (server) and `cargo check --target wasm32-unknown-unknown` (engine), confirming no regression to wall/shape/lighting authoring or the existing `TokenPanel` health-bar/bulk-CRUD responsibilities that remain in scope.
