# Quickstart: Validating the Genie Session Resource Economy

Validates spec 020's three User Stories end-to-end once implemented. Mirrors
spec 018's quickstart pattern (two connected clients where cross-client sync
matters).

## Prerequisites

- Local stack running (server + web dev servers, Postgres migrated with this
  spec's three new migrations applied).
- A world running the Genie system, with an active Genie session (spec 018
  `startGenieSession`) and at least one party PC actor.
- Two logged-in browser sessions: one as the world's GM, one as a player who
  controls a party PC (claimed or owned, per spec 018/019's
  `caller_controls_actor`).

## Scenario A — GM grants a resource and an item (User Story 1)

1. As GM, open the Genie session panel; grant 3 Essence to the player's PC.
2. On the player's client (no manual reload), confirm the PC's
   `SessionResourceTrade` holdings display updates to reflect +3 Essence.
3. As GM, grant an existing world item to the same PC via
   `ActorInventoryPanel`'s new "Grant" action.
4. On the player's client, confirm the item appears in `ActorInventoryPanel`
   without reload.
5. As GM, end the session (or attempt a grant with no active session);
   confirm the grant mutation is rejected with a clear error.
6. As the player, attempt to call the grant mutation directly (e.g. via
   devtools/API) against their own or another PC; confirm rejection —
   granting is GM-only.
7. Enable `genie_resource_carryover_enabled` for the world; end the session
   with nonzero holdings; start a new session; confirm holdings carry over.
   Disable it; repeat; confirm holdings reset to 0.

**Expected outcome**: All 5 Acceptance Scenarios of User Story 1 pass; step
2/4's live update (no reload) confirms FR-007's NOTIFY wiring works for the
new `"resource_grant"` event kind.

## Scenario B — NPC shop, resource-priced and barter listings (User Story 2)

1. As GM, create/select an NPC actor; add 2 items to its inventory.
2. Create a shop listing on one item priced at 2 Insight
   (`createShopListing`, `priceKind: RESOURCE`).
3. Create a second listing priced as item-for-item barter
   (`priceKind: ITEM`) against an item the player already holds.
4. As a player with ≥2 Insight, purchase the resource-priced listing;
   confirm Insight holding -2, item appears in player inventory, NPC stock
   -1.
5. As a player with <2 Insight, attempt the same purchase; confirm rejection
   with no state change.
6. As a player holding the required barter item, purchase the barter
   listing; confirm the barter item leaves their inventory, the listed item
   arrives, and the NPC's stock adjusts symmetrically.
7. As a player without the barter item, attempt that purchase; confirm
   rejection.
8. Set a listing's backing inventory quantity to 1; from two browser
   contexts (or two rapid overlapping requests), attempt to purchase it
   simultaneously; confirm exactly one succeeds and the other receives a
   clean "out of stock" error with no partial deduction (FR-005a).
9. View an NPC with zero listings configured; confirm no shop UI appears.

**Expected outcome**: All 6 Acceptance Scenarios of User Story 2 pass,
including the FR-005a concurrency case in step 8.

## Scenario C — Puzzle Clock segment rewards (User Story 3)

1. As GM, create a Puzzle Clock ("Forge Daggers," 20 segments); configure a
   reward entry on every segment 1-20, `recipientMode: TRIGGERING_ACTOR`,
   each granting 1 "Dagger" item.
2. Advance the clock one segment at a time, passing the smith actor's
   `actorId` on each `advancePuzzleClock` call; confirm each advance grants
   exactly one dagger to that actor (check inventory after each advance —
   not a lump sum at segment 20).
3. Separately, create a Puzzle Clock ("Recover the Sealed Lamp," 4
   segments) with a single reward entry at segment 4 only (2 Favor,
   `recipientMode: WHOLE_PARTY`); advance it to segment 4; confirm the
   reward grants once, split across the current party.
4. Attempt to trigger the same reward entry twice (e.g. re-advance past an
   already-granted segment, if reachable); confirm no double-grant
   (`granted_at` guard).
5. Create/advance a Puzzle Clock with zero configured reward entries;
   confirm behavior is unchanged from spec 018/019 (no side effects).
6. Advance a `TRIGGERING_ACTOR`-mode reward's segment via a plain
   `advancePuzzleClock` call with no `actorId` supplied; confirm it falls
   back to whole-party split rather than failing or crediting no one
   (FR-006a).

**Expected outcome**: All 4 Acceptance Scenarios of User Story 3 pass, plus
FR-006a's fallback behavior confirmed in step 6.

## Verification commands (per Constitution Principle V)

```bash
# Server
cargo check
cargo test -p thunderforge-server genie_economy   # or whatever module/test-name filter tasks.md lands on

# Web
pnpm --filter web exec tsc --noEmit
pnpm --filter web exec playwright test genie-economy   # or the actual spec filename tasks.md lands on
```
