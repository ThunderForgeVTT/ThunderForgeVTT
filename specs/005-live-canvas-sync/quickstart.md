# Quickstart: Live Cross-Client Canvas Sync via GraphQL Subscriptions

Validation scenarios below map to spec.md's Acceptance Scenarios. Run against a local dev stack with two browser contexts (GM + player, or two GM-owned test accounts on the same world).

## Prerequisites

- `docker compose up`, server running (`cargo run -p thunderforge`), `apps/web` dev server running
- Two browser contexts (or two tabs in a private/incognito pairing) viewing the same scene
- A way to simulate a network drop for one client (e.g. browser devtools' offline toggle, or killing/restoring the dev server briefly)

## Scenario 1 — Live sync for walls/lights/shapes (US1)

1. Open the same scene in both contexts.
2. In context A, create a wall; confirm it appears in context B within a few seconds, no reload.
3. In context A, toggle that wall's passability; confirm context B updates the same way.
4. Repeat for a light source (create/move) and a shape annotation (draw/edit).
5. Confirm context A's own change never flickers or reverts when its own event round-trips back via the subscription.

**Expected**: matches FR-001–FR-007, SC-001, SC-002.

## Scenario 2 — Reconnect and resync (US2)

1. With both contexts live, simulate a network drop on context B (devtools offline mode).
2. Confirm context B shows a persistent "reconnecting" indicator, not a silent stale view.
3. While context B is offline, make a change in context A (e.g. move a light).
4. Restore context B's network; confirm it reconnects automatically, performs a full scene re-fetch, and now correctly shows the change made while it was offline — without a manual reload.
5. Confirm context B keeps retrying (does not require any manual action) if the drop is extended.

**Expected**: matches FR-008, FR-008a, FR-009, FR-009a, SC-003, SC-004.

## Scenario 3 — Authorization: a non-member cannot subscribe to a world's events (server-side correction)

1. As a user who is not a member of a given world, attempt to open `worldEventsCreated(worldId)` for that world's ID (e.g. via a raw GraphQL WebSocket client or a server-side test).
2. Confirm no events for that world are received/streamed.

**Expected**: matches the research.md §2 authorization correction — verifies the previously-latent gap is closed.

## Scenario 4 — Tokens ride the same transport (US3, once spec 004 lands)

1. With spec 004's token canvas authoring in place (or, in the interim, today's existing `upsert_token` GraphQL path), move/resize/rotate a token in context A.
2. Confirm context B sees the change within a few seconds, via the same transport as Scenario 1 — no token-specific transport code involved.

**Expected**: matches FR-002-005 applied to tokens, SC-005.

## Scenario 5 — A GM invites a genuine second player (US4)

1. As a GM, create a new world (or use an existing one) and open its campaign/invite settings.
2. Generate an invite code; confirm it succeeds (no argument-shape error) and returns a usable code/URL.
3. As a second, genuinely distinct user account (not the GM's own login in a second tab), use the invite code to join the world.
4. Confirm the second account is now a member and can view the world's scenes, with no wall/light/shape/token authoring controls shown (existing GM-only gate, unchanged).

**Expected**: matches FR-012-015, SC-006, SC-007. This scenario, once it passes, upgrades every earlier scenario in this file to be verifiable with a genuinely distinct second account instead of the GM's own login reused in a second browser context.

## Scenario 6 — Regression check

Run the full existing `cargo test` (server, including the new `world_events_created` authorization test and the new invite/membership tests) and `apps/web/e2e/canvas-authoring.spec.ts` suite, confirming no regression to existing wall/shape/light/token authoring or outbound mutation behavior (FR-006).
