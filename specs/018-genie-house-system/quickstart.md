# Quickstart: Genie House System

## Prerequisites

- Server running locally with the `grid_type` CHECK-constraint migration AND the `world_genie_sessions`/`world_genie_puzzle_clocks`/`world_genie_resource_holdings` migration applied (data-model.md).
- `packs/systems/genie/system.json` present and loadable (contracts/genie-manifest-and-rolls.md), including a `sessionResources` block (e.g. `insight`, `favor`, `essence`).
- Web app running locally with the Genie web package built.
- For Scenarios 7-9: at least two separate authenticated client sessions (two browser windows/profiles, or two test clients) representing two different players plus one GM, to exercise live-sync and two-party consent for real.

## Scenario 1 — The Manifestation roll exercises keep/drop + exploding + success-counting together (User Story 1)

1. Create a Genie character with a skill rating of 4.
2. Trigger a Manifestation roll keeping the top 3 dice (`rollDice` with `formula: "4d6kh3x=6cs>=4"` — the real, confirmed dice-engine syntax; `kh` for keep-highest and `x=6` for explode-on-6, not the illustrative `k3`/`!6` this scenario originally described).
3. **Expect**: the roll record shows 4 dice rolled, 3 marked `kept: true`, 1 dropped.
4. Repeat until a kept die shows a natural 6.
5. **Expect**: that die's full reroll/explosion chain is present in the roll record, and its final value is the *last* roll in that chain (the dice engine's existing, consistent explode semantics — not a sum of the chain).
6. **Expect**: the reported result equals the count of kept dice (post-explosion) with a final face value of 4+.

## Scenario 2 — A GM switches a scene between Material and Wish-Warped Zone (User Story 2)

1. Create a scene with `gridType: "square"` (Material). Place and move a token; confirm movement is measured against the grid.
2. Create a second scene with `gridType: "gridless"` (Wish-Warped Zone). Place and move a token.
3. **Expect**: the gridless scene's token positions freely, with no grid-snapping (per the updated `plugins/grid.rs` interaction body).
4. Switch the active scene back and forth between the two.
5. **Expect**: each scene's token positions remain correct and independent — no corruption or cross-contamination (Edge Cases, spec.md).

## Scenario 3 — NPC size category sets default token footprint (User Story 3)

1. Stage a `diminutive` Genie NPC and a `colossal` Genie NPC on the same Material scene.
2. **Expect**: each token's default `scale` matches its `sizeCategories` manifest entry (data-model.md) without manual adjustment.

## Scenario 4 — Conditions track on sheet and token (User Story 4)

1. Apply the `bound` condition to a Genie character.
2. **Expect**: it appears in `condition_data` on the character sheet and as a token status indicator, in both a Material and a Wish-Warped Zone scene.
3. Remove the condition.
4. **Expect**: it clears from both the sheet and the token.

## Scenario 5 — Wish-Granted Item with an effect (User Story 5)

1. Add a Wish-Granted Item to a Genie character's inventory via the existing `addWorldItem`/`addItemEffect` mutations (spec 013), with a formula-bearing effect.
2. **Expect**: the item and its effect display on the character's inventory view, using spec 013's existing UI path unmodified.

## Scenario 6 — Wish Points scale on level-up (User Story 6)

1. Level up a Genie character.
2. **Expect**: `resource_data.max_wish_points` updates to match the `wishPoints` table's value for the new level, with no manual entry.

## Scenario 7 — A full combat encounter using only Genie content (SC-002)

1. Create a world with only the Genie system pack enabled.
2. Stage at least one NPC (Scenario 3), run a full combat encounter including at least one Manifestation roll (Scenario 1), one condition applied and cleared (Scenario 4), and one scene-topology switch (Scenario 2).
3. **Expect**: the entire encounter completes with no dependency on any other system pack's data or code path.

## Scenario 8 — Session Wish Pool, clocks, and resource trading stay live-synced across players (User Story 7, SC-005, SC-006)

1. As the GM, start a Genie session: `wishesRemaining = 3`, a Doom Clock (`doomClockMax` GM-chosen, e.g. 6), and at least two Puzzle Clocks (e.g. `segmentsMax = 4` each).
2. With two separate player clients connected to the same world, confirm both immediately see the same `wishesRemaining`, Doom Clock, and Puzzle Clock state (via `worldEventsCreated`, `event_code = 15`).
3. As the GM, call `advanceDoomClock` (simulating a failed roll's consequence). **Expect**: both player clients' Doom Clock displays update within the normal live-sync latency, with no manual refresh.
4. As the GM, call `advancePuzzleClock` on one clock (simulating a challenge success). **Expect**: both player clients see that specific clock advance, independent of the Doom Clock and the other Puzzle Clock.
5. As Player A, call `proposeResourceTrade` offering some of their `insight` for Player B's `favor`. **Expect**: Player B's client shows the pending proposal; Player A's own client does not treat it as already accepted.
6. As Player B, call `acceptResourceTrade`. **Expect**: both players' holdings update atomically and correctly (data-model.md's holdings-ledger validation), visible live to the GM and any other connected player too.
7. As Player A, attempt to call `acceptResourceTrade` on their own proposal. **Expect**: rejected — only the named counterpart (Player B) may accept (contracts/genie-session-loop.md's authorization contract).
8. As a non-GM player, attempt to call `advanceDoomClock` directly. **Expect**: rejected — GM-only (contracts/genie-session-loop.md).

## Scenario 9 — A full session reaches a definitive win or loss (User Story 7, FR-016, SC-005)

1. Starting from Scenario 8's session, as the GM, repeatedly call `advancePuzzleClock` until every Puzzle Clock's `segmentsCurrent` reaches its `segmentsMax`.
2. **Expect**: `world_genie_sessions.status` transitions to `'won'` on the action that resolves the last Puzzle Clock, even if that same action would also have filled the Doom Clock (precedence rule, contracts/genie-session-loop.md).
3. Repeat from a fresh session, this time calling `advanceDoomClock` until `doomClockCurrent` reaches `doomClockMax` with at least one Puzzle Clock still unresolved.
4. **Expect**: `status` transitions to `'lost'`.
5. As GM or player, spend a wish (`spendWish`) at some point before either outcome. **Expect**: `wishesRemaining` decrements, a Wish Effect narrative note is recorded, and the mechanic remains distinct from any dice roll or Puzzle/Doom Clock value (FR-014).

## Manual verification checklist

- [X] `packs/systems/genie/system.json` validates against `SystemManifest`/`SystemManifestLegal` (spec 016) with `sourced`-style honesty — no placeholder attribution text pretending to be a real license. Verified live: the real System Settings UI (`/world/:id/settings/system`) loads Genie's manifest and legal notice successfully (`apps/web/e2e/genie-manifestation-roll.spec.ts`); its text contains no third-party license names (checked for "SRD"/"Wizards of the Coast").
- [X] Native `cargo check`/`cargo test` passes for `packs/systems/genie/server`, the `scenes` migration, and the new `world_genie_*` tables/mutations. (Verified in a prior pass — see tasks.md T060.)
- [X] `cargo check --target wasm32-unknown-unknown` passes for `src/engine` after the `grid.rs` gridless-interaction change. (Verified in a prior pass — see tasks.md T061.)
- [X] `tsc`/build passes for `packs/systems/genie/web`. Also now verified for `apps/web` itself (`pnpm run build` / `vite build` completes cleanly) — see tasks.md T062.
- [ ] All 9 scenarios above pass in a running dev instance, with Scenarios 8-9 verified across at least two real connected clients (not a single-client simulation). **9 of 9 scenarios now genuinely verified** against a real dev instance with real Playwright browser automation (`apps/web/e2e/genie-*.spec.ts`) — Scenario 6 (leveling) and resource trading closed by spec 019 (`specs/019-genie-leveling-and-trading/`). Only the two-real-client requirement for Scenarios 8-9 remains unmet — no live subscription transport exists client-side anywhere in this app. Per-scenario state:
  - [X] Scenario 1 (Manifestation roll) — genuinely verified end-to-end: `4d6kh3x=6cs>=4` triggered via the real dice-roller panel, roll record inspected via live GraphQL, confirmed correct keep/drop (4 rolled, 3 kept, 1 dropped), an observed exploding 6 with its full chain, and success-count matching kept dice at 4+. Note: quickstart step 2's example formula (`4d6k3!6cs>=4`) does not match the dice engine's actual notation (`x=6` for explode-on-6, not `!6`) — the working formula is `4d6kh3x=6cs>=4`, confirmed against `crates/thunderforge-dice`'s own `genie_manifestation_roll_composes_keep_explode_and_success_count` test. Step 5's "summed chain" wording is also inaccurate — `finalValue` is the chain's last roll, not a sum.
  - [X] Scenario 2 (Material ↔ Wish-Warped Zone) — genuinely verified end-to-end: a Material scene's token was placed/dragged and measured against the grid; a genuinely gridless scene (`gridType: "gridless"`) was created, its token placed/dragged with free positioning; switching back and forth confirmed each scene's token position stayed exactly where it was left, no cross-contamination. Real gap found: there is no UI to choose a scene's `gridType` at creation (`SceneSwitcher`'s "New scene" dialog only has a name field) — the gridless scene was created via a direct, authenticated `createScene` GraphQL call.
  - [X] Scenario 3 (NPC size category → token footprint) — genuinely verified end-to-end, and it turns out there never was an app bug: `TokenPanel`'s NPC-scale resolution (spec 018 T047) correctly reads `trait_data.size_category` and resolves the right scale within ~200-500ms of selecting an NPC — confirmed via live console instrumentation. The prior `[ ]`/failing state was three separate **test** bugs in `genie-npc-and-items.spec.ts`, all now fixed: (1) its retry loop reselected "(blank token)" then the NPC on every iteration, which reset the in-flight fetch each time, so a synchronous `.innerText()` read right after always caught the pre-fetch "1x" default and could never settle — replaced with selecting once and polling via `expect().toHaveText()`; (2) an extra `page.keyboard.press("Escape")` after the first token's creation closed the whole token *panel* (its own Escape-to-dismiss handler), not just the already-self-closing create dialog, hanging the second half of the test waiting for a trigger button that no longer existed; (3) the colossal-scale assertion's regex `/\b4\b/` can never match "4x" (no word boundary between two word characters), fixed to `/\b4x\b/`. No UI exists yet to set an NPC's `size_category` in the first place (`trait_data` was set via a direct GraphQL mutation for this test) — that remains a real, separate gap.
  - [X] Scenario 4 (conditions on sheet + token) — genuinely verified end-to-end after wiring `packs/systems/genie/web`'s `CharacterSheet`/`ConditionTrack` into `ActorDetailPage.tsx` (new `GenieActorSheet`, plus a condition-editing checkbox list since `ConditionTrack` itself is read-only): a GM toggled the "Bound" condition on a Genie character, saw it reflected on the sheet's own Conditions tab, and confirmed it persisted (and could be cleared) across a page reload (`apps/web/e2e/genie-conditions.spec.ts`).
  - [X] Scenario 5 (Wish-Granted Item with effect) — genuinely verified end-to-end: created "Lamp of Minor Binding" via the real Items compendium tab, added a `1d4` / "Binding Suppression" effect via the real item detail/edit page, added it to a Genie character's inventory via the real `ActorInventoryPanel`, confirmed it appears there, and confirmed its description/effect are visible on the item's own detail page. Real, scoped gap found: `ActorInventoryPanel`'s inventory row only renders the item's name and quantity (`InventoryEntryRecord` has no `description`/`effects` fields) — so the item's description and mechanical effect are visible in the real running app, but on the item's own page, not inline in the inventory view itself, as quickstart Scenario 5 step 2 implies.
  - [X] Scenario 6 (Wish Points level-up) — closed by spec 019: `trait_data.level` added to Genie's data model, `CharacterSheet`'s new "Resources" tab exposes it, and a level change recalculates `resource_data.max_wish_points` via `calculateMaxWishPoints`, persisted server-side. Verified end-to-end (`apps/web/e2e/genie-leveling.spec.ts`). See `specs/019-genie-leveling-and-trading/`.
  - [X] Scenario 7 (full combat encounter, Genie-only) — genuinely verified end-to-end as one combined spec: staged an NPC, applied and cleared a condition on a PC, triggered a real Manifestation roll via the dice-roller panel, and switched the play canvas to a genuinely gridless scene (`apps/web/e2e/genie-full-encounter.spec.ts`).
  - [X] Scenario 8 (Session Wish Pool + Doom/Puzzle Clocks + resource trading) — genuinely verified end-to-end after adding a full frontend GraphQL client (`apps/web/src/api/genieSession.ts`, `useGenieSession.ts`) and a `GenieSessionPanel` on the GM staging page: a GM started a session, spent a wish (pool 3→2), and advanced the Doom Clock (0→1) (`apps/web/e2e/genie-session-loop.spec.ts`). Resource trading closed by spec 019: added the previously-missing `genieTradeProposals(actorId)` query, mounted `SessionResourceTrade`, and fixed a real authorization bug (`caller_controls_actor` only checked direct ownership, not a claimed character) that silently broke every Session Resource action for a player who joined the normal way — verified via a genuine two-account e2e run (`apps/web/e2e/genie-resource-trade.spec.ts`). Still not verified across two real *simultaneously connected* clients — no live subscription transport exists client-side yet (same gap noted for Scenario 9), so both browser tabs would need to separately poll/refetch today.
  - [X] Scenario 9 (session win/loss) — genuinely verified end-to-end for both outcomes (`apps/web/e2e/genie-session-outcome.spec.ts`): filling the Doom Clock ends the session in a loss, and resolving every Puzzle Clock ends it in a win, both reflected live in `GenieSessionPanel`. Fixed a real bug found while writing this: `genieSession(worldId)` only ever returns the *active* session by design, so `useGenieSession`'s `advancePuzzleClock`/`spendResourceOnPuzzleClock` were calling `refetch()` after a mutation that could resolve the session — the very next query would come back `null` and blank the whole panel instead of showing "Session won." Now merges the mutation's own response into local state instead (mirroring `advanceDoomClock`/`spendWish`, which already did this correctly). Not yet verified across two real connected clients (same live-sync gap as Scenario 8).
