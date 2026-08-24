---

description: "Task list for Genie Session Resource Economy — Grants, NPC Shops, Quest/Contract Rewards"
---

# Tasks: Genie Session Resource Economy — Grants, NPC Shops, Quest/Contract Rewards

**Input**: Design documents from `/specs/020-genie-economy-and-shops/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/genie-economy.md, quickstart.md

**Tests**: Included — spec.md's Acceptance Scenarios are explicit and research.md R5 commits to a `cargo test` + Playwright strategy mirroring existing Genie coverage.

**Organization**: Tasks are grouped by user story (US1 grants, US2 shops, US3 clock rewards) per spec.md's priorities (P1, P2, P2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 / US2 / US3

## Path Conventions

Extends the existing Genie pack + core server per plan.md's Project Structure — no new top-level directories.

---

## Phase 1: Setup

**Purpose**: Diesel migrations for all three new schema pieces (data-model.md). All three are independent tables/columns — no ordering dependency between them, but all must land before any mutation code references the generated `schema.rs` symbols.

- [X] T001 [P] Create migration `src/server/migrations/<timestamp>_genie_resource_carryover_setting/{up,down}.sql` adding `worlds.genie_resource_carryover_enabled boolean NOT NULL DEFAULT false` (data-model.md)
- [X] T002 [P] Create migration `src/server/migrations/<timestamp>_genie_shop_listings/{up,down}.sql` creating `world_genie_shop_listings` per data-model.md's column list and `CHECK` constraints
- [X] T003 [P] Create migration `src/server/migrations/<timestamp>_genie_puzzle_clock_rewards/{up,down}.sql` creating `world_genie_puzzle_clock_rewards` per data-model.md's column list and `CHECK` constraints
- [X] T004 Run `diesel migration run` against the local dev database and confirm `src/server/src/schema.rs` regenerates with `worlds.genie_resource_carryover_enabled`, `world_genie_shop_listings`, and `world_genie_puzzle_clock_rewards`

**Checkpoint**: Schema exists; `cargo check` passes with the new tables visible to Diesel.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Rust model structs and shared helpers every user story's mutations depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 [P] Add `GenieShopListing`/`NewGenieShopListing` structs to `src/server/src/models.rs`, mirroring `GeniePuzzleClock`'s existing `Queryable`/`Insertable` derive pattern
- [X] T006 [P] Add `GeniePuzzleClockReward`/`NewGeniePuzzleClockReward` structs to `src/server/src/models.rs`, same pattern
- [X] T007 Add `"resource_grant"`, `"purchase"`, `"clock_reward"` to the `EVENT_CODE_GENIE_SESSION_STATE` payload `kind` handling in `src/server/src/world_events.rs` (or wherever the existing kinds are enumerated/documented, per research.md R3)
- [X] T008 [P] Add `resource_grant`/`purchase`/`clock_reward` branches to `apps/web/src/engine/world/sync/genieSession.ts`'s `applyGenieSessionWorldEvent` dispatcher (research.md R3 — additive, existing kinds already fall through to a generic refetch so this is a precision improvement, not a correctness fix)

**Checkpoint**: Model structs compile; event-kind plumbing is ready for every mutation added below to call.

---

## Phase 3: User Story 1 — GM grants Session Resources and items directly (Priority: P1) 🎯 MVP

**Goal**: A GM can hand a player Session Resources or an item outright, unblocking the whole economy (spec.md Problem statement — "trading is fully built and fully unusable" without this).

**Independent Test**: As GM, with an active Genie session, grant a resource/item to a player's character; confirm it appears in that character's holdings/inventory immediately, live on the recipient's own client.

### Tests for User Story 1

- [X] T009 [P] [US1] `cargo test` cases in `src/server/src/graphql/mutations_genie_session.rs`'s test module: `grantSessionResource` happy path increases holding by exact amount (Scenario 1); no active session → rejected with clear error (Scenario 3); non-GM caller → rejected (Scenario 4)
- [X] T010 [P] [US1] Playwright spec `apps/web/e2e/genie-resource-grant.spec.ts`, two-browser-context pattern mirroring `genie-resource-trade.spec.ts`: GM grants a resource in context A, player's `SessionResourceTrade` panel updates in context B without reload (Scenario 1); GM grants an item, confirm it appears in `ActorInventoryPanel` in context B without reload (Scenario 2)

### Implementation for User Story 1

- [X] T011 [US1] Implement `grantSessionResource(sessionId, actorId, resourceType, amount)` mutation in `src/server/src/graphql/mutations_genie_session.rs`, using the existing `set_holding_quantity` helper (`:208-259`) and the same `is_dm_of_world` GM-only check `spendWish`/`advanceDoomClock` use; reject with a clear error when no active session exists for `sessionId` (FR-001)
- [X] T012 [US1] Record a `world_events` NOTIFY row with `kind: "resource_grant"` on `grantSessionResource` success, reusing `record_world_event`/`EVENT_CODE_GENIE_SESSION_STATE` (FR-007)
- [X] T013 [US1] Add `worlds.genie_resource_carryover_enabled` to `startGenieSession` in `src/server/src/graphql/mutations_genie_session.rs`: when enabled, copy every character's ending holdings from the most recently concluded session (by `created_at`) into the new session's holdings before returning (FR-003); when disabled, unchanged existing behavior (Scenario 5)
- [X] T014 [P] [US1] Add `grantSessionResource` client call to `apps/web/src/api/genieSession.ts`, matching the existing `advanceDoomClock`/`spendWish` request shape
- [X] T015 [US1] Add a `grantSessionResource` action callback to `apps/web/src/hooks/useGenieSession.ts` (depends on T014), following the existing `advanceDoomClock` callback pattern (direct response merge, no extra refetch)
- [X] T016 [US1] Wire `onSessionStateChanged`/T007-T008's `"resource_grant"` kind into `useGenieSession.ts`'s live-sync effect (already generically calls `refetch()` for any kind — confirm the new kind is covered, add a dedicated branch only if a narrower update is warranted)
- [X] T017 [US1] Add a GM-only "Grant Resource" control to the Genie session panel (wherever `spendWish`/`advanceDoomClock` controls currently live, e.g. `apps/web/src/components/world/GenieSessionPanel/GenieSessionPanel.tsx`), calling T015's callback
- [X] T018 [US1] Add a GM-only "Grant" action to `apps/web/src/components/*/ActorInventoryPanel` (exact path per current component location) that calls the existing `addItemToInventory` mutation unchanged (FR-002) — no new mutation, only a new UI affordance
- [X] T019 [US1] Add a per-world "Resource Carryover" toggle to the world's Genie system settings surface (checking `WorldSystemSettingsPage.tsx` per research.md R1 for where a Genie-specific world setting belongs), calling a small `updateWorld`-style mutation/field for `genie_resource_carryover_enabled`
- [X] T020 [US1] Add non-GM rejection test coverage confirmation and run `pnpm exec tsc --noEmit` + `cargo check` (Constitution Principle V)

**Checkpoint**: User Story 1 fully functional and independently testable — grants work, carryover setting works, live sync confirmed across two clients.

---

## Phase 4: User Story 2 — NPC shop sells items for Session Resources or barter (Priority: P2)

**Goal**: A GM stocks an NPC's inventory and prices listings (resource or barter); players buy.

**Independent Test**: As GM, add items to an NPC's inventory, price one in Session Resources and another as an item-for-item trade; as a player, complete a purchase of each kind; confirm correct payment deduction, item transfer, and NPC stock decrement.

### Tests for User Story 2

- [X] T021 [P] [US2] `cargo test` cases: `createShopListing` GM-only + validation (exactly one of resource/item price pair populated, matching `price_kind`); `purchaseFromShop` resource-priced happy path (Scenario 1); insufficient resources → rejected, no state change (Scenario 2); barter happy path (Scenario 3); missing barter item → rejected (Scenario 4); no listings on an NPC → no shop UI signal from the query layer (Scenario 6)
- [X] T022 [P] [US2] `cargo test` concurrency case for FR-005a: two overlapping transactions purchasing a listing with `world_actor_inventory.quantity = 1`; assert exactly one succeeds and the other receives a clean "out of stock" error with no partial deduction (Scenario 5) — mirrors `accept_resource_trade_impl`'s existing transaction test structure
- [X] T023 [P] [US2] Playwright spec `apps/web/e2e/genie-npc-shop.spec.ts`: GM stocks an NPC, creates a resource-priced and a barter listing; player buys each; confirm inventory/holdings updates and live NPC-stock-decrement visibility across two browser contexts

### Implementation for User Story 2

- [X] T024 [US2] Implement `createShopListing(actorId, itemId, priceKind, priceResourceType, priceResourceAmount, priceItemId, priceItemQuantity)` mutation in `src/server/src/graphql/mutations_genie_session.rs` (or a new `mutations_genie_shop.rs` if the file is getting large — match existing file-size conventions), GM-only (`is_dm_of_world`), validating exactly one price pair is populated per `price_kind` (data-model.md)
- [X] T025 [US2] Implement `purchaseFromShop(listingId, buyerActorId)` mutation: single DB transaction (research.md R2, mirroring `accept_resource_trade_impl`) that verifies afford (resource balance via `load_holding_quantity`, or held item quantity for barter), deducts/transfers the price, transfers one unit of the listed item via `addItemToInventory`'s `_impl`, and performs the FR-005a atomic conditional stock decrement (`UPDATE world_actor_inventory SET quantity = quantity - 1 WHERE quantity > 0`, checking rows-affected) — authorized via `caller_controls_actor` on `buyerActorId`, not GM-only (FR-005, FR-005a)
- [X] T026 [US2] Record a `world_events` NOTIFY row with `kind: "purchase"` on `purchaseFromShop` success (FR-007)
- [X] T027 [P] [US2] Add a `GenieShopListing.stockQuantity` GraphQL resolver field deriving from `world_actor_inventory.quantity` for `(actorId, itemId)` (contracts/genie-economy.md — derived, not stored)
- [X] T028 [P] [US2] Add `createShopListing`/`purchaseFromShop` client calls and a shop-listings query to `apps/web/src/api/genieSession.ts`
- [X] T029 [US2] Add shop action callbacks (`createShopListing`, `purchaseFromShop`) to `apps/web/src/hooks/useGenieSession.ts` (depends on T028)
- [X] T030 [US2] Wire the `"purchase"` event kind into live-sync handling in `useGenieSession.ts` (same pattern as T016)
- [X] T031 [US2] Add a GM-only "Add Listing" UI on the NPC actor detail page (wherever `ActorInventoryPanel` renders for `is_npc: true`, e.g. `apps/web/src/pages/world/actor/ActorDetailPage.tsx` / `GenieActorSheet.tsx`), gated to only render when `is_npc` is true
- [X] T032 [US2] Add a shop panel/section to the player-facing NPC actor view showing configured listings with a "Buy" action per listing, rendering nothing when an NPC has zero listings (Scenario 6) — calling T029's `purchaseFromShop` callback
- [X] T033 [US2] Run `pnpm exec tsc --noEmit` + `cargo check` (Constitution Principle V)

**Checkpoint**: User Stories 1 AND 2 both work independently — shops function end-to-end, concurrent purchase safety verified.

---

## Phase 5: User Story 3 — Puzzle Clock segments carry configured rewards (Priority: P2)

**Goal**: A GM configures per-segment (or milestone) rewards on a Puzzle Clock — supporting both a single end-of-quest payout and a per-tick production run (e.g. 20 daggers, one per segment).

**Independent Test**: As GM, configure a Puzzle Clock with a reward on every segment and separately one with a single reward at the final segment; advance each; confirm rewards fire exactly at their configured segment(s), and a clock with zero configured rewards behaves exactly as it does today.

### Tests for User Story 3

- [X] T034 [P] [US3] `cargo test` cases: `configurePuzzleClockReward` GM-only + validation (exactly one of resource/item reward pair populated); per-segment reward fires exactly once per advance, not before/repeated (Scenario 3); zero-reward clock unchanged from spec 018/019 behavior (Scenario 4)
- [X] T035 [P] [US3] `cargo test` case: 20-entry "Forge Daggers" clock advanced one segment at a time with `actorId` passed each call grants exactly one dagger per advance, not a lump sum at segment 20 (Scenario 1); 4-segment "Recover the Sealed Lamp" clock with a single reward at segment 4 grants once, split across party (Scenario 2)
- [X] T036 [P] [US3] `cargo test` case for FR-006a: `advancePuzzleClock` called with no `actorId` against a `triggering_actor`-mode reward falls back to whole-party split rather than failing or crediting no one
- [X] T037 [P] [US3] Playwright spec `apps/web/e2e/genie-puzzle-clock-rewards.spec.ts`: GM configures a per-segment reward clock, advances it with an actor attributed, confirms the item lands in that actor's inventory live in a second browser context

### Implementation for User Story 3

- [X] T038 [US3] Implement `configurePuzzleClockReward(clockId, triggerSegment, rewardResourceType, rewardResourceAmount, rewardItemId, rewardItemQuantity, recipientMode)` mutation in `src/server/src/graphql/mutations_genie_session.rs`, GM-only, validating exactly one reward pair populated (data-model.md)
- [X] T039 [US3] Extend `advancePuzzleClock(clockId, delta)` to `advancePuzzleClock(clockId, delta, actorId: Option<Uuid>)` in `src/server/src/graphql/mutations_genie_session.rs` (FR-006a, backward-compatible — existing callers omitting `actorId` unaffected)
- [X] T040 [US3] Within `advancePuzzleClock`'s existing transaction, after the segment update: select `world_genie_puzzle_clock_rewards` rows for this clock where `granted_at IS NULL` and `trigger_segment` falls within `(old segments_current, new segments_current]`; for each, grant via `set_holding_quantity` (resource) or `addItemToInventory`'s `_impl` (item), crediting `actorId` if `recipient_mode = 'triggering_actor'` and `actorId` was supplied, else splitting/granting to the whole party (research.md R4, FR-006, FR-006a)
- [X] T041 [US3] Set `granted_at = now()` on each reward row granted in T040, in the same transaction, guaranteeing exactly-once (research.md R4, Scenario 3)
- [X] T042 [US3] Record a `world_events` NOTIFY row with `kind: "clock_reward"` when at least one reward fires on an advance (FR-007), in addition to the existing puzzle-clock-state NOTIFY
- [X] T043 [P] [US3] Update `apps/web/src/api/genieSession.ts`'s `advancePuzzleClock` client call to accept an optional `actorId`, and add `configurePuzzleClockReward`
- [X] T044 [US3] Update `useGenieSession.ts`'s `advancePuzzleClock` callback (depends on T043) to pass through the optional `actorId`; add a `configurePuzzleClockReward` callback
- [X] T045 [US3] Wire the `"clock_reward"` event kind into live-sync handling in `useGenieSession.ts` (same pattern as T016/T030)
- [X] T046 [US3] Add a GM-only "Configure Rewards" UI on each Puzzle Clock in the Genie session panel (add/list reward entries per segment), and update the existing "Advance" control to optionally attribute an actor (e.g. a dropdown of party members, defaulting to none)
- [X] T047 [US3] Run `pnpm exec tsc --noEmit` + `cargo check` (Constitution Principle V)

**Checkpoint**: All three user stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final verification across all three stories together.

- [X] T048 Run the full `quickstart.md` validation (Scenarios A, B, C) against a local two-browser-context setup
- [X] T049 [P] Run `cargo test` (full suite) and confirm no regressions in existing `mutations_genie_session.rs` tests or `genie_wish_granted_item_round_trips_through_inventory`
- [X] T050 [P] Run the full `apps/web/e2e/genie-*.spec.ts` suite and confirm no regressions
- [X] T051 Confirm no `NEEDS CLARIFICATION` markers or dangling TODOs remain in the three new mutation implementations; review authorization on all five new/changed mutations against contracts/genie-economy.md's authorization table

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — three migrations are `[P]`, but all must complete (and `diesel migration run`, T004) before Phase 2.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories (model structs and event-kind plumbing are shared).
- **User Stories (Phase 3-5)**: All depend on Foundational completion.
  - **US1 has no dependency on US2/US3** — it's the MVP and can ship alone.
  - **US2 has no dependency on US1** at the mutation level, but its shop UI naturally sits alongside US1's GM-grant UI in the same actor/session panels — implement after US1 if working solo, in parallel if staffed separately.
  - **US3 has no dependency on US1/US2** — Puzzle Clocks are independent of grants/shops; only shares the Foundational event-kind plumbing.
- **Polish (Phase 6)**: Depends on all three user stories being complete.

### Parallel Opportunities

- T001-T003 (migrations) in parallel.
- T005-T006 (model structs) in parallel; T008 in parallel with either.
- Once Foundational (Phase 2) completes, US1/US2/US3 phases can proceed in parallel if staffed by different developers — each touches `mutations_genie_session.rs` and `useGenieSession.ts`, so parallel work on those two shared files will need careful merge/rebase, but no story blocks another functionally.
- Within each story, test tasks marked `[P]` run in parallel with each other; client-API tasks marked `[P]` run in parallel with server-mutation tasks in the same story since they touch different files.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (all three migrations, even though only the carryover column and Foundational model prep are needed for US1 — cheaper to land all three schema changes in one migration pass than to re-run `diesel migration run` per story).
2. Complete Phase 2: Foundational.
3. Complete Phase 3: User Story 1.
4. **STOP and VALIDATE**: Run quickstart.md Scenario A independently.
5. This alone unblocks spec 019's trading feature (previously "fully built and fully unusable" per spec.md's Problem statement) — ship as MVP.

### Incremental Delivery

1. Setup + Foundational → Foundation ready.
2. Add User Story 1 → validate via quickstart Scenario A → deploy (MVP: grants work, trading becomes usable).
3. Add User Story 2 → validate via quickstart Scenario B → deploy (shops).
4. Add User Story 3 → validate via quickstart Scenario C → deploy (clock rewards).
5. Phase 6 polish once all three are in.
