# Quickstart: Hand-Drawn Authoring & Per-Campaign Asset Storage

Validation scenarios below map 1:1 to spec.md's Acceptance Scenarios. Run against a
local dev stack (`docker compose up`, server, and `apps/web` dev server per the
project's existing local-dev flow — no new manual steps beyond that per FR-020/SC-008).

## Prerequisites

- `docker compose up` (now also starts the new `rustfs` service — see FR-020)
- Server running with `DATABASE_URL` and RustFS root credentials in `.env`
- `apps/web` dev server running
- Two test user accounts, each owning a separate world (for Scenario 4 below)

## Scenario 1 — Hand-drawn wall (US1, SC-001)

1. Log in as a GM, open a scene with no imported map.
2. Select the wall tool (`WallTool` panel).
3. Click three distinct points on the canvas; press the end-chain action.
4. Confirm a 2-segment wall renders immediately.
5. Reload the page; confirm the wall is still present, identically placed.
6. Place two test tokens on either side of the wall; confirm line-of-sight is blocked between them.
7. Select the wall, toggle door, confirm it renders as a door and blocks/passes correctly when closed/open.
8. Select the wall, delete; confirm it disappears and no longer blocks line of sight.
9. Start a new chain, place one point, press cancel; confirm nothing persists after reload.

**Expected**: all steps succeed within 10s of starting the interaction (SC-001).

## Scenario 2 — Hand-drawn shapes (US2, SC-002, SC-003)

1. On any scene, select the shape tool, freehand mode; drag a stroke; release.
2. Repeat for rectangle, ellipse, line/arrow (drag start→end) and text (click + type).
3. Confirm each shape/annotation appears immediately.
4. Switch to a second scene, then back; confirm Scene A's shapes are all present and Scene B's are not shown on Scene A (SC-003, at least 3 switches).
5. Select a shape, delete it; confirm it's gone for both the GM and a player viewing the same scene.

## Scenario 3 — Paste image to canvas (US3, SC-004, SC-005)

1. Copy a PNG (non-WebP) image to the system clipboard.
2. Focus the scene canvas as a GM, paste (Ctrl/Cmd+V).
3. Confirm the image appears on the canvas within 10s (SC-004).
4. Inspect the stored asset (via `canvasImageAssetsForScene` query or storage path) and confirm it is WebP, not PNG (SC-005).
5. Attempt to paste an image larger than the configured max size; confirm a clear error and no `CanvasImageAsset` row is created.
6. As a player who is a member of the world, load the same scene; confirm the pasted image renders.

## Scenario 4 — Cross-campaign asset isolation (US4, SC-006, SC-007)

1. User A (owns World 1) attempts `uploadCanvasImage` against User B's World 2, without being a member. Confirm rejection before any object exists in RustFS.
2. User B invites User A to World 2; User A accepts.
3. User A retries the same write against World 2; confirm it now succeeds.
4. User B removes User A from World 2.
5. User A attempts another write against World 2; confirm rejection.
6. Via a server-side test/log inspection of `write_object`'s internal call in step 3 (no GraphQL response ever carries a credential — it is minted and consumed entirely server-side): confirm the credential used was short-lived and scoped to exactly the one object key written, not the RustFS root/admin credential (SC-006, SC-007).
7. Attempt to read World 2's assets (`canvasImageAssetsForScene`) as User A after removal (step 4); confirm rejection before any row is returned (FR-014, FR-019).

## Scenario 5 — Local dev provisioning (FR-020, SC-008)

1. From a clean checkout, run the project's single provisioning command (`docker compose up`).
2. Confirm the RustFS service starts, and the server successfully authorizes a write (Scenario 3, step 2) without any manual bucket/credential setup step beyond what the compose stack already provisions.
