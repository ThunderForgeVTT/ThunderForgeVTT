# Quickstart: Player Onboarding — Invite-to-Actor Selection

Validation scenarios to run against a live local dev stack (real browser, not just `cargo test`) before this feature is considered done.

## Prerequisites

- Local server + Postgres running with migrations applied.
- Two distinct browser contexts/users available (a GM account, and one-or-two joining-player accounts) — Playwright's multi-context support is the easiest way to drive this.

## US1: GM-designated character claiming

1. As the GM, create two PC-classified Actors ("Aria", "Borin"). Mark both `available_for_claim = true` via `setActorAvailability`.
2. Generate an invite link (world dashboard's existing "Generate Join Link").
3. As a new user, follow the link, register/log in, click "Join Campaign."
4. **Expect**: land on `/world/:id/actor-select`, NOT `/world/:id` — the screen lists exactly "Aria" and "Borin," nothing else.
5. Select "Aria." **Expect**: claim succeeds, now recognized as Aria.
6. Log out, log back in, revisit the world. **Expect**: go straight to the world dashboard — Actor Selection is NOT shown again (FR-002/Acceptance Scenario 4).
7. As a second new user, follow the same invite link, join. **Expect**: Actor Selection now lists only "Borin" (Aria no longer offered).

## US1 edge case: zero available, create-off

1. As the GM, ensure `allow_player_created_actors = false` and zero actors are `available_for_claim`.
2. As a new joining player, join the world.
3. **Expect**: Actor Selection shows a clear "ask your GM" wait state — no error, no blank screen, no silent redirect. Confirm the player can still view world content per baseline non-claimed-member access (e.g. Compendium).

## US2: player-created character

1. As the GM, turn ON `allow_player_created_actors`.
2. As a new joining player, join. **Expect**: Actor Selection shows a "create your own" option alongside any available GM-designated characters.
3. Create a character. **Expect**: automatically claimed, no separate claim step; now recognized as that character.
4. As the GM, turn the setting back OFF. As a *different* new joining player (who has not yet claimed), view Actor Selection. **Expect**: no "create your own" option — reflects the current setting, not whatever it was when they joined (Acceptance Scenario 4). Also confirm server-side: attempt `createAndClaimActor` directly against the GraphQL API with the setting off — must be rejected even if a stale client UI somehow showed the option.

## US3: GM management

1. As the GM, view a PC actor's detail page. Toggle "available for claiming" on. **Expect**: appears on Actor Selection for a joining player.
2. After a player claims it, as the GM view the actor again. **Expect**: shows who currently has it claimed (FR-012).
3. Un-claim it. **Expect**: becomes available again on Actor Selection; the previously-claiming player, on next visit, is routed back to Actor Selection (not removed from the world — confirm they're still a world member and can still see baseline world content).
4. Mark a different, unclaimed actor as unavailable. **Expect**: disappears from Actor Selection; still exists/editable elsewhere in the Compendium.

## Concurrency (FR-006/SC-003)

1. As the GM, mark exactly one actor available.
2. Using two separate browser contexts (two different joining players, neither with a claim yet), trigger `claimActor` for that same actor at nearly the same instant (Playwright: fire both `claimActor` requests without awaiting the first before starting the second).
3. **Expect**: exactly one succeeds; the other receives the "just claimed" error and, on refreshing Actor Selection, no longer sees that actor listed.

## Session Setup invite link (FR-015/SC-005)

1. As the GM, navigate to `/world/:id/staging` (Session Setup) without visiting the world dashboard first.
2. **Expect**: the same invite URL obtainable from the dashboard's "Generate Join Link" is visible/copyable directly on this page, in one action.

## Cross-cutting checks

- `cargo test` (server, native) — all new resolver tests pass, including the unique-constraint-backstop test for concurrent claims.
- `cargo check` (server, native) clean; this feature has no wasm/engine surface.
- `pnpm --filter @thunderforge/web build` succeeds; `eslint` clean on new files.
- The GM's own invite-link visit is NEVER routed to Actor Selection (Edge Cases) — confirm explicitly.
